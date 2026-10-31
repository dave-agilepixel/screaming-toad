//! Checks that need JavaScript rendering: Redirection (JavaScript),
//! Blocked Resource, Pages with JavaScript Warnings, Pages with Chrome
//! Issues, and the Mobile tab's layout checks.
//!
//! Needs Chrome, Edge or Chromium; skipped when none is found. Set
//! TOAD_TEST_BROWSER to a browser executable to choose one.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::{RenderMode, RenderingConfig};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, Store};
use url::Url;

const VIEWPORT: &str = r#"<meta name="viewport" content="width=device-width, initial-scale=1">"#;

fn page(title: &str, body: &str) -> String {
    format!("<!doctype html><html lang=\"en\"><head><title>{title}</title>{VIEWPORT}</head><body>{body}</body></html>")
}

fn respond(path: &str) -> (u16, &'static str, String) {
    match path {
        "/robots.txt" => (200, "text/plain", "User-agent: *\nDisallow: /blocked/\n".into()),
        "/" => (200, "text/html", page(
            "Home page of the rendering checks site",
            r#"<h1>Home</h1><a href="/js-redirect">JS redirect</a> <a href="/wide">Wide</a> <a href="/tiny">Tiny</a>
               <div><button style="width:10px;height:10px;padding:0;margin:0;border:0">1</button><button style="width:10px;height:10px;padding:0;margin:0;border:0">2</button></div>
               <form><label for="nothing-has-this-id">Name</label><input id="name" name="name"></form>
               <script src="/blocked/app.js"></script>
               <script>console.warn('careful');</script>"#,
        )),
        "/js-redirect" => (200, "text/html", page("Sends the browser on with JavaScript", "<h1>Moving</h1><script>location.replace('/landing');</script>")),
        "/landing" => (200, "text/html", page("Where the JavaScript redirect lands", "<h1>Landing</h1>")),
        "/wide" => (200, "text/html", page("Wider than a phone", r#"<h1>Wide</h1><div style="width:3000px">A very wide block of content.</div>"#)),
        "/tiny" => (200, "text/html", page(
            "Text too small to read",
            r#"<h1 style="font-size:20px">Tiny</h1><p style="font-size:9px">Most of this page is set in very small type that is hard to read on a phone, which is what the illegible font size check looks for.</p>"#,
        )),
        "/blocked/app.js" => (200, "application/javascript", "window.blocked = true;".into()),
        _ => (404, "text/html", page("Not found", "")),
    }
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 4096];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let path = String::from_utf8_lossy(&buf[..n]).split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, ctype, body) = respond(&path);
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

fn addresses(store: &Store, tab: &str, filter: &str) -> Vec<String> {
    let t = store.rows(&RowQuery { tab: tab.into(), filter: filter.into(), ..Default::default() }).unwrap();
    let col = t.columns.iter().position(|c| c == "Address").unwrap();
    let mut v: Vec<String> = t.rows.iter().map(|r| r[col].clone()).collect();
    v.sort();
    v
}

#[tokio::test(flavor = "multi_thread")]
async fn rendering_checks() {
    let mut rendering = RenderingConfig {
        mode: RenderMode::JavaScript,
        js_error_reporting: true,
        accessibility: true,
        screenshots: false,
        ajax_timeout_secs: 1.0,
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
    let u = |p: &str| format!("{origin}{p}");
    let start = Url::parse(&u("/")).unwrap();
    let cfg = CrawlConfig { threads: 2, rendering, ..CrawlConfig::default() };
    let db = std::env::temp_dir().join(format!("toad-rendering-checks-{}.toad", std::process::id()));
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

    // JavaScript redirect: reported, and its target crawled.
    assert_eq!(addresses(&store, "Response Codes", "Internal Redirection (JavaScript)"), vec![u("/js-redirect")]);
    assert_eq!(addresses(&store, "Response Codes", "Redirection (JavaScript)"), vec![u("/js-redirect")]);
    assert!(addresses(&store, "Response Codes", "Internal Success (2xx)").contains(&u("/landing")));

    // The script robots.txt blocks is a URL of its own.
    assert_eq!(addresses(&store, "Response Codes", "Internal Blocked Resource"), vec![u("/blocked/app.js")]);
    assert_eq!(store.count("Response Codes", "External Blocked Resource").unwrap(), 0);
    assert_eq!(addresses(&store, "JavaScript", "Pages with Blocked Resources"), vec![u("/")]);

    // Console warnings and DevTools issues (the label points at no id).
    assert_eq!(addresses(&store, "JavaScript", "Pages with JavaScript Warnings"), vec![u("/")]);
    assert_eq!(addresses(&store, "JavaScript", "Pages with Chrome Issues"), vec![u("/")]);

    // Mobile layout, in the default Googlebot Smartphone viewport.
    assert_eq!(addresses(&store, "Mobile", "Content Not Sized Correctly"), vec![u("/wide")]);
    assert_eq!(addresses(&store, "Mobile", "Illegible Font Size"), vec![u("/tiny")]);
    assert_eq!(addresses(&store, "Mobile", "Target Size"), vec![u("/")]);
    assert_eq!(store.count("Mobile", "Viewport Not Set").unwrap(), 0);
}
