//! MCP crawl tools end to end: `crawl` runs this `toad` binary against a
//! local site, `crawl_progress` follows it, `export` writes a report.

use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};
use toad_mcp::Server;

fn serve(listener: TcpListener) {
    for mut sock in listener.incoming().flatten() {
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            let n = sock.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/");
            let (status, body) = match path {
                "/" => (200, r#"<html><head><title>Home page of the MCP crawl test</title></head><body><h1>Home</h1><a href="/a">A</a> <a href="/gone">Gone</a></body></html>"#),
                "/a" => (200, r#"<html><head><title>A page of the MCP crawl test</title></head><body><h1>A</h1><a href="/">Home</a></body></html>"#),
                _ => (404, "<html><head><title>Not found</title></head><body></body></html>"),
            };
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes());
            let _ = sock.write_all(body.as_bytes());
        });
    }
}

fn call(server: &Server, name: &str, args: Value) -> (String, bool) {
    let r = server.handle(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": name, "arguments": args}})).unwrap();
    let res = &r["result"];
    (res["content"][0]["text"].as_str().unwrap_or("").to_string(), res["isError"].as_bool().unwrap_or(false))
}

#[test]
fn crawl_progress_export() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || serve(listener));
    let dir = std::env::temp_dir().join(format!("toad-mcp-crawl-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut server = Server::new(&dir);
    server.toad_bin = Some(env!("CARGO_BIN_EXE_toad").into());

    let (started, err) = call(&server, "crawl", json!({"url": format!("{origin}/"), "output": "site.toad", "threads": 2}));
    assert!(!err && started.contains("site.toad"), "{started}");
    let deadline = Instant::now() + Duration::from_secs(120);
    let progress = loop {
        let (p, err) = call(&server, "crawl_progress", json!({"crawl": "site.toad"}));
        assert!(!err, "{p}");
        if !p.starts_with("Status: running") {
            break p;
        }
        assert!(Instant::now() < deadline, "crawl did not finish: {p}");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(progress.contains("Status: Complete") && progress.contains("URLs: 3"), "{progress}");
    assert!(progress.contains("Response Codes: Internal Client Error (4xx)"), "{progress}");

    let out = dir.join("issues.csv");
    let (wrote, err) = call(&server, "export", json!({"crawl": "site.toad", "report": "Issues Overview", "out": out}));
    assert!(!err, "{wrote}");
    let csv = std::fs::read_to_string(&out).unwrap();
    assert!(csv.starts_with("Issue Name,Issue Type,Issue Priority,URLs,% of Total,Description,How To Fix"), "{csv}");
    assert!(csv.contains("Response Codes: Internal Client Error (4xx),Issue,High,1,"), "{csv}");
}

/// An unknown --save-report name is skipped with a warning; the other
/// reports are still written.
#[test]
fn save_report_skips_unknown_names() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    std::thread::spawn(move || serve(listener));
    let dir = std::env::temp_dir().join(format!("toad-save-report-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_toad"))
        .args(["crawl", &format!("{origin}/"), "--threads", "1", "--save-report", "Not A Report,Issues Overview,Crawl Overview"])
        .arg("--out")
        .arg(dir.join("c.toad"))
        .arg("--output-folder")
        .arg(&dir)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("no report or bulk export named 'Not A Report'"), "{stderr}");
    assert!(dir.join("issues_overview.csv").exists() && dir.join("crawl_overview.csv").exists());
}
