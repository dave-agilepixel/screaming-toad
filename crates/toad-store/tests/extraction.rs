//! Spider > Extraction toggles: switched-off elements leave their tabs empty;
//! HTTP Headers and Cookies are captured when switched on; Meta Robots off
//! means a noindex page counts as indexable.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::Extraction;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (extra, body) = match path.as_str() {
                "/" => (
                    "Set-Cookie: sid=abc; Path=/; HttpOnly\r\nSet-Cookie: theme=dark; Max-Age=60\r\nX-Served-By: toad-test\r\n",
                    "<html><head><title>Home</title></head><body><h1>Home</h1><a href=\"/hidden\">x</a></body></html>",
                ),
                _ => ("", "<html><head><title>Hidden</title><meta name=\"robots\" content=\"noindex\"></head><body><h1>Hidden</h1></body></html>"),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn extraction_toggles() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(listener));
    let start = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    let cfg = CrawlConfig {
        extraction: Extraction { page_title: false, meta_robots: false, http_headers: true, cookies: true, hash: false, ..Extraction::default() },
        ..CrawlConfig::default()
    };
    let db = std::env::temp_dir().join(format!("toad-extraction-{port}.toad"));
    let mut store = Store::create(&db, &start, &cfg).unwrap();
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
    store.analyse().unwrap();
    let home = start.to_string();
    let hidden = format!("http://127.0.0.1:{port}/hidden");

    // Page Title off: no titles stored, and the tab reports nothing (not "Missing").
    assert_eq!(store.count("Page Titles", "Missing").unwrap(), 0);
    assert_eq!(store.count("Page Titles", "All").unwrap(), 0);
    let detail = |url: &str, key: &str| store.url_details(url).unwrap().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap();
    assert_eq!(detail(&home, "title_1"), "");
    assert_eq!(detail(&home, "hash"), "", "Hash Value off");
    assert_eq!(detail(&home, "h1_1"), "Home", "H1 still on");

    // Meta Robots off: the noindex isn't seen.
    assert_eq!(detail(&hidden, "indexable"), "1");
    assert_eq!(store.count("Directives", "Noindex").unwrap(), 0);

    // HTTP Headers and Cookies on.
    let headers = store.http_headers(&home).unwrap();
    assert!(headers.rows.iter().any(|r| r[0].eq_ignore_ascii_case("x-served-by") && r[1] == "toad-test"));
    let cookies = store.cookies(&home).unwrap();
    assert_eq!(cookies.rows.len(), 2);
    assert!(cookies.rows.iter().any(|r| r[0] == "sid" && r[6] == "true"));
    assert!(store.report("HTTP Header Summary").unwrap().rows.iter().any(|r| r[0].eq_ignore_ascii_case("x-served-by")));
    assert_eq!(store.report("All Cookies").unwrap().rows.len(), 2);
    let internal = store.rows(&toad_store::RowQuery { tab: "Internal".into(), filter: "HTML".into(), ..Default::default() }).unwrap();
    let col = internal.columns.iter().position(|c| c == "Cookies").expect("Cookies column when Extraction > Cookies is on");
    assert!(internal.rows.iter().any(|r| r[col] == "2"));
}
