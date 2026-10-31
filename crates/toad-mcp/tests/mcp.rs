//! MCP server: protocol handshake, tools over a real crawl, and the
//! Streamable HTTP transport (with its cross-origin guard).

use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_mcp::Server;
use toad_store::Store;
use url::Url;

async fn site(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, body) = match path.as_str() {
                "/" => (200, r#"<html><head><title>Home</title></head><body><a href="/a">A</a> <a href="/gone">Gone</a></body></html>"#),
                "/a" => (200, r#"<html><head><title>A page</title></head><body><a href="/">Home</a></body></html>"#),
                _ => (404, "<html><head><title>Not found</title></head><body></body></html>"),
            };
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(dir: &std::path::Path) -> (String, String) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(site(listener));
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig::default();
    let path = dir.join("site.toad");
    let mut store = Store::create(&path, &start, &cfg).unwrap();
    store.mark_started("MCP test", "spider").unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg, &start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    store.insert_batch(&records).unwrap();
    store.mark_finished(0).unwrap();
    store.analyse().unwrap();
    (path.display().to_string(), origin)
}

fn call(server: &Server, name: &str, args: Value) -> (String, bool) {
    let r = server.handle(&json!({"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
    let res = &r["result"];
    (res["content"][0]["text"].as_str().unwrap_or("").to_string(), res["isError"].as_bool().unwrap_or(false))
}

#[tokio::test(flavor = "multi_thread")]
async fn mcp_tools_and_http() {
    let dir = std::env::temp_dir().join(format!("toad-mcp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let (crawl, origin) = crawl(&dir).await;
    let server = Server::new(&dir);

    // Handshake.
    let init = server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "t", "version": "1"}}})).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(init["result"]["serverInfo"]["name"], "screaming-toad");
    assert!(server.handle(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"})).is_none(), "notifications get no reply");
    let tools = server.handle(&json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"})).unwrap();
    let names: Vec<&str> = tools["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"query_urls") && names.contains(&"crawl_issues") && names.contains(&"compare_crawls"));
    assert!(names.contains(&"crawl") && names.contains(&"crawl_progress") && names.contains(&"export"));
    let unknown = server.handle(&json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list"})).unwrap();
    assert_eq!(unknown["error"]["code"], -32601);

    // Tools.
    let (list, _) = call(&server, "list_crawls", json!({}));
    assert!(list.contains("MCP test") && list.contains("site.toad"), "{list}");
    let (issues, _) = call(&server, "crawl_issues", json!({"crawl": crawl}));
    assert!(issues.contains("Internal Client Error (4xx)"), "{issues}");
    let (rows, err) = call(&server, "query_urls", json!({"crawl": "site.toad", "tab": "Response Codes", "filter": "Internal Client Error (4XX)"}));
    assert!(!err && rows.contains(&format!("{origin}/gone")), "{rows}");
    let (details, _) = call(&server, "url_details", json!({"crawl": crawl, "url": format!("{origin}/a")}));
    assert!(details.contains("title_1: A page"), "{details}");
    let (inlinks, _) = call(&server, "inlinks", json!({"crawl": crawl, "url": format!("{origin}/gone")}));
    assert!(inlinks.lines().count() >= 2, "{inlinks}");
    let (report, _) = call(&server, "run_report", json!({"crawl": crawl, "name": "Crawl Overview", "limit": 5}));
    assert!(report.starts_with("Tab\tFilter\tURLs"), "{report}");
    let (bad, is_err) = call(&server, "query_urls", json!({"crawl": crawl, "tab": "Nope"}));
    assert!(is_err && bad.contains("unknown tab"), "{bad}");
    let (same, _) = call(&server, "compare_crawls", json!({"current": crawl, "previous": crawl}));
    assert!(same.contains("same file"), "{same}");

    // Export a filter and a report to files.
    let csv = dir.join("out").join("4xx.csv");
    let (wrote, err) = call(&server, "export", json!({"crawl": crawl, "tab": "Response Codes", "filter": "Internal Client Error (4XX)", "out": csv}));
    assert!(!err && wrote.contains("Wrote 1 rows"), "{wrote}");
    assert!(std::fs::read_to_string(&csv).unwrap().contains(&format!("{origin}/gone")));
    let xlsx = dir.join("issues.xlsx");
    let (wrote, err) = call(&server, "export", json!({"crawl": crawl, "report": "Issues Overview", "out": xlsx}));
    assert!(!err && wrote.contains("Wrote"), "{wrote}");
    assert!(std::fs::read(&xlsx).unwrap().starts_with(b"PK"));
    let (missing, is_err) = call(&server, "crawl_progress", json!({"crawl": "never-started.toad"}));
    assert!(is_err && missing.contains("no crawl file"), "{missing}");

    // Streamable HTTP.
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let http = tokio::spawn(Server::new(&dir).serve_listener(listener, async {
        let _ = stop_rx.await;
    }));
    let post = |body: String, origin: Option<&str>| {
        let origin = origin.map(|o| format!("Origin: {o}\r\n")).unwrap_or_default();
        async move {
            let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let req = format!("POST /mcp HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nAccept: application/json, text/event-stream\r\n{origin}Content-Length: {}\r\n\r\n{body}", body.len());
            s.write_all(req.as_bytes()).await.unwrap();
            let mut out = String::new();
            s.read_to_string(&mut out).await.unwrap();
            out
        }
    };
    let resp = post(json!({"jsonrpc": "2.0", "id": 9, "method": "tools/list"}).to_string(), None).await;
    assert!(resp.starts_with("HTTP/1.1 200") && resp.contains("query_urls"), "{resp}");
    let note = post(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string(), None).await;
    assert!(note.starts_with("HTTP/1.1 202"), "{note}");
    let evil = post(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string(), Some("https://evil.example")).await;
    assert!(evil.starts_with("HTTP/1.1 403"), "{evil}");
    let local = post(json!({"jsonrpc": "2.0", "id": 1, "method": "ping"}).to_string(), Some("http://localhost:3000")).await;
    assert!(local.starts_with("HTTP/1.1 200"), "{local}");
    let _ = stop_tx.send(());
    http.await.unwrap().unwrap();
}
