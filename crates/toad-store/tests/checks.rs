//! Screaming Frog checks Toad was missing, crawled from a local site:
//! HTTP Refresh redirects, Bad Content Type, Broken Bookmark, the raw-source
//! Validation checks, Missing Size Attributes, Fonts / Plugins / XML, the
//! Mobile tab's source checks and Rich Result Feature Detected.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, Store};
use url::Url;

const VIEWPORT: &str = r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#;
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n0000IHDR";

fn page(title: &str, head: &str, body: &str) -> Vec<u8> {
    format!("<!doctype html><html lang=\"en\"><head><title>{title}</title>{VIEWPORT}{head}</head><body>{body}</body></html>").into_bytes()
}

/// (status, content type, extra headers, body).
fn respond(path: &str) -> (u16, &'static str, String, Vec<u8>) {
    let html = "text/html";
    match path {
        "/" => (200, html, String::new(), page(
            "Home page of the checks test site",
            r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Product","name":"Toad","offers":{"@type":"Offer","price":"5","priceCurrency":"GBP"}}</script>"#,
            r#"<h1>Home</h1><img src="/pic.png" alt="A toad"><img src="/sized.png" alt="Sized" width="10" height="10">
               <a href="/refresh">Refresh</a> <a href="/bookmarks">Bookmarks</a> <a href="/fake.png">Fake image</a>
               <a href="/badtype">Bad type</a> <a href="/order">Order</a> <a href="/early-body">Early body</a> <a href="/plugin">Plugin</a>
               <a href="/f.woff2">Font</a> <a href="/feed.xml">Feed</a> <a href="/no-viewport">No viewport</a>"#,
        )),
        // A Refresh response header, not a meta tag.
        "/refresh" => (200, html, "Refresh: 0; url=/bookmarks\r\n".into(), page("Refreshes to the bookmarks page", "", "<h1>Refresh</h1>")),
        "/bookmarks" => (200, html, String::new(), page(
            "Bookmarks to sections that do and don't exist",
            "",
            r##"<h1>Bookmarks</h1><h2 id="here">Here</h2><a href="#here">Fine</a> <a href="#missing">Broken</a>
                <a href="/refresh#nowhere">Broken elsewhere</a> <a href="#top">Top</a>"##,
        )),
        // An HTML error page served as an image, and an invalid MIME type.
        "/fake.png" => (200, "image/png", String::new(), page("Not found", "", "<h1>Not found</h1>")),
        "/badtype" => (200, "html", String::new(), page("Served with an invalid content type", "", "<h1>Bad type</h1>")),
        "/order" => (200, html, String::new(), b"<html><p>first</p><head><title>Head not first, with a span in it</title><span>bad</span></head><body><h1>Order</h1></body></html>".to_vec()),
        "/early-body" => (200, html, String::new(), b"<body><html><head><title>Body tag before the html tag</title></head></html>".to_vec()),
        "/plugin" => (200, html, String::new(), page(
            "A page with a plugin and a mobile alternate",
            r#"<link rel="alternate" media="only screen and (max-width: 640px)" href="https://m.example.com/plugin">"#,
            r#"<h1>Plugin</h1><embed src="/movie.swf">"#,
        )),
        "/no-viewport" => (200, html, String::new(), b"<!doctype html><html lang=\"en\"><head><title>A page without a viewport tag</title></head><body><h1>No viewport</h1></body></html>".to_vec()),
        "/pic.png" | "/sized.png" => (200, "image/png", String::new(), PNG.to_vec()),
        "/f.woff2" => (200, "font/woff2", String::new(), b"wOF2".to_vec()),
        "/feed.xml" => (200, "application/rss+xml", String::new(), b"<?xml version=\"1.0\"?><rss/>".to_vec()),
        "/movie.swf" => (200, "application/x-shockwave-flash", String::new(), b"FWS".to_vec()),
        _ => (404, html, String::new(), page("Not found", "", "")),
    }
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, ctype, extra, body) = respond(&path);
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(&body).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(origin: &str) -> Store {
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig { threads: 2, store_mobile_alternate: true, ..CrawlConfig::default() };
    let db = std::env::temp_dir().join(format!("toad-checks-{}.toad", std::process::id()));
    let _ = std::fs::remove_file(&db);
    let mut store = Store::create(&db, &start, &cfg).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
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
    store
}

fn addresses(store: &Store, tab: &str, filter: &str) -> Vec<String> {
    let t = store.rows(&RowQuery { tab: tab.into(), filter: filter.into(), ..Default::default() }).unwrap();
    let col = t.columns.iter().position(|c| c == "Address").unwrap();
    let mut v: Vec<String> = t.rows.iter().map(|r| r[col].clone()).collect();
    v.sort();
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn missing_checks() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let u = |p: &str| format!("{origin}{p}");
    let store = crawl(&origin).await;

    // Response Codes: a Refresh header is a redirect, and is followed.
    assert_eq!(addresses(&store, "Response Codes", "Internal Redirection (HTTP Refresh)"), vec![u("/refresh")]);
    assert_eq!(addresses(&store, "Response Codes", "Redirection (HTTP Refresh)"), vec![u("/refresh")]);
    assert_eq!(store.count("Response Codes", "External Redirection (HTTP Refresh)").unwrap(), 0);
    assert!(store.inlinks(&u("/bookmarks")).unwrap().rows.iter().any(|r| r[0] == "Redirect" && r[1] == u("/refresh")));

    // Security: an HTML page served as image/png, and an invalid MIME type.
    assert_eq!(addresses(&store, "Security", "Bad Content Type"), vec![u("/badtype"), u("/fake.png")]);
    assert_eq!(store.count("Security", "All").unwrap(), 7, "All is still the HTML pages");

    // URL: #missing on the page itself and #nowhere on /refresh; #here and #top are fine.
    assert_eq!(addresses(&store, "URL", "Broken Bookmark"), vec![u("/bookmarks"), u("/refresh")]);
    let report = store.report("Broken Bookmarks").unwrap();
    let mut broken: Vec<String> = report.rows.iter().map(|r| r[1].clone()).collect();
    broken.sort();
    assert_eq!(broken, vec![u("/bookmarks#missing"), u("/refresh#nowhere")]);

    // Validation, from the source as written.
    assert_eq!(addresses(&store, "Validation", "<head> Not First In <html> Element"), vec![u("/order")]);
    assert_eq!(addresses(&store, "Validation", "Invalid HTML Elements in <head>"), vec![u("/order")]);
    assert_eq!(addresses(&store, "Validation", "<body> Element Preceding <html>"), vec![u("/early-body")]);

    // Images: /pic.png has no width or height; /sized.png has both.
    assert_eq!(addresses(&store, "Images", "Missing Size Attributes"), vec![u("/pic.png")]);

    // Internal content types.
    assert_eq!(addresses(&store, "Internal", "Fonts"), vec![u("/f.woff2")]);
    assert_eq!(addresses(&store, "Internal", "XML"), vec![u("/feed.xml")]);
    assert_eq!(addresses(&store, "Internal", "Plugins"), vec![u("/movie.swf")]);
    assert!(!addresses(&store, "Internal", "Other").contains(&u("/f.woff2")));

    // Mobile tab checks that need no rendering.
    assert_eq!(addresses(&store, "Mobile", "Viewport Not Set"), vec![u("/early-body"), u("/no-viewport"), u("/order")]);
    assert_eq!(addresses(&store, "Mobile", "Contains Unsupported Plugins"), vec![u("/plugin")]);
    assert_eq!(addresses(&store, "Mobile", "Mobile Alternate Link"), vec![u("/plugin")]);
    assert_eq!(store.count("Mobile", "Content Not Sized Correctly").unwrap(), 0, "needs rendering");

    // Structured Data: the home page's Product is a rich result type.
    assert_eq!(addresses(&store, "Structured Data", "Rich Result Feature Detected"), vec![u("/")]);

    // Embedding filters are listed (empty) without Embeddings.
    assert_eq!(store.count("Content", "Semantically Similar").unwrap(), 0);
    assert_eq!(store.count("Content", "Low Relevance Content").unwrap(), 0);
}
