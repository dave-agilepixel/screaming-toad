//! Stop a crawl part-way (by the URL limit), resume it from the crawl file,
//! and check it ends with exactly the URLs a full crawl finds.

use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord};
use toad_store::{CrawlStatus, Store};
use url::Url;

/// Home links /p1–/p8; each /pN links /deep/N (17 pages in all).
fn respond(path: &str) -> (u16, String) {
    let page = |title: &str, body: &str| format!("<!doctype html><html><head><title>{title}</title></head><body>{body}</body></html>");
    if path == "/" {
        let links: String = (1..=8).map(|i| format!("<a href=\"/p{i}\">P{i}</a> ")).collect();
        return (200, page("Home", &links));
    }
    if let Some(n) = path.strip_prefix("/p") {
        return (200, page(&format!("Page {n}"), &format!("<a href=\"/deep/{n}\">Deeper</a> <a href=\"/\">Home</a>")));
    }
    if path.starts_with("/deep/") {
        return (200, page("Deep page", "<a href=\"/\">Home</a>"));
    }
    (404, page("Not found", ""))
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, body) = respond(&path);
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

/// Drain a crawl's events into the store; returns the final queue length.
async fn drain(store: &mut Store, mut rx: tokio::sync::mpsc::Receiver<CrawlEvent>) -> u64 {
    let mut records: Vec<PageRecord> = Vec::new();
    let mut queued = 0;
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(p) => {
                queued = p.queued;
                break;
            }
            _ => {}
        }
    }
    store.insert_batch(&records).unwrap();
    queued
}

fn urls(db: &Path) -> Vec<String> {
    let (mut crawled, _) = Store::open(db).unwrap().resume_state().unwrap();
    crawled.sort();
    crawled
}

#[tokio::test(flavor = "multi_thread")]
async fn resume_a_stopped_crawl() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(serve(listener));
    let start = Url::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
    let dir = std::env::temp_dir().join(format!("toad-resume-{port}"));
    std::fs::create_dir_all(&dir).unwrap();

    // A full crawl, for reference.
    let full = dir.join("full.toad");
    {
        let cfg = CrawlConfig { threads: 1, ..CrawlConfig::default() };
        let mut store = Store::create(&full, &start, &cfg).unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let run = tokio::spawn(Crawler::new(cfg, &start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
        assert_eq!(drain(&mut store, rx).await, 0);
        run.await.unwrap().unwrap();
    }
    assert_eq!(urls(&full).len(), 17);

    // The same crawl, stopped after 3 URLs.
    let part = dir.join("part.toad");
    {
        let cfg = CrawlConfig { threads: 1, max_urls: 3, ..CrawlConfig::default() };
        let mut store = Store::create(&part, &start, &cfg).unwrap();
        store.mark_started("test", "spider").unwrap();
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let run = tokio::spawn(Crawler::new(cfg, &start).unwrap().run(vec![start.clone()], tx, CrawlControl::default()));
        let queued = drain(&mut store, rx).await;
        run.await.unwrap().unwrap();
        assert!(queued > 0, "the URL limit leaves URLs queued");
        store.mark_finished(queued).unwrap();
        assert_eq!(store.status().unwrap(), CrawlStatus::Stopped);
    }
    assert_eq!(urls(&part).len(), 3);

    // Resume with the limit lifted.
    {
        let mut store = Store::open(&part).unwrap();
        let (crawled, pending) = store.resume_state().unwrap();
        assert!(!pending.is_empty());
        let cfg = CrawlConfig { threads: 1, ..CrawlConfig::default() };
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        let run = tokio::spawn(Crawler::new(cfg, &start).unwrap().resume(crawled, pending, tx, CrawlControl::default()));
        let queued = drain(&mut store, rx).await;
        let progress = run.await.unwrap().unwrap();
        assert_eq!(queued, 0);
        assert_eq!(progress.crawled, 17, "progress counts the URLs from before the resume");
        store.mark_finished(queued).unwrap();
        store.analyse().unwrap();
        assert_eq!(store.status().unwrap(), CrawlStatus::Complete);
    }
    assert_eq!(urls(&part), urls(&full));

    // Depths survive the resume: /deep/N is two clicks from home.
    let store = Store::open(&part).unwrap();
    let depth = store
        .url_details(&format!("http://127.0.0.1:{port}/deep/5"))
        .unwrap()
        .into_iter()
        .find(|(k, _)| k == "depth")
        .map(|(_, v)| v)
        .unwrap();
    assert_eq!(depth, "2");
}
