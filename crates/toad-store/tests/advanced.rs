//! Spider > Advanced: cookie storage, always follow redirects/canonicals,
//! self-referencing meta refresh and Respect noindex, against a local site.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::CookieStorage;
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

fn page(body: &str, head: &str) -> String {
    format!("<!doctype html><html><head><title>A page with a long enough title</title>{head}</head><body>{body}</body></html>")
}

/// (status, extra headers, body) for a request path and its Cookie header.
fn respond(path: &str, cookie: &str) -> (u16, String, String) {
    let has = |name: &str| cookie.split(';').any(|c| c.trim().starts_with(&format!("{name}=")));
    match path {
        "/" => (200, String::new(), page(r#"<a href="/login">Login</a> <a href="/set">Set</a> <a href="/self">Self</a> <a href="/hidden">Hidden</a>"#, "")),
        // A redirect that sets a cookie the destination needs (Session Only).
        "/login" => (302, "Location: /account\r\nSet-Cookie: session=1; Path=/\r\n".into(), String::new()),
        "/account" if has("session") => (200, String::new(), page("Welcome back", "")),
        "/account" => (403, String::new(), page("Forbidden", "")),
        // A page that sets a cookie that a later, separate page needs (Persistent).
        "/set" => (200, "Set-Cookie: pref=1; Path=/\r\n".into(), page(r#"<a href="/needs">Needs</a>"#, "")),
        "/needs" if has("pref") => (200, String::new(), page("Has the cookie", "")),
        "/needs" => (403, String::new(), page("No cookie", "")),
        "/self" => (200, String::new(), page("Refreshes to itself", r#"<meta http-equiv="refresh" content="5; url=/self">"#)),
        "/hidden" => (200, String::new(), page("Noindex page", r#"<meta name="robots" content="noindex">"#)),
        "/old" => (301, "Location: /new\r\n".into(), String::new()),
        "/new" => (200, String::new(), page("Redirect target", "")),
        "/dup" => (200, String::new(), page("Canonicalised", r#"<link rel="canonical" href="/main">"#)),
        "/main" => (200, String::new(), page("Canonical target", "")),
        _ => (404, String::new(), page("Not found", "")),
    }
}

async fn serve(listener: TcpListener) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]).to_string();
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let cookie = req
                .lines()
                .find_map(|l| l.strip_prefix("cookie: ").or_else(|| l.strip_prefix("Cookie: ")))
                .unwrap_or("")
                .to_string();
            let (status, extra, body) = respond(&path, &cookie);
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\n{extra}Connection: close\r\n\r\n", body.len());
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn crawl(origin: &str, seeds: &[&str], cfg: CrawlConfig, name: &str) -> Store {
    let seeds: Vec<Url> = seeds.iter().map(|s| Url::parse(&format!("{origin}{s}")).unwrap()).collect();
    let db = std::env::temp_dir().join(format!("toad-advanced-{name}-{}.toad", std::process::id()));
    let mut store = Store::create(&db, &seeds[0], &cfg).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let run = tokio::spawn(Crawler::new(cfg, &seeds[0]).unwrap().run(seeds, tx, CrawlControl::default()));
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

fn field(store: &Store, url: &str, key: &str) -> String {
    store.url_details(url).unwrap().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread")]
async fn advanced_behaviours() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener));
    let u = |p: &str| format!("{origin}{p}");
    let base = CrawlConfig { threads: 1, ..CrawlConfig::default() };

    // Session Only (default): the redirect's cookie reaches /account, but
    // /set's cookie doesn't carry over to /needs.
    let session = crawl(&origin, &["/"], base.clone(), "session").await;
    assert_eq!(field(&session, &u("/account"), "status_code"), "200");
    assert_eq!(field(&session, &u("/needs"), "status_code"), "403");

    // Persistent: one jar for the crawl, so /needs gets /set's cookie.
    let persistent = crawl(&origin, &["/"], CrawlConfig { cookie_storage: CookieStorage::Persistent, ..base.clone() }, "persistent").await;
    assert_eq!(field(&persistent, &u("/needs"), "status_code"), "200");
    assert_eq!(field(&persistent, &u("/account"), "status_code"), "200");

    // Do Not Store: no cookies at all.
    let none = crawl(&origin, &["/"], CrawlConfig { cookie_storage: CookieStorage::DoNotStore, ..base.clone() }, "none").await;
    assert_eq!(field(&none, &u("/account"), "status_code"), "403");

    // Self-referencing meta refresh makes the page Non-Indexable.
    assert_eq!(field(&session, &u("/self"), "indexability_status"), "Self Referencing Meta Refresh");

    // Respect noindex: crawled, but hidden from the tabs.
    let all_before = session.count("Internal", "All").unwrap();
    let respect = crawl(&origin, &["/"], CrawlConfig { respect_noindex: true, ..base.clone() }, "respect").await;
    assert_eq!(respect.url_count().unwrap(), session.url_count().unwrap());
    assert_eq!(respect.count("Internal", "All").unwrap(), all_before - 1, "/hidden is hidden");
    assert_eq!(respect.count("Directives", "Noindex").unwrap(), 0);

    // List mode (depth 0): redirects and canonicals are only followed when asked.
    let list = base.clone().for_list_mode();
    let plain = crawl(&origin, &["/old", "/dup"], list.clone(), "list").await;
    assert_eq!(field(&plain, &u("/new"), "status_code"), "", "not followed at depth 0");
    assert_eq!(field(&plain, &u("/main"), "status_code"), "");
    let follow = crawl(&origin, &["/old", "/dup"], CrawlConfig { always_follow_redirects: true, always_follow_canonicals: true, ..list }, "follow").await;
    assert_eq!(field(&follow, &u("/new"), "status_code"), "200");
    assert_eq!(field(&follow, &u("/main"), "status_code"), "200");
}
