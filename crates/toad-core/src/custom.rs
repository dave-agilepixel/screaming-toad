//! Custom Search and Custom Extraction, compiled once per crawl.

use crate::config::{CrawlConfig, ExtractWhat, ExtractorKind, QueryKind, SearchIn, SearchMode};
use regex::Regex;
use scraper::{Html, Selector};

/// Values kept per extractor per page.
const MAX_VALUES_PER_EXTRACTOR: usize = 100;

#[derive(Debug)]
pub struct CompiledSearch {
    pub name: String,
    pub mode: SearchMode,
    pub search_in: SearchIn,
    regex: Regex,
}

#[derive(Debug)]
enum Matcher {
    Css { selector: Selector, attribute: Option<String>, what: ExtractWhat },
    Regex(Regex),
}

#[derive(Debug)]
pub struct CompiledExtractor {
    pub name: String,
    matcher: Matcher,
}

#[derive(Debug, Default)]
pub struct CustomRules {
    pub searches: Vec<CompiledSearch>,
    pub extractors: Vec<CompiledExtractor>,
}

impl CustomRules {
    pub fn compile(cfg: &CrawlConfig) -> anyhow::Result<Self> {
        let mut searches = Vec::new();
        for s in cfg.custom_search.iter().filter(|s| !s.query.is_empty()) {
            let pattern = match s.kind {
                QueryKind::Regex => s.query.clone(),
                QueryKind::Text if s.case_sensitive => regex::escape(&s.query),
                QueryKind::Text => format!("(?i){}", regex::escape(&s.query)),
            };
            let regex = Regex::new(&pattern).map_err(|e| anyhow::anyhow!("custom search '{}': {e}", s.name))?;
            searches.push(CompiledSearch { name: s.name.clone(), mode: s.mode, search_in: s.search_in, regex });
        }
        let mut extractors = Vec::new();
        for x in cfg.custom_extraction.iter().filter(|x| !x.expression.is_empty()) {
            let matcher = match x.kind {
                ExtractorKind::Css => Matcher::Css {
                    selector: Selector::parse(&x.expression)
                        .map_err(|e| anyhow::anyhow!("custom extraction '{}': invalid CSS selector: {e:?}", x.name))?,
                    attribute: x.attribute.clone().filter(|a| !a.is_empty()),
                    what: x.extract,
                },
                // Like Screaming Frog: `.` matches newlines.
                ExtractorKind::Regex => Matcher::Regex(
                    Regex::new(&format!("(?s){}", x.expression))
                        .map_err(|e| anyhow::anyhow!("custom extraction '{}': {e}", x.name))?,
                ),
            };
            extractors.push(CompiledExtractor { name: x.name.clone(), matcher });
        }
        Ok(Self { searches, extractors })
    }

    pub fn is_empty(&self) -> bool {
        self.searches.is_empty() && self.extractors.is_empty()
    }
}

/// The page in the forms Custom Search can look at.
pub struct SearchTargets<'a> {
    pub html: &'a str,
    pub head_html: &'a str,
    pub body_html: &'a str,
    pub page_text: &'a str,
    pub content_text: &'a str,
}

/// Match count per search. For "Does Not Contain" the count is still the
/// number of matches; the filter checks for zero.
pub fn run_searches(rules: &CustomRules, t: &SearchTargets) -> Vec<u32> {
    rules
        .searches
        .iter()
        .map(|s| {
            let haystack = match s.search_in {
                SearchIn::Html => t.html,
                SearchIn::HtmlHead => t.head_html,
                SearchIn::HtmlBody => t.body_html,
                SearchIn::PageText => t.page_text,
                SearchIn::ContentArea => t.content_text,
            };
            s.regex.find_iter(haystack).count() as u32
        })
        .collect()
}

pub fn run_extractors(rules: &CustomRules, doc: &Html, html: &str) -> Vec<Vec<String>> {
    rules
        .extractors
        .iter()
        .map(|x| match &x.matcher {
            Matcher::Css { selector, attribute, what } => doc
                .select(selector)
                .filter_map(|el| match attribute {
                    Some(a) => el.value().attr(a).map(|v| v.trim().to_string()),
                    None => Some(match what {
                        ExtractWhat::Element => el.html(),
                        ExtractWhat::InnerHtml => el.inner_html(),
                        ExtractWhat::Text => el.text().collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" "),
                    }),
                })
                .take(MAX_VALUES_PER_EXTRACTOR)
                .collect(),
            // The first capture group if there is one, otherwise the whole match.
            Matcher::Regex(re) => re
                .captures_iter(html)
                .filter_map(|c| c.get(1).or_else(|| c.get(0)).map(|m| m.as_str().trim().to_string()))
                .take(MAX_VALUES_PER_EXTRACTOR)
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CustomExtractor, CustomSearch};

    #[test]
    fn search_and_extract() {
        let mut cfg = CrawlConfig::default();
        cfg.custom_search = vec![
            CustomSearch { name: "GA".into(), query: "gtag(".into(), ..Default::default() },
            CustomSearch { name: "Price".into(), kind: QueryKind::Regex, query: r"£\d+".into(), search_in: SearchIn::PageText, ..Default::default() },
        ];
        cfg.custom_extraction = vec![
            CustomExtractor { name: "SKU".into(), expression: "span.sku".into(), ..Default::default() },
            CustomExtractor { name: "Img".into(), expression: "img".into(), attribute: Some("src".into()), ..Default::default() },
            CustomExtractor { name: "Author".into(), kind: ExtractorKind::Regex, expression: r#""author":\s*"([^"]+)""#.into(), ..Default::default() },
        ];
        let rules = CustomRules::compile(&cfg).unwrap();
        let html = r#"<html><head><script>GTAG(1)</script><script>{"author": "Ann"}</script></head><body><span class="sku"> A1 </span><span class="sku">B2</span><img src="/x.png"><p>£20 or £30</p></body></html>"#;
        let doc = Html::parse_document(html);
        let t = SearchTargets { html, head_html: "", body_html: "", page_text: "£20 or £30", content_text: "" };
        assert_eq!(run_searches(&rules, &t), vec![1, 2]);
        assert_eq!(
            run_extractors(&rules, &doc, html),
            vec![vec!["A1".to_string(), "B2".to_string()], vec!["/x.png".to_string()], vec!["Ann".to_string()]]
        );
    }
}
