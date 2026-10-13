//! Crawl Analysis: with Auto-analyse off, a crawl ends with link counts but
//! no Link Score or near duplicates, and is flagged as needing analysis;
//! Crawl Analysis > Start fills them in. Resuming clears the flag again.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::CrawlAnalysisConfig;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

const TEXT: &str = "Toads are amphibians that live in damp places near ponds and streams and eat insects at night across the garden. They breed in shallow water each spring, laying long strings of eggs that hatch into tadpoles within a couple of weeks. Adult toads return to the same breeding pond year after year, often crossing roads on mild wet evenings to get there. Their dry warty skin holds glands that make them taste bad to most predators, although grass snakes still eat them.";

fn respond(path: &str) -> String {
    let page = |title: &str, body: &str| format!("<!doctype html><html><head><title>{title}</title></head><body>{body}</body></html>");
    match path {
        "/" => page("Home", r#"<a href="/a">A</a> <a href="/b">B</a> <a href="/c">C</a>"#),
        // /a and /b are near duplicates; /c differs.
        "/a" => page("A", &format!("<p>{TEXT} Alpha.</p><a href=\"/\">Home</a>")),
        "/b" => page("B", &format!("<p>{TEXT} Beta.</p><a href=\"/\">Home</a>")),
        _ => page("C", "<p>Completely different words about frogs, newts and salamanders in winter.</p>"),
    }
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let body = respond(&path);
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn manual_crawl_analysis() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig {
        threads: 2,
        near_duplicates: true,
        near_duplicate_threshold: 0.8,
        crawl_analysis: CrawlAnalysisConfig { auto_analyse: false, ..Default::default() },
        ..CrawlConfig::default()
    };
    let dir = std::env::temp_dir().join(format!("toad-analysis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let _ = std::fs::remove_file(&db);
    let mut store = Store::create(&db, &start, &cfg).unwrap();
    store.mark_started("test", "spider").unwrap();
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
    store.finish_crawl_analysis().unwrap();

    let field = |store: &Store, url: &str, key: &str| store.url_details(url).unwrap().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default();
    let home = start.to_string();
    // Always worked out: link counts.
    assert_eq!(field(&store, &home, "unique_inlinks"), "2", "/a and /b link home");
    // Waiting for Crawl Analysis.
    assert!(!store.analysed().unwrap());
    assert_eq!(field(&store, &home, "link_score"), "");
    assert_eq!(store.count("Content", "Near Duplicates").unwrap(), 0);

    store.analyse().unwrap();
    assert!(store.analysed().unwrap());
    assert!(!field(&store, &home, "link_score").is_empty());
    assert_eq!(store.count("Content", "Near Duplicates").unwrap(), 2);

    // Reopening for a resume makes the analysis stale again.
    store.mark_started("", "spider").unwrap();
    assert!(!store.analysed().unwrap());
}
