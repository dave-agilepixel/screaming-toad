//! API Access > PageSpeed Insights (PSI API v5): Lighthouse lab metrics,
//! Chrome UX Report field data and the Core Web Vitals assessment, plus a
//! score per Lighthouse audit for the PageSpeed tab's opportunity filters.

use crate::{for_each_url, send_json, ApiRunner, Throttle};
use serde_json::Value;
use toad_core::api_config::{PsiConfig, PsiDevice};
use toad_store::api::{ApiValue, PSI, PSI_AUDITS};
use crate::StoreLock;

pub async fn run(runner: &ApiRunner, store: &dyn StoreLock) -> anyhow::Result<()> {
    let cfg = &runner.cfg.apis.psi;
    let urls: Vec<String> = {
        let s = store.lock_store()?;
        let done = s.api_done(PSI)?;
        s.api_targets(cfg.indexable_only)?.into_iter().filter(|u| !done.contains(u)).collect()
    };
    // PSI's default quota is 240 requests a minute.
    let throttle = Throttle::new(cfg.max_outstanding, 240);
    let key = runner.secrets.get(crate::keys::PSI).map(str::to_string);
    for_each_url(runner, store, PSI, urls, throttle, |url| {
        let key = key.clone();
        async move { fetch(runner, &url, key.as_deref()).await }
    })
    .await
}

/// One PSI request. Works without a key at a low shared quota.
pub async fn fetch(runner: &ApiRunner, url: &str, key: Option<&str>) -> anyhow::Result<Vec<ApiValue>> {
    let cfg = &runner.cfg.apis.psi;
    let strategy = match cfg.device {
        PsiDevice::Mobile => "mobile",
        PsiDevice::Desktop => "desktop",
    };
    let json = send_json(|| {
        let mut q: Vec<(&str, &str)> = vec![("url", url), ("strategy", strategy), ("category", "performance")];
        if let Some(k) = key {
            q.push(("key", k));
        }
        runner.client.get(&runner.endpoints.psi).query(&q)
    })
    .await?;
    Ok(parse(url, &json, cfg))
}

/// Turn a PSI response into values, keeping the metric groups switched on.
pub fn parse(url: &str, json: &Value, cfg: &PsiConfig) -> Vec<ApiValue> {
    let mut out = Vec::new();
    let mut put = |metric: &str, value: String| out.push(ApiValue::new(url, metric, value));
    let lh = &json["lighthouseResult"];
    let audit = |id: &str| &lh["audits"][id];
    let round = |v: f64| if v.fract() == 0.0 { format!("{v}") } else { format!("{:.3}", v).trim_end_matches('0').trim_end_matches('.').to_string() };

    if cfg.summary {
        if let Some(score) = lh["categories"]["performance"]["score"].as_f64() {
            put("performance_score", format!("{}", (score * 100.0).round()));
        }
        if let Some(b) = audit("total-byte-weight")["numericValue"].as_f64() {
            put("total_bytes", format!("{}", b.round()));
        }
    }
    if cfg.lighthouse_metrics {
        for (id, metric) in [
            ("first-contentful-paint", "lh_fcp_ms"),
            ("largest-contentful-paint", "lh_lcp_ms"),
            ("total-blocking-time", "lh_tbt_ms"),
            ("speed-index", "lh_si_ms"),
        ] {
            if let Some(v) = audit(id)["numericValue"].as_f64() {
                put(metric, format!("{}", v.round()));
            }
        }
        if let Some(v) = audit("cumulative-layout-shift")["numericValue"].as_f64() {
            put("lh_cls", round(v));
        }
    }
    if cfg.crux {
        let le = &json["loadingExperience"];
        // Without enough page-level data PSI falls back to the origin's.
        let page_data = le["metrics"].is_object() && !le["origin_fallback"].as_bool().unwrap_or(false);
        let p = |k: &str| le["metrics"][k]["percentile"].as_f64();
        if page_data {
            let lcp = p("LARGEST_CONTENTFUL_PAINT_MS");
            let inp = p("INTERACTION_TO_NEXT_PAINT");
            // The CLS percentile is reported ×100.
            let cls = p("CUMULATIVE_LAYOUT_SHIFT_SCORE").map(|c| c / 100.0);
            if let Some(v) = lcp {
                put("crux_lcp_ms", format!("{v}"));
            }
            if let Some(v) = inp {
                put("crux_inp_ms", format!("{v}"));
            }
            if let Some(v) = cls {
                put("crux_cls", round(v));
            }
            if let Some(v) = p("FIRST_CONTENTFUL_PAINT_MS") {
                put("crux_fcp_ms", format!("{v}"));
            }
            if let Some(v) = p("EXPERIMENTAL_TIME_TO_FIRST_BYTE") {
                put("crux_ttfb_ms", format!("{v}"));
            }
            let assessment = match (lcp, inp, cls) {
                (Some(l), Some(i), Some(c)) => if l <= 2500.0 && i <= 200.0 && c <= 0.1 { "Passed" } else { "Failed" },
                // INP can be missing for pages without interactions; LCP and CLS decide.
                (Some(l), None, Some(c)) => if l <= 2500.0 && c <= 0.1 { "Passed" } else { "Failed" },
                _ => "No Data",
            };
            put("cwv_assessment", assessment.into());
        } else {
            put("cwv_assessment", "No Data".into());
        }
    }
    if cfg.opportunities || cfg.diagnostics {
        for (id, _) in PSI_AUDITS {
            let a = audit(id);
            let Some(score) = a["score"].as_f64() else { continue };
            put(&format!("audit:{id}"), round(score));
            let savings = a["details"]["overallSavingsMs"].as_f64().or_else(|| {
                a["metricSavings"].as_object().and_then(|m| {
                    m.iter().filter(|(k, _)| k.as_str() != "CLS").filter_map(|(_, v)| v.as_f64()).fold(None, |acc: Option<f64>, v| Some(acc.map_or(v, |a| a.max(v))))
                })
            });
            if let Some(ms) = savings.filter(|ms| *ms > 0.0) {
                put(&format!("savings_ms:{id}"), format!("{}", ms.round()));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_response() {
        let json: Value = serde_json::json!({
            "loadingExperience": {
                "metrics": {
                    "LARGEST_CONTENTFUL_PAINT_MS": {"percentile": 2100},
                    "INTERACTION_TO_NEXT_PAINT": {"percentile": 250},
                    "CUMULATIVE_LAYOUT_SHIFT_SCORE": {"percentile": 5},
                    "FIRST_CONTENTFUL_PAINT_MS": {"percentile": 1200},
                    "EXPERIMENTAL_TIME_TO_FIRST_BYTE": {"percentile": 600}
                },
                "overall_category": "AVERAGE"
            },
            "lighthouseResult": {
                "categories": {"performance": {"score": 0.42}},
                "audits": {
                    "first-contentful-paint": {"numericValue": 1834.2},
                    "largest-contentful-paint": {"numericValue": 4021.9},
                    "total-blocking-time": {"numericValue": 310},
                    "speed-index": {"numericValue": 3999.5},
                    "cumulative-layout-shift": {"numericValue": 0.0812},
                    "total-byte-weight": {"numericValue": 1834567},
                    "render-blocking-insight": {"score": 0, "metricSavings": {"FCP": 450, "LCP": 620}},
                    "unused-javascript": {"score": 0.5, "details": {"overallSavingsMs": 300}},
                    "dom-size-insight": {"score": 1},
                    "viewport-insight": {"score": null}
                }
            }
        });
        let v = parse("https://x.com/", &json, &PsiConfig::default());
        let get = |m: &str| v.iter().find(|x| x.metric == m).map(|x| x.value.clone());
        assert_eq!(get("performance_score").as_deref(), Some("42"));
        assert_eq!(get("lh_lcp_ms").as_deref(), Some("4022"));
        assert_eq!(get("lh_cls").as_deref(), Some("0.081"));
        assert_eq!(get("crux_cls").as_deref(), Some("0.05"));
        assert_eq!(get("cwv_assessment").as_deref(), Some("Failed"), "INP 250 ms is not good");
        assert_eq!(get("audit:render-blocking-insight").as_deref(), Some("0"));
        assert_eq!(get("savings_ms:render-blocking-insight").as_deref(), Some("620"));
        assert_eq!(get("savings_ms:unused-javascript").as_deref(), Some("300"));
        assert_eq!(get("audit:dom-size-insight").as_deref(), Some("1"));
        assert_eq!(get("audit:viewport-insight"), None, "not applicable");
        let origin_only = serde_json::json!({"loadingExperience": {"metrics": {"LARGEST_CONTENTFUL_PAINT_MS": {"percentile": 1}}, "origin_fallback": true}});
        let v = parse("https://x.com/", &origin_only, &PsiConfig::default());
        assert!(v.iter().any(|x| x.metric == "cwv_assessment" && x.value == "No Data"));
    }
}
