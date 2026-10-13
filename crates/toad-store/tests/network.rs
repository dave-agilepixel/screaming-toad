//! Configuration > Authentication and File > Settings > Proxy end to end:
//! Basic and Digest logins (stored and prompted), forms-based login cookies,
//! and crawling through an authenticating HTTP proxy.

use base64::Engine;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::auth::{AuthConfig, AuthCookie, Credential, FormLogin, Network, NetworkSettings, ProxyConfig};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::Store;
use url::Url;

struct Request {
    target: String,
    headers: HashMap<String, String>,
}

type Handler = Arc<dyn Fn(&Request) -> (u16, Vec<(String, String)>, String) + Send + Sync>;

async fn serve(listener: TcpListener, handler: Handler) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let handler = handler.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let text = String::from_utf8_lossy(&buf[..n]).to_string();
            let mut lines = text.split("\r\n");
            let target = lines.next().unwrap_or("").split_whitespace().nth(1).unwrap_or("/").to_string();
            let headers = lines
                .take_while(|l| !l.is_empty())
                .filter_map(|l| l.split_once(':'))
                .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
                .collect();
            let (status, extra, body) = handler(&Request { target, headers });
            let mut head = format!("HTTP/1.1 {status} X\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
            for (k, v) in extra {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

async fn start(handler: Handler) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
    tokio::spawn(serve(listener, handler));
    origin
}

const HOME: &str = r#"<html><head><title>Home</title></head><body><a href="/a">A</a><a href="/b">B</a></body></html>"#;
const PAGE: &str = "<html><head><title>Page</title></head><body>Hi</body></html>";

fn page_for(path: &str) -> String {
    if path == "/" { HOME.into() } else { PAGE.into() }
}

async fn crawl(start: &str, network: Network, name: &str) -> Store {
    let start = Url::parse(start).unwrap();
    let cfg = CrawlConfig { threads: 2, ..CrawlConfig::default() };
    let dir = std::env::temp_dir().join(format!("toad-network-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let _ = std::fs::remove_file(&db);
    let mut store = Store::create(&db, &start, &cfg).unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let crawler = Crawler::new(cfg, &start).unwrap().with_network(network.shared()).unwrap();
    let run = tokio::spawn(crawler.run(vec![start.clone()], tx, CrawlControl::default()));
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

fn status(store: &Store, url: &str) -> String {
    store.url_details(url).unwrap().into_iter().find(|(k, _)| k == "status_code").map(|(_, v)| v).unwrap_or_default()
}

fn basic_site(challenges: Arc<AtomicUsize>) -> Handler {
    let expected = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode("toad:secret"));
    Arc::new(move |r: &Request| {
        if r.headers.get("authorization") == Some(&expected) {
            (200, vec![], page_for(&r.target))
        } else {
            challenges.fetch_add(1, Ordering::SeqCst);
            (401, vec![("WWW-Authenticate".into(), r#"Basic realm="Staging""#.into())], "no".into())
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn basic_auth_with_stored_credentials() {
    let challenges = Arc::new(AtomicUsize::new(0));
    let origin = start(basic_site(challenges.clone())).await;

    let none = crawl(&format!("{origin}/"), Network::default(), "basic-none").await;
    assert_eq!(status(&none, &format!("{origin}/")), "401");

    challenges.store(0, Ordering::SeqCst);
    let auth = AuthConfig { standards: vec![Credential { url: "127.0.0.1".into(), username: "toad".into(), password: "secret".into() }], ..Default::default() };
    let store = crawl(&format!("{origin}/"), Network::new(NetworkSettings::default(), auth, None), "basic").await;
    for p in ["/", "/a", "/b"] {
        assert_eq!(status(&store, &format!("{origin}{p}")), "200", "{p}");
    }
    // After the first challenge, credentials are sent up front.
    assert_eq!(challenges.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn digest_auth() {
    let site: Handler = Arc::new(|r: &Request| {
        let challenge = (401, vec![("WWW-Authenticate".into(), r#"Digest realm="Toads", qop="auth", nonce="abc123", opaque="xyz""#.into())], "no".into());
        let Some(h) = r.headers.get("authorization") else { return challenge };
        let p = |k: &str| {
            let at = h.find(&format!("{k}=")).unwrap() + k.len() + 1;
            let rest = &h[at..];
            if let Some(q) = rest.strip_prefix('"') { q[..q.find('"').unwrap()].to_string() } else { rest.split(',').next().unwrap().to_string() }
        };
        let ha1 = format!("{:x}", md5::compute("toad:Toads:secret"));
        let ha2 = format!("{:x}", md5::compute(format!("GET:{}", p("uri"))));
        let want = format!("{:x}", md5::compute(format!("{ha1}:abc123:{}:{}:auth:{ha2}", p("nc"), p("cnonce"))));
        if p("response") == want && p("opaque") == "xyz" { (200, vec![], page_for(&r.target)) } else { challenge }
    });
    let origin = start(site).await;
    let auth = AuthConfig { standards: vec![Credential { url: origin.clone(), username: "toad".into(), password: "secret".into() }], ..Default::default() };
    let store = crawl(&format!("{origin}/"), Network::new(NetworkSettings::default(), auth, None), "digest").await;
    for p in ["/", "/a", "/b"] {
        assert_eq!(status(&store, &format!("{origin}{p}")), "200", "{p}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn prompts_once_per_realm() {
    let origin = start(basic_site(Arc::new(AtomicUsize::new(0)))).await;
    let (tx, mut rx) = tokio::sync::mpsc::channel::<toad_core::auth::AuthPrompt>(4);
    let prompts = Arc::new(AtomicUsize::new(0));
    let counted = prompts.clone();
    tokio::spawn(async move {
        while let Some(p) = rx.recv().await {
            assert_eq!(p.challenge.realm, "Staging");
            counted.fetch_add(1, Ordering::SeqCst);
            let _ = p.reply.send(Some(("toad".into(), "secret".into())));
        }
    });
    let store = crawl(&format!("{origin}/"), Network::new(NetworkSettings::default(), AuthConfig::default(), Some(tx)), "prompt").await;
    for p in ["/", "/a", "/b"] {
        assert_eq!(status(&store, &format!("{origin}{p}")), "200", "{p}");
    }
    assert_eq!(prompts.load(Ordering::SeqCst), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn forms_login_cookies() {
    let site: Handler = Arc::new(|r: &Request| {
        let logged_in = r.headers.get("cookie").is_some_and(|c| c.split("; ").any(|p| p == "session=abc"));
        if logged_in { (200, vec![], page_for(&r.target)) } else { (302, vec![("Location".into(), "/login".into())], String::new()) }
    });
    let origin = start(site).await;
    let auth = AuthConfig {
        forms: vec![FormLogin {
            name: "Site".into(),
            login_url: format!("{origin}/login"),
            cookies: vec![AuthCookie { name: "session".into(), value: "abc".into(), domain: "127.0.0.1".into(), path: "/".into(), host_only: true, ..Default::default() }],
        }],
        ..Default::default()
    };
    let store = crawl(&format!("{origin}/"), Network::new(NetworkSettings::default(), auth, None), "forms").await;
    for p in ["/", "/a", "/b"] {
        assert_eq!(status(&store, &format!("{origin}{p}")), "200", "{p}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn crawls_through_an_authenticating_proxy() {
    // The proxy answers absolute-form requests itself; the site never exists.
    let seen = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let log = seen.clone();
    let expected = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode("bob:pw"));
    let proxy: Handler = Arc::new(move |r: &Request| {
        if r.headers.get("proxy-authorization") != Some(&expected) {
            return (407, vec![("Proxy-Authenticate".into(), r#"Basic realm="proxy""#.into())], String::new());
        }
        log.lock().unwrap().push(r.target.clone());
        let path = Url::parse(&r.target).map(|u| u.path().to_string()).unwrap_or_default();
        (200, vec![], if path == "/" { HOME.into() } else { PAGE.into() })
    });
    let origin = start(proxy).await;
    let port: u16 = origin.rsplit(':').next().unwrap().parse().unwrap();
    let settings = NetworkSettings {
        proxy: ProxyConfig { enabled: true, host: "127.0.0.1".into(), port, username: "bob".into(), password: "pw".into(), ..Default::default() },
        ..Default::default()
    };
    let store = crawl("http://toad-site.invalid/", Network::new(settings, AuthConfig::default(), None), "proxy").await;
    assert_eq!(status(&store, "http://toad-site.invalid/"), "200");
    assert_eq!(status(&store, "http://toad-site.invalid/a"), "200");
    assert!(seen.lock().unwrap().iter().any(|t| t == "http://toad-site.invalid/b"));
}

#[test]
fn trusted_certificate_files() {
    let dir = std::env::temp_dir().join(format!("toad-certs-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("bad.pem");
    std::fs::write(&bad, "not a certificate").unwrap();
    let cfg = CrawlConfig::default();
    let net = |paths: Vec<String>| Network::new(NetworkSettings { trusted_certificates: paths, ..Default::default() }, AuthConfig::default(), None).shared();
    let err = toad_core::fetch::Fetcher::with_network(&cfg, net(vec![bad.display().to_string()])).err().expect("a bad file is an error");
    assert!(err.to_string().contains("bad.pem"), "{err}");
    let missing = toad_core::fetch::Fetcher::with_network(&cfg, net(vec![dir.join("none.pem").display().to_string()])).err().unwrap();
    assert!(missing.to_string().contains("none.pem"));
    let good = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/test-ca.pem");
    assert!(toad_core::fetch::Fetcher::with_network(&cfg, net(vec![good.to_string()])).is_ok());
}
