//! API Access end to end against a mock of every provider: Search Console
//! and GA4 before the crawl (with an orphan URL crawled because Search
//! Console knows it), then PSI, URL Inspection, AI prompts, embeddings and
//! link metrics over the crawled pages, and the tabs that show them.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_apis::{keys, ApiRunner, Endpoints, Secrets};
use toad_core::api_config::{AiPrompt, AiProvider, ApiConfig};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, Store};
use url::Url;

struct Req {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Value,
}

type Handler = Arc<dyn Fn(&Req) -> (u16, String, String) + Send + Sync>;

async fn read_request(sock: &mut tokio::net::TcpStream) -> Option<Req> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    let header_end = loop {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    let method = first.next()?.to_string();
    let target = first.next()?.to_string();
    let headers: HashMap<String, String> =
        lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    while buf.len() < header_end + len {
        let n = sock.read(&mut chunk).await.ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body_bytes = &buf[header_end..(header_end + len).min(buf.len())];
    let u = Url::parse(&format!("http://x{target}")).ok()?;
    let query = u.query_pairs().map(|(k, v)| (k.into_owned(), v.into_owned())).collect();
    let body = serde_json::from_slice(body_bytes).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(body_bytes).into_owned()));
    Some(Req { method, path: u.path().to_string(), query, headers, body })
}

async fn serve(listener: TcpListener, handler: Handler) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let handler = handler.clone();
        tokio::spawn(async move {
            let Some(req) = read_request(&mut sock).await else { return };
            let (status, ctype, body) = handler(&req);
            let head = format!("HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
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

const TOADS: &str = "Toads are amphibians living in damp gardens near ponds, eating slugs and beetles at night.";

fn site() -> Handler {
    Arc::new(|r: &Req| {
        let page = |title: &str, body: &str| format!("<!doctype html><html lang=\"en\"><head><title>{title}</title></head><body>{body}</body></html>");
        let html = match r.path.as_str() {
            "/" => page("Home", r#"<a href="/a">A</a> <a href="/b">B</a><p>Welcome to the pond.</p>"#),
            "/a" => page("A", &format!("<p>{TOADS} Common toads.</p>")),
            "/b" => page("B", &format!("<p>{TOADS} Natterjack toads.</p>")),
            // Not linked from anywhere: only Search Console knows it.
            "/orphan" => page("Orphan", "<p>Quarterly tax filing deadlines for small businesses.</p>"),
            _ => return (404, "text/html".into(), page("Not found", "")),
        };
        (200, "text/html".into(), html)
    })
}

/// A crude embedding: word counts over a small vocabulary.
fn embed(text: &str) -> Vec<f64> {
    let vocab = ["toads", "amphibians", "ponds", "gardens", "slugs", "tax", "businesses", "deadlines", "welcome", "pond"];
    let words: Vec<String> = text.to_lowercase().split(|c: char| !c.is_alphanumeric()).map(str::to_string).collect();
    vocab.iter().map(|v| words.iter().filter(|w| w == v).count() as f64 + 0.01).collect()
}

fn providers(site_origin: String, seen: Arc<Mutex<Vec<String>>>) -> Handler {
    Arc::new(move |r: &Req| {
        seen.lock().unwrap().push(format!("{} {}", r.method, r.path));
        let ok = |v: Value| (200, "application/json".to_string(), v.to_string());
        let p = r.path.as_str();
        match p {
            "/psi" => {
                assert_eq!(r.query.get("key").map(String::as_str), Some("psi-key"));
                let slow = r.query.get("url").is_some_and(|u| u.ends_with("/a"));
                ok(json!({
                    "loadingExperience": {"metrics": {
                        "LARGEST_CONTENTFUL_PAINT_MS": {"percentile": if slow { 4000 } else { 1500 }},
                        "INTERACTION_TO_NEXT_PAINT": {"percentile": 120},
                        "CUMULATIVE_LAYOUT_SHIFT_SCORE": {"percentile": 2}
                    }},
                    "lighthouseResult": {
                        "categories": {"performance": {"score": if slow { 0.35 } else { 0.95 }}},
                        "audits": {"render-blocking-insight": {"score": if slow { 0 } else { 1 }, "metricSavings": {"FCP": 900}}}
                    }
                }))
            }
            "/token" => {
                assert!(r.body.as_str().is_some_and(|b| b.contains("grant_type=refresh_token") && b.contains("refresh_token=rt")));
                ok(json!({"access_token": "at", "expires_in": 3600}))
            }
            _ if p.starts_with("/gsc/sites/") && p.ends_with("/searchAnalytics/query") => {
                assert_eq!(r.headers.get("authorization").map(String::as_str), Some("Bearer at"));
                assert_eq!(r.body["dimensions"][0], "page");
                assert_eq!(r.body["dimensionFilterGroups"][0]["filters"][0]["expression"], "MOBILE");
                ok(json!({"rows": [
                    {"keys": [format!("{site_origin}/a/")], "clicks": 12, "impressions": 300, "ctr": 0.04, "position": 4.2},
                    {"keys": [format!("{site_origin}/orphan")], "clicks": 3, "impressions": 90, "ctr": 0.033, "position": 9.1}
                ]}))
            }
            "/inspect" => {
                let u = r.body["inspectionUrl"].as_str().unwrap_or("");
                let verdict = if u.ends_with("/b") { "NEUTRAL" } else { "PASS" };
                ok(json!({"inspectionResult": {"indexStatusResult": {"verdict": verdict, "coverageState": "x", "googleCanonical": u, "userCanonical": u}}}))
            }
            "/ga4/properties/123:runReport" => {
                assert_eq!(r.body["metrics"][0]["name"], "sessions");
                let host = site_origin.trim_start_matches("http://");
                ok(json!({
                    "metricHeaders": [{"name": "sessions"}, {"name": "bounceRate"}],
                    "rows": [
                        {"dimensionValues": [{"value": host}, {"value": "/b"}], "metricValues": [{"value": "40"}, {"value": "0.8"}]},
                        {"dimensionValues": [{"value": "other-site.com"}, {"value": "/b"}], "metricValues": [{"value": "999"}, {"value": "0.1"}]}
                    ],
                    "rowCount": 2
                }))
            }
            "/openai/chat/completions" => {
                assert_eq!(r.headers.get("authorization").map(String::as_str), Some("Bearer sk-test"));
                let text = r.body["messages"][0]["content"].as_str().unwrap_or("");
                let topic = if text.contains("Toads") { "toads" } else { "other" };
                ok(json!({"choices": [{"message": {"content": format!("Topic: {topic}")}}]}))
            }
            "/openai/embeddings" => {
                let v = embed(r.body["input"].as_str().unwrap_or(""));
                ok(json!({"data": [{"embedding": v}]}))
            }
            "/ahrefs/site-explorer/backlinks-stats" => ok(json!({"metrics": {"live": 120, "live_refdomains": 30}})),
            "/ahrefs/site-explorer/domain-rating" => ok(json!({"domain_rating": {"domain_rating": 55.0}})),
            "/majestic" => {
                let n: usize = r.query.get("items").and_then(|v| v.parse().ok()).unwrap_or(0);
                let data: Vec<Value> = (0..n).map(|_| json!({"TrustFlow": 21, "CitationFlow": 33, "ExtBackLinks": 80, "RefDomains": 12})).collect();
                ok(json!({"Code": "OK", "DataTables": {"Results": {"Data": data}}}))
            }
            "/moz/url_metrics" => {
                let n = r.body["targets"].as_array().map_or(0, |a| a.len());
                let rows: Vec<Value> = (0..n).map(|_| json!({"page_authority": 31, "domain_authority": 44, "spam_score": 2})).collect();
                ok(json!({"results": rows}))
            }
            _ => (404, "application/json".into(), json!({"error": {"message": format!("no mock for {p}")}}).to_string()),
        }
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn api_access_end_to_end() {
    let origin = start(site()).await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let api = start(providers(origin.clone(), seen.clone())).await;

    let mut apis = ApiConfig::default();
    apis.psi.enabled = true;
    apis.gsc.enabled = true;
    apis.gsc.property = "sc-domain:example.com".into();
    apis.gsc.device = toad_core::api_config::GscDevice::Mobile;
    apis.gsc.crawl_new_urls = true;
    apis.gsc.url_inspection = true;
    apis.ga4.enabled = true;
    apis.ga4.property = "123".into();
    apis.ga4.metrics = vec!["sessions".into(), "bounceRate".into()];
    apis.ai.prompts = vec![AiPrompt { name: "Topic".into(), provider: AiProvider::OpenAi, model: "m".into(), prompt: "What is this page about?".into(), ..Default::default() }];
    apis.ai.requests_per_minute = 0;
    apis.ai.indexable_only = false;
    apis.embeddings.enabled = true;
    apis.embeddings.similarity_threshold = 0.9;
    apis.embeddings.low_relevance_threshold = 0.5;
    apis.ahrefs.enabled = true;
    apis.majestic.enabled = true;
    apis.moz.enabled = true;
    let cfg = CrawlConfig { threads: 2, apis, ..CrawlConfig::default() };

    let secrets = Secrets::new(
        [
            (keys::PSI, "psi-key"),
            (keys::GOOGLE_CLIENT_ID, "cid"),
            (keys::GOOGLE_CLIENT_SECRET, "cs"),
            (keys::GOOGLE_REFRESH_TOKEN, "rt"),
            (keys::OPENAI, "sk-test"),
            (keys::AHREFS, "ah"),
            (keys::MAJESTIC, "mj"),
            (keys::MOZ, "id:secret"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    );
    let endpoints = Endpoints {
        psi: format!("{api}/psi"),
        google_token: format!("{api}/token"),
        gsc: format!("{api}/gsc"),
        inspection: format!("{api}/inspect"),
        ga4_data: format!("{api}/ga4"),
        ga4_admin: format!("{api}/ga4admin"),
        ahrefs: format!("{api}/ahrefs"),
        majestic: format!("{api}/majestic"),
        moz: format!("{api}/moz"),
        openai: Some(format!("{api}/openai")),
        ..Endpoints::default()
    };
    let start_url = Url::parse(&format!("{origin}/")).unwrap();
    let runner = ApiRunner::new(cfg.clone(), secrets).with_endpoints(endpoints).with_site(start_url.clone());

    // Before the crawl: Search Console and GA4.
    let bulk = runner.fetch_bulk().await;
    assert!(bulk.errors.is_empty(), "{:?}", bulk.errors);
    let orphan = format!("{origin}/orphan");
    assert!(bulk.new_urls.iter().any(|u| u.as_str() == orphan), "{:?}", bulk.new_urls);

    let dir = std::env::temp_dir().join(format!("toad-apis-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let _ = std::fs::remove_file(&db);
    let store = Store::create(&db, &start_url, &cfg).unwrap();
    let store = Mutex::new(store);
    runner.store_bulk(&store, &bulk).unwrap();

    let mut seeds = vec![start_url.clone()];
    seeds.extend(bulk.new_urls.iter().cloned());
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg.clone(), &start_url).unwrap().run(seeds, tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    {
        let mut s = store.lock().unwrap();
        s.insert_batch(&records).unwrap();
        s.analyse().unwrap();
    }

    // After the crawl: per-URL sources.
    let errors = runner.fetch_per_url(&store).await;
    assert!(errors.is_empty(), "{errors:?}");
    let s = store.into_inner().unwrap();

    let rows = |tab: &str, filter: &str| s.rows(&RowQuery { tab: tab.into(), filter: filter.into(), ..Default::default() }).unwrap();
    let cell = |tab: &str, path: &str, col: &str| {
        let t = rows(tab, "All");
        let c = t.columns.iter().position(|x| x == col).unwrap_or_else(|| panic!("{col} in {:?}", t.columns));
        t.rows.iter().find(|r| r[0] == format!("{origin}{path}")).map(|r| r[c].clone()).unwrap_or_else(|| panic!("{path} not in {tab}"))
    };
    let count = |tab: &str, filter: &str| s.count(tab, filter).unwrap();

    // PageSpeed.
    assert_eq!(cell("PageSpeed", "/a", "Performance Score"), "35");
    assert_eq!(cell("PageSpeed", "/a", "Core Web Vitals Assessment"), "Failed");
    assert_eq!(cell("PageSpeed", "/b", "Core Web Vitals Assessment"), "Passed");
    assert_eq!(count("PageSpeed", "Render Blocking Requests"), 1);
    assert_eq!(cell("PageSpeed", "/a", "Render Blocking Requests Savings (ms)"), "900");

    // Search Console: /a/ matches /a (trailing slash ignored); the orphan was crawled.
    assert_eq!(cell("Search Console", "/a", "Clicks"), "12");
    assert_eq!(count("Search Console", "Clicks Above 0"), 2);
    assert_eq!(count("Search Console", "Orphan URLs"), 1);
    assert_eq!(rows("Search Console", "Orphan URLs").rows[0][0], orphan);
    assert_eq!(count("Search Console", "URL Is Not on Google"), 1);
    assert_eq!(cell("Search Console", "/b", "Summary"), "NEUTRAL");

    // GA4: only this site's rows, with the crawl's scheme.
    assert_eq!(cell("Analytics", "/b", "GA4 sessions"), "40");
    assert_eq!(count("Analytics", "Bounce Rate Above 70%"), 1);
    assert_eq!(count("Analytics", "Sessions Above 0"), 1);

    // AI prompt per page.
    assert_eq!(cell("AI", "/a", "Topic"), "Topic: toads");
    assert_eq!(cell("AI", "/orphan", "Topic"), "Topic: other");

    // Embeddings: /a and /b are semantically similar; the orphan is off-topic.
    assert_eq!(count("Content", "Semantically Similar"), 2);
    assert_eq!(cell("Content", "/a", "Closest Semantically Similar Address"), format!("{origin}/b"));
    assert!(rows("Content", "Low Relevance Content").rows.iter().any(|r| r[0] == orphan));

    // Link metrics.
    assert_eq!(cell("Link Metrics", "/a", "Ahrefs Backlinks"), "120");
    assert_eq!(cell("Link Metrics", "/a", "Ahrefs Domain Rating"), "55");
    assert_eq!(cell("Link Metrics", "/b", "Majestic Trust Flow"), "21");
    assert_eq!(cell("Link Metrics", "/", "Moz Domain Authority"), "44");

    // Request API Data again: nothing is requested twice.
    let before = seen.lock().unwrap().len();
    let store = Mutex::new(s);
    assert!(runner.fetch_per_url(&store).await.is_empty());
    let again: Vec<String> = seen.lock().unwrap()[before..].to_vec();
    assert!(again.iter().all(|r| !r.contains("/psi") && !r.contains("/inspect") && !r.contains("/openai")), "{again:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn failed_requests_are_recorded() {
    let origin = start(site()).await;
    let api = start(Arc::new(|_r: &Req| (403, "application/json".to_string(), json!({"error": {"message": "API key not valid"}}).to_string()))).await;
    let mut apis = ApiConfig::default();
    apis.psi.enabled = true;
    let cfg = CrawlConfig { threads: 2, max_urls: 1, apis, ..CrawlConfig::default() };
    let start_url = Url::parse(&format!("{origin}/")).unwrap();
    let dir = std::env::temp_dir().join(format!("toad-apis-err-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let _ = std::fs::remove_file(&db);
    let store = Mutex::new(Store::create(&db, &start_url, &cfg).unwrap());
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let run = tokio::spawn(Crawler::new(cfg.clone(), &start_url).unwrap().run(vec![start_url.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Finished(_) => break,
            _ => {}
        }
    }
    run.await.unwrap().unwrap();
    store.lock().unwrap().insert_batch(&records).unwrap();
    let runner = ApiRunner::new(cfg, Secrets::default()).with_endpoints(Endpoints { psi: format!("{api}/psi"), ..Endpoints::default() });
    runner.fetch_per_url(&store).await;
    let s = store.into_inner().unwrap();
    assert_eq!(s.count("PageSpeed", "Request Error").unwrap(), 1);
    let t = s.rows(&RowQuery { tab: "PageSpeed".into(), filter: "Request Error".into(), ..Default::default() }).unwrap();
    let c = t.columns.iter().position(|x| x == "PSI Error").unwrap();
    assert!(t.rows[0][c].contains("API key not valid"), "{:?}", t.rows[0]);
}
