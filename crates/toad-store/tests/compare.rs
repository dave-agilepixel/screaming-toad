//! Crawl a small local site twice, changing it in between, and check that
//! Compare mode reports the differences (with URL Mapping lining up the
//! previous crawl's trailing-slash URLs with the current crawl's).

use std::path::Path;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::compare::{ChangeElement, CompareConfig, CompareState, Comparison, LinkState, UrlMapping};
use toad_store::{RowQuery, Store};
use url::Url;

const TOADS: &str = "Toads are amphibians that live on land for most of the year and return to ponds to breed in spring. \
They have dry warty skin, short legs and a broad body, and they walk rather than hop. \
Common toads eat insects, slugs and worms, and they can live for many years in a garden.";

const FROGS: &str = "Frogs have smooth moist skin and long powerful legs built for leaping across wet ground. \
Most frogs stay close to water all year, laying clumps of eggs that float near the surface of ponds. \
Their calls fill summer evenings, and each species has a distinct song used to attract a mate.";

fn page(title: &str, body: &str, head_extra: &str) -> String {
    format!("<!doctype html><html lang=\"en\"><head><title>{title}</title>{head_extra}</head><body>{body}</body></html>")
}

/// Version 1 links pages with a trailing slash; version 2 without.
fn respond(path: &str, version: u8) -> (u16, String) {
    let v1 = version == 1;
    let p = |s: &str| if v1 { format!("{s}/") } else { s.to_string() };
    let ok = |body: String| (200, body);
    match (path, v1) {
        ("/", _) => {
            let extra = if v1 { format!("<a href=\"{}\">Gone</a>", p("/gone")) } else { format!("<a href=\"{}\">New</a>", p("/new")) };
            ok(page(
                "Home page of the comparison test site",
                &format!("<h1>Home</h1><a href=\"{}\">A</a> <a href=\"{}\">B</a> <a href=\"{}\">C</a> {extra}", p("/a"), p("/b"), p("/c")),
                "",
            ))
        }
        ("/a/", true) => ok(page("All about toads in the garden", &format!("<h1>A</h1><p>{TOADS}</p><a href=\"/\">Home</a>"), "")),
        ("/a", false) => ok(page("All about frogs by the pond", &format!("<h1>A</h1><p>{FROGS}</p><a href=\"/\">Home</a>"), "")),
        ("/b/", true) => ok(page("Page B of the comparison test", "<h1>B</h1><a href=\"/\">Home</a>", r#"<meta name="robots" content="noindex">"#)),
        ("/b", false) => ok(page("Page B of the comparison test", "<h1>B</h1><a href=\"/\">Home</a>", "")),
        ("/c/", true) => ok(page("Page C of the comparison test", "<h2>No H1 yet</h2><a href=\"/\">Home</a>", "")),
        ("/c", false) => ok(page("Page C of the comparison test", "<h1>C</h1><a href=\"/\">Home</a>", "")),
        ("/gone/", true) => ok(page("A page that is removed later on", "<h1>Gone</h1><a href=\"/\">Home</a>", "")),
        ("/new", false) => ok(page("A page added in the second crawl", "<h2>No H1</h2><a href=\"/\">Home</a>", "")),
        _ => (404, page("Not Found", "<h1>404</h1>", "")),
    }
}

async fn serve(listener: TcpListener, version: Arc<AtomicU8>) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let version = version.load(Ordering::SeqCst);
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, body) = respond(&path, version);
            let head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(start: &Url, db: &Path) {
    // Store HTML gives View Source and the content signature that Content
    // change detection needs.
    let cfg = CrawlConfig { store_html: true, ..CrawlConfig::default() };
    let mut store = Store::create(db, start, &cfg).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let crawler = Crawler::new(cfg, start).unwrap();
    let handle = tokio::spawn(crawler.run(vec![start.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    handle.await.unwrap().unwrap();
    store.insert_batch(&records).unwrap();
    store.analyse().unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn compare_two_crawls() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("http://127.0.0.1:{port}");
    let version = Arc::new(AtomicU8::new(1));
    tokio::spawn(serve(listener, version.clone()));
    let start = Url::parse(&format!("{origin}/")).unwrap();

    let dir = std::env::temp_dir().join(format!("toad-compare-{port}"));
    std::fs::create_dir_all(&dir).unwrap();
    let (prev_db, cur_db, cmp_db) = (dir.join("previous.toad"), dir.join("current.toad"), dir.join("comparison.toadcmp"));
    crawl(&start, &prev_db).await;
    version.store(2, Ordering::SeqCst);
    crawl(&start, &cur_db).await;

    let cfg = CompareConfig {
        // Strip the trailing slash from previous-crawl URLs, but not the root.
        url_mapping: vec![UrlMapping { regex: r"(:\d+/.+)/$".into(), replace: "$1".into() }],
        ..Default::default()
    };
    let cmp = Comparison::build(&cur_db, &prev_db, &cmp_db, &cfg).unwrap();
    let u = |p: &str| format!("{origin}{p}");

    // Every filter obeys Current = Previous + Added + New − Removed − Missing.
    let overview = cmp.overview().unwrap();
    for r in overview.iter().filter(|r| r.added.is_some()) {
        let rhs = r.previous as i64 + r.added.unwrap() as i64 + r.new.unwrap() as i64 - r.removed.unwrap() as i64 - r.missing.unwrap() as i64;
        assert_eq!(r.current as i64, rhs, "{} > {}", r.tab, r.filter);
    }
    let row = |tab: &str, filter: &str| overview.iter().find(|r| r.tab == tab && r.filter == filter).unwrap().clone();

    // Mapping lines up /, /a, /b and /c; /gone is missing and /new is new.
    let summary = cmp.summary().unwrap();
    assert_eq!((summary.current_urls, summary.previous_urls, summary.in_both), (5, 5, 4));
    let html = row("Internal", "HTML");
    assert_eq!((html.current, html.previous, html.new, html.missing), (5, 5, Some(1), Some(1)));

    // /b lost its noindex (Removed); /c gained an H1 (Removed) and /new has none (New).
    let noindex = row("Directives", "Noindex");
    assert_eq!((noindex.current, noindex.previous, noindex.removed), (0, 1, Some(1)));
    let missing_h1 = row("H1", "Missing");
    assert_eq!((missing_h1.current, missing_h1.previous, missing_h1.removed, missing_h1.new), (1, 1, Some(1), Some(1)));

    // Grid rows: Removed shows the previous crawl's original URL; New shows the current one.
    let q = |tab: &str, filter: &str| RowQuery { tab: tab.into(), filter: filter.into(), ..Default::default() };
    let removed = cmp.rows(&q("Directives", "Noindex"), CompareState::Removed).unwrap();
    assert_eq!(removed.rows.len(), 1);
    assert_eq!(removed.rows[0][0], u("/b/"));
    let new = cmp.rows(&q("H1", "Missing"), CompareState::New).unwrap();
    assert_eq!(new.rows.iter().map(|r| r[0].clone()).collect::<Vec<_>>(), vec![u("/new")]);
    let missing = cmp.rows(&q("Internal", "HTML"), CompareState::Missing).unwrap();
    assert_eq!(missing.rows[0][0], u("/gone/"));
    assert_eq!(cmp.rows(&q("Directives", "Noindex"), CompareState::Previous).unwrap().total, 1);

    // Change Detection over the four pages in both crawls.
    let cd = |e: ChangeElement| row("Change Detection", e.name()).current;
    assert_eq!(row("Change Detection", "All").current, 4);
    assert_eq!(cd(ChangeElement::PageTitles), 1, "/a");
    assert_eq!(cd(ChangeElement::Indexability), 1, "/b");
    assert_eq!(cd(ChangeElement::H1), 1, "/c");
    let content = cmp.change_rows(Some(ChangeElement::Content), true, &RowQuery::default()).unwrap();
    let mut changed: Vec<String> = content.rows.iter().map(|r| r[0].clone()).collect();
    changed.sort();
    assert_eq!(changed, vec![u("/"), u("/a")], "/a's text was replaced; the short home page swapped a link; /c is too short to shingle");
    assert_eq!(content.columns, vec!["Address", "Similarity Match %", "Change %", "Current Word Count", "Previous Word Count"]);
    assert!(summary.content_available);
    assert_eq!(cd(ChangeElement::CrawlDepth), 0);
    let titles = cmp.rows(&q("Change Detection", "Page Titles"), CompareState::Current).unwrap();
    assert_eq!(titles.columns, vec!["Address", "Current Page Title", "Previous Page Title"]);
    assert_eq!(titles.rows[0], vec![u("/a"), "All about frogs by the pond".to_string(), "All about toads in the garden".to_string()]);
    let words = cmp.change_rows(Some(ChangeElement::WordCount), true, &RowQuery::default()).unwrap();
    assert_eq!(&words.columns[3..], ["Change", "Change %"]);

    // Ignoring non-indexable URLs drops /b from everything but Indexability.
    let strict = Comparison::build(&cur_db, &prev_db, &dir.join("strict.toadcmp"), &CompareConfig { ignore_non_indexable: true, ..cfg.clone() }).unwrap();
    let strict_all = strict.change_detection_overview().unwrap();
    assert_eq!(strict_all[0].current, 3);

    // Lower window: URL details side by side, and link diffs with mapping.
    let details = cmp.url_details(&u("/a")).unwrap();
    let title = details.rows.iter().find(|r| r[0] == "title_1").unwrap();
    assert_eq!((title[1].as_str(), title[2].as_str()), ("All about frogs by the pond", "All about toads in the garden"));
    let added = cmp.links(&u("/"), true, LinkState::Added).unwrap();
    assert_eq!(added.rows.iter().map(|r| r[2].clone()).collect::<Vec<_>>(), vec![u("/new")]);
    let removed = cmp.links(&u("/"), true, LinkState::Removed).unwrap();
    assert_eq!(removed.rows.iter().map(|r| r[2].clone()).collect::<Vec<_>>(), vec![u("/gone")]);

    // View Source compare: /a's paragraph changed, line by line.
    assert!(cmp.current().page_source(&u("/a")).unwrap().unwrap().contains("Frogs have smooth"));
    let src = cmp.source_compare(&u("/a")).unwrap();
    assert!(src.previous.as_deref().unwrap().contains("Toads are amphibians"));
    assert!(src.changed > 0);
    assert!(src.lines.iter().any(|l| l.tag == "delete" && l.text.contains("Toads")));
    assert!(src.lines.iter().any(|l| l.tag == "insert" && l.text.contains("Frogs")));
    let missing_src = cmp.source_compare(&u("/gone/")).unwrap();
    assert!(missing_src.current.is_none() && missing_src.previous.is_some());
    let pages = dir.join("source");
    assert_eq!(cmp.current().export_page_source(&pages).unwrap(), 5);
    assert!(std::fs::read_to_string(pages.join("index.csv")).unwrap().contains(&u("/new")));

    // Issues carry both sides.
    let issues = cmp.issues().unwrap();
    let h1 = issues.iter().find(|i| i.tab == "H1" && i.filter == "Missing").unwrap();
    assert_eq!((h1.current, h1.previous, h1.removed, h1.new), (1, 1, 1, 1));

    // Site structure and exports.
    let tree = cmp.site_structure(3).unwrap();
    assert!(tree.rows.iter().any(|r| r[0] == "/"));
    let csv = dir.join("removed.csv");
    assert_eq!(cmp.export_csv(&q("Directives", "Noindex"), CompareState::Removed, &csv).unwrap(), 1);
    assert_eq!(cmp.overview_table().unwrap().rows.len(), overview.len());

    // The comparison reopens from its file without re-running.
    drop(cmp);
    let reopened = Comparison::open(&cmp_db).unwrap();
    assert_eq!(reopened.overview().unwrap().len(), overview.len());
    assert_eq!(reopened.config().url_mapping.len(), 1);
}
