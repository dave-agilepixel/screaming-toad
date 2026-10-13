//! Spider > Rendering > JavaScript end to end: the rendered HTML is what's
//! extracted, the original HTML's elements are kept, links found only after
//! rendering are crawled with their Link Origin, console errors, blocked
//! resources and screenshots are stored.
//!
//! Needs Chrome, Edge or Chromium; skipped when none is found. Set
//! TOAD_TEST_BROWSER to a browser executable to choose one.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::{RenderMode, RenderingConfig};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

const HOME: &str = r#"<html><head><title>Original</title><script src="/blocked.js"></script></head>
<body><a href="/plain">Plain</a><p>One two three.</p><img src="/toad.png">
<script>
  document.title = 'Rendered';
  const h = document.createElement('h1'); h.textContent = 'Made by JS'; document.body.appendChild(h);
  const a = document.createElement('a'); a.href = '/js-link'; a.textContent = 'JS link'; document.body.appendChild(a);
  console.error('boom');
</script></body></html>"#;

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (ctype, body) = match path.as_str() {
                "/" => ("text/html", HOME.to_string()),
                "/robots.txt" => ("text/plain", "User-agent: *\nDisallow: /blocked.js\n".to_string()),
                "/blocked.js" => ("application/javascript", "window.blockedRan = true;".to_string()),
                p => ("text/html", format!("<html><head><title>{p}</title></head><body>Hi</body></html>")),
            };
            let head = format!("HTTP/1.1 200 OK\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn javascript_rendering() {
    let mut rendering = RenderingConfig {
        mode: RenderMode::JavaScript,
        js_error_reporting: true,
        store_rendered_html: true,
        accessibility: true,
        ajax_timeout_secs: 0.5,
        resize_to_content: false,
        ..RenderingConfig::default()
    };
    if let Ok(p) = std::env::var("TOAD_TEST_BROWSER") {
        rendering.browser_path = Some(p);
    }
    if toad_core::render::find_browser(&rendering).is_none() {
        eprintln!("skipping: no Chrome, Edge or Chromium found");
        return;
    }

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let start = Url::parse(&format!("{origin}/")).unwrap();
    let cfg = CrawlConfig { threads: 2, rendering, ..CrawlConfig::default() };
    let dir = std::env::temp_dir().join(format!("toad-render-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let _ = std::fs::remove_file(&db);
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
    let field = |url: &str, key: &str| store.url_details(url).unwrap().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default();
    assert_eq!(field(&home, "render_error"), "");
    assert_eq!(field(&home, "rendered"), "1");
    assert_eq!(field(&home, "title_1"), "Rendered", "extraction uses the rendered HTML");
    assert_eq!(field(&home, "orig_title"), "Original");
    assert_eq!(field(&home, "h1_1"), "Made by JS");
    assert_eq!(field(&home, "orig_h1"), "");
    assert_eq!(field(&home, "js_link_count"), "1");
    assert_eq!(field(&home, "js_errors"), "1");
    assert_eq!(field(&home, "blocked_resources"), "1");
    assert!(!field(&home, "carbon_rating").is_empty());

    // The JS-inserted link was followed, with its origin.
    let js_link = format!("{origin}/js-link");
    assert_eq!(field(&js_link, "status_code"), "200");
    let inlinks = store.inlinks(&js_link).unwrap();
    assert_eq!(inlinks.rows[0][13], "Rendered HTML");
    let plain = store.inlinks(&format!("{origin}/plain")).unwrap();
    assert_eq!(plain.rows[0][13], "HTML & Rendered HTML");

    let console = store.console_log(&home).unwrap();
    assert!(console.rows.iter().any(|r| r[0] == "error" && r[2].contains("boom")), "{:?}", console.rows);
    assert_eq!(store.blocked_resources(&home).unwrap().rows[0][0], format!("{origin}/blocked.js"));
    let png = store.screenshot(&home).unwrap().expect("a screenshot");
    assert!(png.starts_with(b"\x89PNG"));
    assert!(store.rendered_source(&home).unwrap().unwrap().contains("Made by JS"));

    // JavaScript tab.
    let count = |filter: &str| store.count("JavaScript", filter).unwrap();
    assert_eq!(count("Page Title Updated by JavaScript"), 1);
    assert_eq!(count("H1 Only in Rendered HTML"), 1);
    assert_eq!(count("Contains JavaScript Links"), 1);
    assert_eq!(count("Contains JavaScript Content"), 1);
    assert_eq!(count("Pages with JavaScript Errors"), 1);
    assert_eq!(count("Pages with Blocked Resources"), 1);
    assert_eq!(count("Page Title Only in Rendered HTML"), 0);
    assert_eq!(count("Canonical Mismatch"), 0);
    assert_eq!(count("Noindex Only in Original HTML"), 0);

    // Accessibility (axe-core): no lang on <html>, an image without alt.
    let a11y = store.accessibility_details(&home).unwrap();
    assert!(a11y.rows.iter().any(|r| r[2] == "image-alt" && r[1] == "critical" && r[0] == "WCAG 2.0 A"), "{:?}", a11y.rows);
    assert!(a11y.rows.iter().any(|r| r[2] == "html-has-lang"), "{:?}", a11y.rows);
    // Every test page lacks lang, so all three crawled pages fail level A.
    assert_eq!(store.count("Accessibility", "WCAG 2.0 A Violations").unwrap(), 3);
    assert_eq!(store.count("Accessibility", "Critical Impact").unwrap(), 1);
    let summary = store.report("Accessibility Violations Summary").unwrap();
    assert!(summary.rows.iter().any(|r| r[1] == "image-alt"));
}
