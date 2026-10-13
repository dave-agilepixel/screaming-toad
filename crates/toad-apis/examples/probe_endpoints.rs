//! Call every provider's real endpoint with an obviously invalid key, to
//! check URLs, request shapes and error parsing without an account:
//! `cargo run -p toad-apis --example probe_endpoints`. A "bad key" style
//! error means the endpoint and request are right; 404 or a format error
//! means they're wrong.

use toad_apis::{keys, ApiRunner, Secrets};
use toad_core::api_config::AiProvider;
use toad_core::CrawlConfig;

#[tokio::main]
async fn main() {
    let fake = "toad-invalid-test-key";
    let secrets = Secrets::new(
        [
            (keys::PSI, fake),
            (keys::GOOGLE_CLIENT_ID, "toad-test.apps.googleusercontent.com"),
            (keys::GOOGLE_CLIENT_SECRET, fake),
            (keys::GOOGLE_REFRESH_TOKEN, fake),
            (keys::OPENAI, fake),
            (keys::ANTHROPIC, fake),
            (keys::GEMINI, fake),
            (keys::AHREFS, fake),
            (keys::MAJESTIC, fake),
            (keys::MOZ, "toad-id:toad-secret"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    );
    let mut cfg = CrawlConfig::default();
    cfg.apis.gsc.property = "sc-domain:example.com".into();
    cfg.apis.ga4.property = "123456789".into();
    let r = ApiRunner::new(cfg, secrets);
    let show = |name: &str, res: anyhow::Result<String>| match res {
        Ok(v) => println!("{name:<12} OK (unexpected): {}", v.chars().take(120).collect::<String>()),
        Err(e) => println!("{name:<12} {}", format!("{e:#}").chars().take(220).collect::<String>()),
    };

    show("PSI", toad_apis::psi::fetch(&r, "https://example.com/", Some(fake)).await.map(|v| format!("{} values", v.len())));
    show("Google", r.google_token().await);
    show("OpenAI", r.complete(AiProvider::OpenAi, "gpt-4o-mini", "Say hi", "").await);
    show("Anthropic", r.complete(AiProvider::Anthropic, "claude-haiku-5-5", "Say hi", "").await);
    show("Gemini", r.complete(AiProvider::Gemini, "gemini-2.5-flash", "Say hi", "").await);
    show("OpenAI emb", r.embed(AiProvider::OpenAi, "text-embedding-3-small", "toads").await.map(|v| format!("{} dims", v.len())));
    show("Gemini emb", r.embed(AiProvider::Gemini, "gemini-embedding-001", "toads").await.map(|v| format!("{} dims", v.len())));
    // Google API paths, with a fake access token (expect 401, not 404).
    let e = toad_apis::Endpoints::default();
    let sc = format!("{}/sites/{}/searchAnalytics/query", e.gsc, "sc-domain%3Aexample.com");
    for (name, req) in [
        ("GSC sites", r.client.get(format!("{}/sites", e.gsc))),
        ("GSC query", r.client.post(&sc).json(&serde_json::json!({"startDate": "2026-01-01", "endDate": "2026-01-31", "dimensions": ["page"]}))),
        ("URL Inspect", r.client.post(&e.inspection).json(&serde_json::json!({"inspectionUrl": "https://example.com/", "siteUrl": "sc-domain:example.com"}))),
        ("GA4 admin", r.client.get(format!("{}/accountSummaries", e.ga4_admin))),
        ("GA4 report", r.client.post(format!("{}/properties/123456789:runReport", e.ga4_data)).json(&serde_json::json!({"dimensions": [{"name": "pagePath"}], "metrics": [{"name": "sessions"}]}))),
    ] {
        match req.bearer_auth("ya29.toad-invalid").send().await {
            Ok(resp) => {
                let st = resp.status();
                let body = resp.text().await.unwrap_or_default();
                let msg = serde_json::from_str::<serde_json::Value>(&body).ok().and_then(|v| v.pointer("/error/status").and_then(|m| m.as_str()).map(str::to_string)).unwrap_or_default();
                println!("{name:<12} HTTP {st} {msg}");
            }
            Err(err) => println!("{name:<12} {err}"),
        }
    }
    for (name, res) in toad_apis::links::probe(&r, "https://example.com/").await {
        show(name, res);
    }
}
