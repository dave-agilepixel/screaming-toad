//! Google Search Console (Search Analytics, URL Inspection) and GA4 (Data
//! API), with an OAuth 2.0 loopback sign-in.
//!
//! Screaming Frog ships its own Google OAuth client. Toad can't, so the user
//! creates a "Desktop app" OAuth client in Google Cloud and enters its ID and
//! secret; signing in stores a refresh token in the OS credential store.

use crate::{keys, send_json, ApiRunner, Throttle};
use base64::Engine;
use serde_json::{json, Value};
use sha2::Digest;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use toad_core::api_config::GscDevice;
use toad_store::api::{ApiValue, INSPECT};
use crate::StoreLock;

pub const SCOPES: &str = "https://www.googleapis.com/auth/webmasters.readonly https://www.googleapis.com/auth/analytics.readonly";

#[derive(Debug, Clone)]
pub struct AccessToken {
    token: String,
    expires: Instant,
}

fn random_b64(n: usize) -> String {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).ok();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// Sign in with Google: opens the consent page (via `open`), waits for the
/// redirect to a local port, and exchanges the code (PKCE) for a refresh
/// token, which is returned.
pub async fn authorize(
    client: &reqwest::Client,
    endpoints: &crate::Endpoints,
    client_id: &str,
    client_secret: &str,
    open: impl FnOnce(&str),
) -> anyhow::Result<String> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let redirect = format!("http://127.0.0.1:{}", listener.local_addr()?.port());
    let verifier = random_b64(48);
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier.as_bytes()));
    let state = random_b64(16);
    let auth_url = url::Url::parse_with_params(
        &endpoints.google_auth,
        &[
            ("client_id", client_id),
            ("redirect_uri", redirect.as_str()),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("code_challenge", challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
            ("prompt", "consent"),
            ("state", state.as_str()),
        ],
    )?;
    open(auth_url.as_str());

    let (mut sock, _) = tokio::time::timeout(Duration::from_secs(300), listener.accept())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for Google sign-in"))??;
    let mut buf = vec![0u8; 8192];
    let n = sock.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]).to_string();
    let target = req.split_whitespace().nth(1).unwrap_or("/").to_string();
    let got = url::Url::parse(&format!("http://localhost{target}"))?;
    let param = |k: &str| got.query_pairs().find(|(n, _)| n == k).map(|(_, v)| v.into_owned());
    let ok = param("state").as_deref() == Some(state.as_str()) && param("code").is_some();
    let page = if ok { "Signed in. You can close this window and return to Screaming Toad." } else { "Sign-in failed. Return to Screaming Toad and try again." };
    let body = format!("<!doctype html><html><body style=\"font-family:sans-serif\"><p>{page}</p></body></html>");
    let _ = sock.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).await;
    let _ = sock.shutdown().await;
    if let Some(e) = param("error") {
        anyhow::bail!("Google sign-in: {e}");
    }
    anyhow::ensure!(ok, "Google sign-in returned no code (or a mismatched state)");

    let code = param("code").unwrap_or_default();
    let json = send_json(|| {
        client.post(&endpoints.google_token).form(&[
            ("code", code.as_str()),
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier.as_str()),
        ])
    })
    .await?;
    json["refresh_token"].as_str().map(str::to_string).ok_or_else(|| anyhow::anyhow!("Google returned no refresh token"))
}

impl ApiRunner {
    /// A current Google access token, refreshed when needed.
    pub async fn google_token(&self) -> anyhow::Result<String> {
        let mut cached = self.google.lock().await;
        if let Some(t) = cached.as_ref().filter(|t| t.expires > Instant::now() + Duration::from_secs(60)) {
            return Ok(t.token.clone());
        }
        let id = self.secrets.require(keys::GOOGLE_CLIENT_ID, "Google")?;
        let secret = self.secrets.require(keys::GOOGLE_CLIENT_SECRET, "Google")?;
        let refresh = self
            .secrets
            .get(keys::GOOGLE_REFRESH_TOKEN)
            .ok_or_else(|| anyhow::anyhow!("Google: not signed in. Use Connect in Configuration > API Access > Google."))?;
        let json = send_json(|| {
            self.client.post(&self.endpoints.google_token).form(&[
                ("client_id", id),
                ("client_secret", secret),
                ("refresh_token", refresh),
                ("grant_type", "refresh_token"),
            ])
        })
        .await?;
        let token = json["access_token"].as_str().ok_or_else(|| anyhow::anyhow!("Google returned no access token"))?.to_string();
        let ttl = json["expires_in"].as_u64().unwrap_or(3600);
        *cached = Some(AccessToken { token: token.clone(), expires: Instant::now() + Duration::from_secs(ttl) });
        Ok(token)
    }

    /// Search Console properties the account can read.
    pub async fn gsc_properties(&self) -> anyhow::Result<Vec<String>> {
        let token = self.google_token().await?;
        let json = send_json(|| self.client.get(format!("{}/sites", self.endpoints.gsc)).bearer_auth(&token)).await?;
        let mut sites: Vec<String> = json["siteEntry"]
            .as_array()
            .map(|a| a.iter().filter_map(|s| s["siteUrl"].as_str().map(str::to_string)).collect())
            .unwrap_or_default();
        sites.sort();
        Ok(sites)
    }

    /// GA4 properties: (property ID, "Account › Property").
    pub async fn ga4_properties(&self) -> anyhow::Result<Vec<(String, String)>> {
        let token = self.google_token().await?;
        let mut out = Vec::new();
        let mut page_token: Option<String> = None;
        loop {
            let json = send_json(|| {
                let mut r = self.client.get(format!("{}/accountSummaries", self.endpoints.ga4_admin)).bearer_auth(&token).query(&[("pageSize", "200")]);
                if let Some(p) = &page_token {
                    r = r.query(&[("pageToken", p.as_str())]);
                }
                r
            })
            .await?;
            for acc in json["accountSummaries"].as_array().into_iter().flatten() {
                let account = acc["displayName"].as_str().unwrap_or("");
                for p in acc["propertySummaries"].as_array().into_iter().flatten() {
                    let id = p["property"].as_str().unwrap_or("").trim_start_matches("properties/").to_string();
                    out.push((id, format!("{account} › {}", p["displayName"].as_str().unwrap_or(""))));
                }
            }
            page_token = json["nextPageToken"].as_str().filter(|s| !s.is_empty()).map(str::to_string);
            if page_token.is_none() {
                break;
            }
        }
        Ok(out)
    }
}

/// YYYY-MM-DD for `days_ago` days before today (UTC).
pub fn date_days_ago(days_ago: u64) -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let days = (secs / 86_400) as i64 - days_ago as i64;
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Search Console > Search Analytics: clicks, impressions, CTR and position
/// per page over the date range (ending 2 days ago, as data lags).
pub async fn search_analytics(runner: &ApiRunner) -> anyhow::Result<Vec<ApiValue>> {
    let cfg = &runner.cfg.apis.gsc;
    anyhow::ensure!(!cfg.property.trim().is_empty(), "choose a Search Console property");
    let token = runner.google_token().await?;
    let endpoint = format!("{}/sites/{}/searchAnalytics/query", runner.endpoints.gsc, url::form_urlencoded::byte_serialize(cfg.property.trim().as_bytes()).collect::<String>());
    let mut filters = Vec::new();
    let device = match cfg.device {
        GscDevice::All => None,
        GscDevice::Desktop => Some("DESKTOP"),
        GscDevice::Mobile => Some("MOBILE"),
        GscDevice::Tablet => Some("TABLET"),
    };
    if let Some(d) = device {
        filters.push(json!({"dimension": "device", "operator": "equals", "expression": d}));
    }
    if !cfg.country.trim().is_empty() {
        filters.push(json!({"dimension": "country", "operator": "equals", "expression": cfg.country.trim().to_lowercase()}));
    }
    if !cfg.query_filter.trim().is_empty() {
        filters.push(json!({"dimension": "query", "operator": if cfg.query_exclude { "notContains" } else { "contains" }, "expression": cfg.query_filter.trim()}));
    }
    const PAGE: usize = 25_000;
    let mut out = Vec::new();
    let mut start_row = 0usize;
    loop {
        let mut body = json!({
            "startDate": date_days_ago(cfg.days as u64 + 2),
            "endDate": date_days_ago(2),
            "dimensions": ["page"],
            "type": cfg.search_type.api_name(),
            "rowLimit": PAGE,
            "startRow": start_row,
            "dataState": "final",
        });
        if !filters.is_empty() {
            body["dimensionFilterGroups"] = json!([{"filters": filters}]);
        }
        let json = send_json(|| runner.client.post(&endpoint).bearer_auth(&token).json(&body)).await?;
        let rows = json["rows"].as_array().cloned().unwrap_or_default();
        for r in &rows {
            let Some(page) = r["keys"][0].as_str() else { continue };
            let n = |k: &str| r[k].as_f64().unwrap_or(0.0);
            out.push(ApiValue::new(page, "clicks", n("clicks")));
            out.push(ApiValue::new(page, "impressions", n("impressions")));
            out.push(ApiValue::new(page, "ctr", format!("{:.4}", n("ctr"))));
            out.push(ApiValue::new(page, "position", format!("{:.2}", n("position"))));
        }
        if rows.len() < PAGE || start_row >= 1_000_000 {
            break;
        }
        start_row += PAGE;
    }
    Ok(out)
}

/// Search Console > URL Inspection, per crawled page, up to the daily limit.
pub async fn run_inspection(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let cfg = &runner.cfg.apis.gsc;
    anyhow::ensure!(!cfg.property.trim().is_empty(), "choose a Search Console property");
    let today = date_days_ago(0);
    let meta_key = format!("inspections:{}:{today}", cfg.property.trim());
    let (urls, used) = {
        let s = store.lock_store()?;
        let done = s.api_done(INSPECT)?;
        let used: u32 = s.meta(&meta_key)?.and_then(|v| v.parse().ok()).unwrap_or(0);
        let urls: Vec<String> = s.api_targets(cfg.inspection_indexable_only)?.into_iter().filter(|u| !done.contains(u)).collect();
        (urls, used)
    };
    let left = cfg.inspection_daily_limit.saturating_sub(used) as usize;
    let urls: Vec<String> = urls.into_iter().take(left).collect();
    if urls.is_empty() {
        return Ok(());
    }
    let count = urls.len();
    let token = runner.google_token().await?;
    let property = cfg.property.trim().to_string();
    // URL Inspection allows 600 requests a minute.
    crate::for_each_url(runner, store, INSPECT, urls, Throttle::new(5, 600), |url| {
        let token = token.clone();
        let property = property.clone();
        async move {
            let json = send_json(|| {
                runner.client.post(&runner.endpoints.inspection).bearer_auth(&token).json(&json!({"inspectionUrl": url, "siteUrl": property, "languageCode": "en-US"}))
            })
            .await?;
            Ok(parse_inspection(&url, &json))
        }
    })
    .await?;
    let s = store.lock_store()?;
    s.set_meta(&meta_key, &(used as usize + count).to_string())?;
    Ok(())
}

pub fn parse_inspection(url: &str, json: &Value) -> Vec<ApiValue> {
    let r = &json["inspectionResult"];
    let ix = &r["indexStatusResult"];
    let mut out = Vec::new();
    for (key, metric) in [
        ("verdict", "verdict"),
        ("coverageState", "coverage"),
        ("robotsTxtState", "robots"),
        ("indexingState", "indexing"),
        ("pageFetchState", "page_fetch"),
        ("lastCrawlTime", "last_crawl"),
        ("crawledAs", "crawled_as"),
        ("googleCanonical", "google_canonical"),
        ("userCanonical", "user_canonical"),
    ] {
        if let Some(v) = ix[key].as_str() {
            out.push(ApiValue::new(url, metric, v));
        }
    }
    if let Some(v) = r["richResultsResult"]["verdict"].as_str() {
        out.push(ApiValue::new(url, "rich_results", v));
    }
    if out.is_empty() {
        // Keep a row so the page counts as inspected.
        out.push(ApiValue::new(url, "verdict", "VERDICT_UNSPECIFIED"));
    }
    out
}

/// GA4 > runReport: the configured metrics per page (host + path) over the
/// date range, optionally for one default channel group.
pub async fn ga4_report(runner: &ApiRunner) -> anyhow::Result<Vec<ApiValue>> {
    let cfg = &runner.cfg.apis.ga4;
    let property = cfg.property.trim().trim_start_matches("properties/");
    anyhow::ensure!(!property.is_empty(), "choose a GA4 property");
    anyhow::ensure!(!cfg.metrics.is_empty(), "choose at least one GA4 metric");
    let token = runner.google_token().await?;
    let endpoint = format!("{}/properties/{property}:runReport", runner.endpoints.ga4_data);
    let limit = cfg.max_rows.clamp(1, 250_000) as usize;
    let page = limit.min(100_000);
    let mut out = Vec::new();
    let mut offset = 0usize;
    loop {
        let mut body = json!({
            "dateRanges": [{"startDate": format!("{}daysAgo", cfg.days.max(1)), "endDate": "yesterday"}],
            "dimensions": [{"name": "hostName"}, {"name": "pagePath"}],
            "metrics": cfg.metrics.iter().map(|m| json!({"name": m})).collect::<Vec<_>>(),
            "limit": page,
            "offset": offset,
        });
        if !cfg.channel.trim().is_empty() {
            body["dimensionFilter"] = json!({"filter": {"fieldName": "sessionDefaultChannelGroup", "stringFilter": {"value": cfg.channel.trim(), "matchType": "EXACT"}}});
        }
        let json = send_json(|| runner.client.post(&endpoint).bearer_auth(&token).json(&body)).await?;
        let rows = json["rows"].as_array().cloned().unwrap_or_default();
        let names: Vec<String> = json["metricHeaders"]
            .as_array()
            .map(|h| h.iter().filter_map(|m| m["name"].as_str().map(str::to_string)).collect())
            .unwrap_or_else(|| cfg.metrics.clone());
        for r in &rows {
            let host = r["dimensionValues"][0]["value"].as_str().unwrap_or("");
            let path = r["dimensionValues"][1]["value"].as_str().unwrap_or("");
            if host.is_empty() || host == "(not set)" || !path.starts_with('/') {
                continue;
            }
            let url = format!("https://{host}{path}");
            for (i, name) in names.iter().enumerate() {
                if let Some(v) = r["metricValues"][i]["value"].as_str() {
                    out.push(ApiValue::new(url.clone(), name.clone(), v));
                }
            }
        }
        offset += rows.len();
        let total = json["rowCount"].as_u64().unwrap_or(0) as usize;
        if rows.is_empty() || offset >= total || offset >= limit {
            break;
        }
    }
    Ok(out)
}

/// GA4 reports hosts, not schemes: match its https URLs to the crawl's
/// scheme (an http crawl gets http keys).
pub fn rescheme(values: &mut [ApiValue], scheme: &str) {
    if scheme == "https" {
        return;
    }
    for v in values {
        if let Some(rest) = v.url.strip_prefix("https://") {
            v.url = format!("{scheme}://{rest}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19_723), (2024, 1, 1));
        assert_eq!(date_days_ago(0).len(), 10);
    }

    #[test]
    fn inspection_parse() {
        let j = json!({"inspectionResult": {"indexStatusResult": {"verdict": "PASS", "coverageState": "Submitted and indexed", "googleCanonical": "https://x.com/a", "userCanonical": "https://x.com/b"}, "richResultsResult": {"verdict": "FAIL"}}});
        let v = parse_inspection("https://x.com/a", &j);
        let get = |m: &str| v.iter().find(|x| x.metric == m).map(|x| x.value.as_str());
        assert_eq!(get("verdict"), Some("PASS"));
        assert_eq!(get("user_canonical"), Some("https://x.com/b"));
        assert_eq!(get("rich_results"), Some("FAIL"));
    }
}
