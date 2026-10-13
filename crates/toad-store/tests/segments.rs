//! Configuration > Segments: first-match precedence over column and Issue
//! rules, the Segment column, the global segment filter, the Segments
//! Overview, applying segments after the crawl, and sitemaps by segment.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::{Segment, SegmentOp, SegmentRule};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, SitemapOptions, Store};
use url::Url;

fn respond(path: &str) -> String {
    let page = |title: &str, body: &str| {
        let head = if title.is_empty() { String::new() } else { format!("<title>{title}</title>") };
        format!("<!doctype html><html><head>{head}</head><body>{body}</body></html>")
    };
    match path {
        "/" => page("Home", r#"<a href="/blog/one">1</a> <a href="/blog/two">2</a> <a href="/shop/item-7">7</a> <a href="/about">About</a>"#),
        "/blog/one" => page("Post one", "<p>Post</p>"),
        // Missing title: the Issue rule puts it in "Fix me" even though it's a blog URL.
        "/blog/two" => page("", "<p>Post</p>"),
        "/shop/item-7" => page("Item", "<p>Item</p>"),
        _ => page("About", "<p>About us</p>"),
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

fn rule(field: &str, op: SegmentOp, value: &str) -> SegmentRule {
    SegmentRule { field: field.into(), op, value: value.into() }
}

#[tokio::test(flavor = "multi_thread")]
async fn segments() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig { threads: 2, ..CrawlConfig::default() };
    let dir = std::env::temp_dir().join(format!("toad-segments-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut store = Store::create(&dir.join("crawl.toad"), &start, &cfg).unwrap();
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

    // Segments are applied to the finished crawl.
    store
        .set_segments(vec![
            Segment { name: "Fix me".into(), rules: vec![rule("Issue", SegmentOp::Equals, "Page Titles: Missing")], ..Default::default() },
            Segment { name: "Blog".into(), rules: vec![rule("Address", SegmentOp::Contains, "/blog/")], ..Default::default() },
            Segment {
                name: "Shop".into(),
                match_all: true,
                rules: vec![rule("Address", SegmentOp::Regex, r"/shop/item-\d+$"), rule("Status Code", SegmentOp::LessThan, "300")],
                ..Default::default()
            },
        ])
        .unwrap();

    let all = store.rows(&RowQuery { tab: "Internal".into(), filter: "HTML".into(), ..Default::default() }).unwrap();
    let seg_col = all.columns.iter().position(|c| c == "Segment").unwrap();
    assert_eq!(all.columns[seg_col - 1], "Address", "Segment sits after Address");
    let segment_of = |path: &str| all.rows.iter().find(|r| r[0] == format!("{origin}{path}")).map(|r| r[seg_col].clone()).unwrap();
    assert_eq!(segment_of("/blog/one"), "Blog");
    assert_eq!(segment_of("/blog/two"), "Fix me", "the first matching segment wins");
    assert_eq!(segment_of("/shop/item-7"), "Shop");
    assert_eq!(segment_of("/about"), "");

    // Segments Overview, in configuration order, then No Segment.
    let overview = store.segments_overview().unwrap();
    let counts: Vec<(String, String)> = overview.rows.iter().map(|r| (r[0].clone(), r[1].clone())).collect();
    assert_eq!(counts, [("Fix me", "1"), ("Blog", "1"), ("Shop", "1"), ("No Segment", "2")].map(|(a, b)| (a.to_string(), b.to_string())));
    assert_eq!(overview.rows[0][4], "1", "the Fix me page has an issue");
    assert_eq!(store.report("Segments Overview").unwrap().rows.len(), 4);

    // The global segment filter narrows every tab, count and issue.
    store.set_segment_filter(Some("Blog".into()));
    assert_eq!(store.count("Internal", "HTML").unwrap(), 1);
    assert_eq!(store.count("Page Titles", "Missing").unwrap(), 0);
    store.set_segment_filter(Some("Fix me".into()));
    assert_eq!(store.count("Page Titles", "Missing").unwrap(), 1);
    assert!(store.issues().unwrap().iter().any(|i| i.filter == "Missing" && i.urls == 1));
    store.set_segment_filter(Some("No Segment".into()));
    assert_eq!(store.count("Internal", "HTML").unwrap(), 2);
    store.set_segment_filter(None);
    assert_eq!(store.count("Internal", "HTML").unwrap(), 5);

    // Saved with the crawl.
    drop(store);
    let store = Store::open(&dir.join("crawl.toad")).unwrap();
    assert_eq!(store.config().segments.len(), 3);

    // XML Sitemap by segment: one file per segment plus an index.
    let out = dir.join("sitemaps");
    let files = store.generate_sitemap(&out, &SitemapOptions { by_segment: true, ..Default::default() }).unwrap();
    let names: Vec<String> = files.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
    for want in ["sitemap-fix-me.xml", "sitemap-blog.xml", "sitemap-shop.xml", "sitemap-other.xml", "sitemap_index.xml"] {
        assert!(names.iter().any(|n| n == want), "{want} in {names:?}");
    }
    assert!(std::fs::read_to_string(out.join("sitemap-blog.xml")).unwrap().contains("/blog/one"));
    assert!(std::fs::read_to_string(out.join("sitemap_index.xml")).unwrap().contains("/sitemap-shop.xml"));
}

#[test]
fn segment_validation() {
    let cfg = CrawlConfig {
        segments: vec![
            Segment { name: "A".into(), rules: vec![rule("Address", SegmentOp::Regex, "(")], ..Default::default() },
            Segment { name: "a".into(), rules: vec![rule("Word Count", SegmentOp::GreaterThan, "lots")], ..Default::default() },
        ],
        ..CrawlConfig::default()
    };
    let problems = cfg.validate();
    assert!(problems.iter().any(|p| p.contains("not a valid regex")), "{problems:?}");
    assert!(problems.iter().any(|p| p.contains("used twice")));
    assert!(problems.iter().any(|p| p.contains("needs a number")));
}
