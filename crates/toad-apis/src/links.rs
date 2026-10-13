//! API Access > Ahrefs, Majestic and Moz: link metrics per crawled page.
//!
//! Target scope (Exact URL, Subdomain, Root Domain) decides what each
//! request asks about. "Root domain" is the host without a leading `www.`
//! (no public-suffix list, so `shop.example.co.uk` stays as it is).

use crate::{keys, save, send_json, ApiRunner, Throttle};
use serde_json::{json, Value};
use std::collections::HashMap;
use toad_core::api_config::LinkTarget;
use toad_store::api::{ApiValue, AHREFS, MAJESTIC, MOZ};
use crate::StoreLock;

fn targets(store: &dyn StoreLock, source: &str) -> anyhow::Result<Vec<String>> {
    let s = store.lock_store()?;
    let done = s.api_done(source)?;
    Ok(s.api_targets(false)?.into_iter().filter(|u| !done.contains(u)).collect())
}

/// What to ask a provider about for a page URL.
pub fn target_of(url: &str, scope: LinkTarget) -> String {
    let host = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default();
    match scope {
        LinkTarget::Url => url.to_string(),
        LinkTarget::Subdomain => host,
        LinkTarget::RootDomain => host.strip_prefix("www.").map(str::to_string).unwrap_or(host),
    }
}

pub async fn run(runner: &ApiRunner, store: &dyn StoreLock) -> Vec<String> {
    let apis = &runner.cfg.apis;
    let mut errors = Vec::new();
    if apis.ahrefs.enabled {
        if let Err(e) = ahrefs(runner, store).await {
            errors.push(format!("Ahrefs: {e:#}"));
        }
    }
    if apis.majestic.enabled {
        if let Err(e) = majestic(runner, store).await {
            errors.push(format!("Majestic: {e:#}"));
        }
    }
    if apis.moz.enabled {
        if let Err(e) = moz(runner, store).await {
            errors.push(format!("Moz: {e:#}"));
        }
    }
    errors
}

/// Ahrefs API v3: Domain Rating (per host, cached), backlinks and referring
/// domains, and optionally organic traffic and keywords.
async fn ahrefs(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let key = runner.secrets.require(keys::AHREFS, "Ahrefs")?.to_string();
    let cfg = runner.cfg.apis.ahrefs.clone();
    let urls = targets(store, AHREFS)?;
    let date = crate::google::date_days_ago(0);
    let mode = match cfg.target {
        LinkTarget::Url => "exact",
        LinkTarget::Subdomain => "domain",
        LinkTarget::RootDomain => "subdomains",
    };
    let dr_cache: tokio::sync::Mutex<HashMap<String, Option<String>>> = Default::default();
    let get = |path: &'static str, target: String| {
        let key = key.clone();
        let date = date.clone();
        async move { ahrefs_get(runner, &key, path, &target, mode, &date).await }
    };
    crate::for_each_url(runner, store, AHREFS, urls, Throttle::new(4, 60), |url| {
        let target = target_of(&url, cfg.target);
        let host = target_of(&url, LinkTarget::Subdomain);
        let dr_cache = &dr_cache;
        let organic = cfg.organic;
        async move {
            let mut out = Vec::new();
            let stats = get("backlinks-stats", target.clone()).await?;
            let m = &stats["metrics"];
            if let Some(v) = m["live"].as_f64() {
                out.push(ApiValue::new(url.clone(), "backlinks", v));
            }
            if let Some(v) = m["live_refdomains"].as_f64() {
                out.push(ApiValue::new(url.clone(), "refdomains", v));
            }
            if organic {
                let metrics = get("metrics", target.clone()).await?;
                for (k, name) in [("org_traffic", "org_traffic"), ("org_keywords", "org_keywords")] {
                    if let Some(v) = metrics["metrics"][k].as_f64() {
                        out.push(ApiValue::new(url.clone(), name, v));
                    }
                }
            }
            let dr = {
                let cached = dr_cache.lock().await.get(&host).cloned();
                match cached {
                    Some(v) => v,
                    None => {
                        let j = get("domain-rating", host.clone()).await?;
                        let v = j.pointer("/domain_rating/domain_rating").and_then(Value::as_f64).map(|v| v.to_string());
                        dr_cache.lock().await.insert(host.clone(), v.clone());
                        v
                    }
                }
            };
            if let Some(v) = dr {
                out.push(ApiValue::new(url, "domain_rating", v));
            }
            Ok(out)
        }
    })
    .await
}

/// Majestic GetIndexItemInfo, 100 items a request.
async fn majestic(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let key = runner.secrets.require(keys::MAJESTIC, "Majestic")?.to_string();
    let cfg = runner.cfg.apis.majestic.clone();
    let urls = targets(store, MAJESTIC)?;
    let total = urls.len() as u64;
    let mut done = 0u64;
    for chunk in urls.chunks(100) {
        let items: Vec<String> = chunk.iter().map(|u| target_of(u, cfg.target)).collect();
        let result = majestic_call(runner, &key, &items, cfg.historic).await;
        match result {
            Ok(j) => {
                let rows = j.pointer("/DataTables/Results/Data").and_then(Value::as_array).cloned().unwrap_or_default();
                let mut values = Vec::new();
                for (url, row) in chunk.iter().zip(rows.iter()) {
                    for (k, name) in [("TrustFlow", "trust_flow"), ("CitationFlow", "citation_flow"), ("ExtBackLinks", "backlinks"), ("RefDomains", "refdomains")] {
                        if let Some(v) = row[k].as_f64() {
                            values.push(ApiValue::new(url.clone(), name, v));
                        }
                    }
                }
                save(store, MAJESTIC, &values, &[])?;
            }
            Err(e) => {
                let failed: Vec<(String, String)> = chunk.iter().map(|u| (u.clone(), format!("{e:#}"))).collect();
                save(store, MAJESTIC, &[], &failed)?;
            }
        }
        done += chunk.len() as u64;
        runner.report(MAJESTIC, done, total);
    }
    Ok(())
}

/// Moz Links API v2 url_metrics, 50 targets a request (access ID:secret).
async fn moz(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let key = runner.secrets.require(keys::MOZ, "Moz")?.to_string();
    let (id, secret) = key.split_once(':').ok_or_else(|| anyhow::anyhow!("enter the Moz key as ACCESS_ID:SECRET_KEY"))?;
    let cfg = runner.cfg.apis.moz.clone();
    let urls = targets(store, MOZ)?;
    let total = urls.len() as u64;
    let mut done = 0u64;
    for chunk in urls.chunks(50) {
        let targets: Vec<String> = chunk.iter().map(|u| target_of(u, cfg.target)).collect();
        match moz_call(runner, id, secret, &targets).await {
            Ok(j) => {
                let rows = j["results"].as_array().cloned().unwrap_or_default();
                let mut values = Vec::new();
                for (url, row) in chunk.iter().zip(rows.iter()) {
                    for (k, name) in [
                        ("page_authority", "page_authority"),
                        ("domain_authority", "domain_authority"),
                        ("spam_score", "spam_score"),
                        ("root_domains_to_page", "refdomains"),
                        ("external_pages_to_page", "backlinks"),
                    ] {
                        if let Some(v) = row[k].as_f64() {
                            values.push(ApiValue::new(url.clone(), name, v));
                        }
                    }
                }
                save(store, MOZ, &values, &[])?;
            }
            Err(e) => {
                let failed: Vec<(String, String)> = chunk.iter().map(|u| (u.clone(), format!("{e:#}"))).collect();
                save(store, MOZ, &[], &failed)?;
            }
        }
        done += chunk.len() as u64;
        runner.report(MOZ, done, total);
    }
    Ok(())
}

/// One Ahrefs v3 site-explorer request.
async fn ahrefs_get(runner: &ApiRunner, key: &str, path: &str, target: &str, mode: &str, date: &str) -> anyhow::Result<Value> {
    send_json(|| {
        runner
            .client
            .get(format!("{}/site-explorer/{path}", runner.endpoints.ahrefs))
            .bearer_auth(key)
            .query(&[("target", target), ("mode", mode), ("date", date), ("output", "json")])
    })
    .await
}

/// One Majestic GetIndexItemInfo request.
async fn majestic_call(runner: &ApiRunner, key: &str, items: &[String], historic: bool) -> anyhow::Result<Value> {
    let j = send_json(|| {
        let mut q: Vec<(String, String)> = vec![
            ("app_api_key".into(), key.to_string()),
            ("cmd".into(), "GetIndexItemInfo".into()),
            ("datasource".into(), if historic { "historic".into() } else { "fresh".into() }),
            ("items".into(), items.len().to_string()),
        ];
        for (i, it) in items.iter().enumerate() {
            q.push((format!("item{i}"), it.clone()));
        }
        runner.client.get(&runner.endpoints.majestic).query(&q)
    })
    .await?;
    if j["Code"].as_str().is_some_and(|c| c != "OK") {
        anyhow::bail!("{} ({})", j["ErrorMessage"].as_str().unwrap_or("Majestic error"), j["Code"].as_str().unwrap_or(""))
    }
    Ok(j)
}

/// One Moz url_metrics request.
async fn moz_call(runner: &ApiRunner, id: &str, secret: &str, targets: &[String]) -> anyhow::Result<Value> {
    let body = json!({"targets": targets});
    send_json(|| runner.client.post(format!("{}/url_metrics", runner.endpoints.moz)).basic_auth(id, Some(secret)).json(&body)).await
}

/// Endpoint checks without an account (`examples/probe_endpoints.rs`): one
/// request to each provider with the runner's keys.
pub async fn probe(runner: &ApiRunner, url: &str) -> Vec<(&'static str, anyhow::Result<String>)> {
    let key = |k: &str| runner.secrets.get(k).unwrap_or("").to_string();
    let date = crate::google::date_days_ago(0);
    let host = target_of(url, LinkTarget::Subdomain);
    let moz_key = key(keys::MOZ);
    let (id, secret) = moz_key.split_once(':').unwrap_or(("", ""));
    vec![
        ("Ahrefs", ahrefs_get(runner, &key(keys::AHREFS), "domain-rating", &host, "domain", &date).await.map(|v| v.to_string())),
        ("Majestic", majestic_call(runner, &key(keys::MAJESTIC), &[url.to_string()], false).await.map(|v| v.to_string())),
        ("Moz", moz_call(runner, id, secret, &[url.to_string()]).await.map(|v| v.to_string())),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_by_scope() {
        assert_eq!(target_of("https://www.x.com/a", LinkTarget::Url), "https://www.x.com/a");
        assert_eq!(target_of("https://www.x.com/a", LinkTarget::Subdomain), "www.x.com");
        assert_eq!(target_of("https://www.x.com/a", LinkTarget::RootDomain), "x.com");
    }
}
