//! Include / Exclude: List mode inputs are filtered, Exclude stops robots.txt
//! fetches, and Exclude can be changed on a running crawl.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::RobotsMode;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord};
use url::Url;

async fn serve(listener: TcpListener, robots_hits: Arc<AtomicUsize>) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let robots_hits = robots_hits.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let body = match path.as_str() {
                "/robots.txt" => {
                    robots_hits.fetch_add(1, Ordering::SeqCst);
                    "User-agent: *\nDisallow:\n".to_string()
                }
                "/" => "<html><body><a href=\"/blog/a\">A</a> <a href=\"/blog/b\">B</a> <a href=\"/shop/c\">C</a></body></html>".into(),
                _ => "<html><body>Page</body></html>".into(),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(seeds: Vec<Url>, cfg: CrawlConfig, control: CrawlControl) -> Vec<PageRecord> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg, &seeds[0]).unwrap().run(seeds, tx, control));
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

fn paths(records: &[PageRecord]) -> Vec<String> {
    let mut v: Vec<String> = records.iter().map(|r| Url::parse(&r.url).unwrap().path().to_string()).collect();
    v.sort();
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn include_and_exclude() {
    let hits = Arc::new(AtomicUsize::new(0));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener, hits.clone()));
    let u = |p: &str| Url::parse(&format!("{origin}{p}")).unwrap();
    let base = CrawlConfig { threads: 1, ..CrawlConfig::default() };

    // List mode: Include and Exclude apply to every uploaded URL.
    let list = CrawlConfig { include: vec!["/blog/".into()], exclude: vec!["/b$".into()], ..base.clone() }.for_list_mode();
    let got = crawl(vec![u("/blog/a"), u("/blog/b"), u("/shop/c")], list, CrawlControl::default()).await;
    assert_eq!(paths(&got), vec!["/blog/a"]);

    // Spider mode: the start URL is always crawled; Exclude applies to the rest.
    let spider = CrawlConfig { exclude: vec!["/shop/".into()], ..base.clone() };
    assert_eq!(paths(&crawl(vec![u("/")], spider, CrawlControl::default()).await), vec!["/", "/blog/a", "/blog/b"]);

    // Exclude applies to robots.txt fetches.
    hits.store(0, Ordering::SeqCst);
    let respect = CrawlConfig { robots_mode: RobotsMode::Respect, exclude: vec!["robots\\.txt".into()], ..base.clone() };
    crawl(vec![u("/")], respect, CrawlControl::default()).await;
    assert_eq!(hits.load(Ordering::SeqCst), 0, "robots.txt is excluded");
    crawl(vec![u("/")], base.clone(), CrawlControl::default()).await;
    assert!(hits.load(Ordering::SeqCst) > 0);

    // Exclude changed on the running crawl's control overrides the config.
    let control = CrawlControl::default();
    control.set_exclude(&["/blog/".to_string()]).unwrap();
    assert_eq!(paths(&crawl(vec![u("/")], base, control).await), vec!["/", "/shop/c"]);
    assert!(CrawlControl::default().set_exclude(&["(".to_string()]).is_err());
}
