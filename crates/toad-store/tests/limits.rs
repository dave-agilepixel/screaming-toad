//! Spider > Limits: per subdomain, per crawl depth and by URL path.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::PathLimit;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord};
use url::Url;

/// Home links /p1–/p8; each /pN links /deep/N (17 pages).
async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let body = if path == "/" {
                (1..=8).map(|i| format!("<a href=\"/p{i}\">P{i}</a> ")).collect::<String>()
            } else if let Some(n) = path.strip_prefix("/p") {
                format!("<a href=\"/deep/{n}\">Deeper</a>")
            } else {
                "<a href=\"/\">Home</a>".into()
            };
            let body = format!("<html><head><title>T</title></head><body>{body}</body></html>");
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(start: &Url, cfg: CrawlConfig) -> Vec<PageRecord> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let run = tokio::spawn(Crawler::new(cfg, start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
    let mut out = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => out.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    out
}

#[tokio::test(flavor = "multi_thread")]
async fn crawl_limits() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let start = Url::parse(&format!("http://127.0.0.1:{}/", listener.local_addr().unwrap().port())).unwrap();
    tokio::spawn(serve(listener));
    let base = CrawlConfig { threads: 1, ..CrawlConfig::default() };

    assert_eq!(crawl(&start, base.clone()).await.len(), 17);

    // Per subdomain: everything is on one host.
    assert_eq!(crawl(&start, CrawlConfig { max_urls_per_subdomain: Some(5), ..base.clone() }).await.len(), 5);

    // Per crawl depth: at most 3 URLs at each depth (1 + 3 + 3).
    let per_depth = crawl(&start, CrawlConfig { max_urls_per_depth: Some(3), ..base.clone() }).await;
    let at = |d: u32| per_depth.iter().filter(|r| r.depth == d).count();
    assert_eq!((at(0), at(1), at(2)), (1, 3, 3));

    // By URL path: at most 2 /deep/ pages.
    let by_path = crawl(&start, CrawlConfig { path_limits: vec![PathLimit { pattern: "/deep/".into(), max: 2 }], ..base.clone() }).await;
    assert_eq!(by_path.iter().filter(|r| r.url.contains("/deep/")).count(), 2);
    assert_eq!(by_path.len(), 1 + 8 + 2);

    // An invalid pattern is a configuration problem.
    let bad = CrawlConfig { path_limits: vec![PathLimit { pattern: "(".into(), max: 1 }], ..base };
    assert!(bad.validate().iter().any(|p| p.contains("Limit by URL Path")));
}
