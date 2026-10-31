//! Screaming Frog parity fixes, crawled from a local site: HTTP canonicals,
//! lazy-loaded images, SearchAction query-input, heading order and the
//! Issues Overview report.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, Store};
use url::Url;

fn page(title: &str, body: &str, head: &str) -> String {
    format!("<!doctype html><html lang=\"en\"><head><title>{title}</title>{head}</head><body>{body}</body></html>")
}

const SEARCH_ACTION: &str = r#"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"WebSite","url":"/","name":"Test",
    "potentialAction":[{"@type":"SearchAction","target":{"@type":"EntryPoint","urlTemplate":"/?s={search_term_string}"},"query-input":"required name=search_term_string"}]}]}</script>"#;

/// (status, extra headers, body).
fn respond(path: &str) -> (u16, String, String) {
    match path {
        "/" => (200, String::new(), page(
            "Home page of the parity test site",
            r#"<h1>Home</h1><h2>Section</h2><a href="/pdf-like">Header canonical</a> <a href="/lazy">Lazy</a> <a href="/skip">Skip</a>
               <a href="/series">Series</a> <a href="/series-2">Page 2</a>"#,
            SEARCH_ACTION,
        )),
        // A canonical only in the Link header used to abort the crawl.
        "/pdf-like" => (200, "Link: </>; rel=\"canonical\"\r\n".into(), page("Canonical in the HTTP header only", "<h1>Header</h1><h2>Two</h2>", "")),
        "/lazy" => (200, String::new(), page(
            "LiteSpeed lazy-loaded images",
            r#"<h1>Lazy</h1><h2>Images</h2>
               <img data-lazyloaded="1" src="data:image/svg+xml;base64,PHN2Zz4=" data-src="/real.webp" alt="">
               <img src="data:image/svg+xml;base64,PHN2Zz4=" data-src="/described.webp" alt="A described image">"#,
            "",
        )),
        "/skip" => (200, String::new(), page("H1 followed by an H3 before the H2", "<h1>Skip</h1><h3>Too deep</h3><h2>Late</h2>", "")),
        // rel=next with no <a href> to page 2 on the page itself.
        "/series" => (200, String::new(), page("First page of a paginated series", "<h1>Series</h1><h2>One</h2>", r#"<link rel="next" href="/series-2">"#)),
        "/series-2" => (200, String::new(), page("Second page of a paginated series", r#"<h1>Series</h1><h2>Two</h2><a href="/series">Previous</a>"#, r#"<link rel="prev" href="/series">"#)),
        "/real.webp" | "/described.webp" => (200, String::new(), "WEBP".into()),
        _ => (404, String::new(), page("Not found", "", "")),
    }
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, extra, body) = respond(&path);
            let ctype = if path.ends_with(".webp") { "image/webp" } else { "text/html" };
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(origin: &str, lazy: bool) -> Store {
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig { threads: 2, extract_lazy_loaded_images: lazy, ..CrawlConfig::default() };
    let db = std::env::temp_dir().join(format!("toad-parity-{lazy}-{}.toad", std::process::id()));
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
    t.rows.iter().map(|r| r[col].clone()).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn parity_fixes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let u = |p: &str| format!("{origin}{p}");
    let store = crawl(&origin, false).await;

    // The HTTP canonical is stored (and no longer aborts the batch insert).
    assert_eq!(addresses(&store, "Canonicals", "Canonicalised"), vec![u("/pdf-like")]);
    let inlinks = store.inlinks(&u("/")).unwrap();
    assert!(inlinks.rows.iter().any(|r| r[0] == "HTTP Canonical"), "{:?}", inlinks.rows);

    // Lazy-loaded images are left alone by default, as in Screaming Frog
    // without rendering; with Extract Lazy-Loaded Images they're crawled
    // from data-src and the empty alt counts.
    assert_eq!(addresses(&store, "Images", "All").len(), 0);
    let lazy = crawl(&origin, true).await;
    assert_eq!(addresses(&lazy, "Images", "All").len(), 2);
    assert_eq!(addresses(&lazy, "Images", "Missing Alt Text"), vec![u("/real.webp")]);
    assert_eq!(lazy.url_count().unwrap(), store.url_count().unwrap() + 2);

    // query-input on a SearchAction is not a schema.org error.
    assert_eq!(store.count("Structured Data", "Validation Errors").unwrap(), 0);
    assert_eq!(store.count("Structured Data", "Contains Structured Data").unwrap(), 1);

    // H1 then H3 before the first H2.
    assert_eq!(addresses(&store, "H2", "Non-sequential"), vec![u("/skip")]);
    assert_eq!(store.count("H1", "Non-sequential").unwrap(), 0);

    // Issues Overview: Screaming Frog's columns, a row per issue.
    let report = store.report("issues overview").unwrap();
    assert_eq!(report.columns, ["Issue Name", "Issue Type", "Issue Priority", "URLs", "% of Total", "Description", "How To Fix"]);
    let h2 = report.rows.iter().find(|r| r[0] == "H2: Non-Sequential").expect("H2 row");
    assert_eq!((h2[1].as_str(), h2[2].as_str(), h2[3].as_str(), h2[4].as_str()), ("Warning", "Low", "1", "16.667"));
    assert!(!h2[5].is_empty() && !h2[6].is_empty(), "{h2:?}");
    assert!(report.rows.iter().all(|r| !r[6].is_empty()), "every issue has a fix: {:?}", report.rows.iter().filter(|r| r[6].is_empty()).collect::<Vec<_>>());

    // Crawl Overview ends with Screaming Frog's depth and top inlinks sections.
    let overview = store.report("Crawl Overview").unwrap();
    let section = |tab: &str| overview.rows.iter().filter(|r| r[0] == tab).map(|r| (r[1].clone(), r[2].clone(), r[3].clone())).collect::<Vec<_>>();
    let depth = section("Depth (Clicks from Start URL)");
    assert_eq!(depth.len(), 11);
    assert_eq!(depth[0], ("0".into(), "1".into(), "16.7%".into()));
    assert_eq!(depth[1], ("1".into(), "5".into(), "83.3%".into()));
    assert_eq!(depth[10].0, "10+");
    let top = section("Inlinks (Top 20 URLs)");
    assert_eq!(top.len(), 5, "every page but home, which nothing links to: {top:?}");
    assert_eq!(top[0], (u("/series"), "2".into(), "33.3%".into()), "linked from home and page 2");

    // Screaming Frog's names, and Toad's older ones, resolve to the same filter.
    let same = |tab: &str, a: &str, b: &str| assert_eq!(store.count(tab, a).unwrap(), store.count(tab, b).unwrap(), "{tab}: {a} / {b}");
    same("Content", "Low Content Pages", "Low Content Pages (under 200 words)");
    assert_eq!(store.count("Content", "Low Content Pages").unwrap(), 5, "every indexable page (/pdf-like is canonicalised)");
    same("URL", "Contains Space", "Contains A Space");
    same("Validation", "Resource Over 2MB", "Resource Over 2 MB");
    same("Response Codes", "Internal Client Error (4xx)", "Internal Client Error (4XX)");
    same("H2", "Non-Sequential", "non-sequential");
    same("Accessibility", "WCAG 2.0 A Violation", "WCAG 2.0 A Violations");
    assert!(store.count("URL", "Contains Spaces Everywhere").is_err(), "no fuzzy matching beyond that");
    assert_eq!(store.count("Response Codes", "Internal All").unwrap(), 6, "six pages; the lazy images aren't crawled by default");
    assert_eq!(store.count("Response Codes", "External All").unwrap(), 0);

    // Pagination URL Not in Anchor Tag: page 2 is linked, but not from /series.
    assert_eq!(addresses(&store, "Pagination", "Pagination URL Not in Anchor Tag"), vec![u("/series")]);
    assert_eq!(store.count("Pagination", "Unlinked Pagination URLs").unwrap(), 0);
}
