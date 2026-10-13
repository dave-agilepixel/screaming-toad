//! Custom Search and Custom Extraction, compiled once per crawl.

use crate::config::{CrawlConfig, ExtractWhat, ExtractorKind, QueryKind, SearchIn, SearchMode};
use regex::Regex;
use scraper::{Html, Selector};

/// Extracted values kept per page, across all extractors.
const MAX_VALUES_PER_PAGE: usize = 1_000;

#[derive(Debug)]
pub struct CompiledSearch {
    pub name: String,
    pub mode: SearchMode,
    pub search_in: SearchIn,
    regex: Regex,
    /// Search In: XPath.
    xpath: Option<String>,
}

#[derive(Debug)]
enum Matcher {
    Css { selector: Selector, attribute: Option<String>, what: ExtractWhat },
    /// Compiled per page (sxd-xpath expressions are not Sync); validated here.
    Xpath { expression: String, attribute: Option<String>, what: ExtractWhat },
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
            let xpath = if s.search_in == SearchIn::Xpath {
                crate::xpath::validate(&s.xpath).map_err(|e| anyhow::anyhow!("custom search '{}': {e}", s.name))?;
                Some(s.xpath.clone())
            } else {
                None
            };
            searches.push(CompiledSearch { name: s.name.clone(), mode: s.mode, search_in: s.search_in, regex, xpath });
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
                ExtractorKind::Xpath => {
                    crate::xpath::validate(&x.expression).map_err(|e| anyhow::anyhow!("custom extraction '{}': {e}", x.name))?;
                    Matcher::Xpath { expression: x.expression.clone(), attribute: x.attribute.clone().filter(|a| !a.is_empty()), what: x.extract }
                }
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

    /// Whether any rule needs the page as an XPath document.
    pub fn needs_xpath(&self) -> bool {
        self.searches.iter().any(|s| s.xpath.is_some()) || self.extractors.iter().any(|x| matches!(x.matcher, Matcher::Xpath { .. }))
    }
}

/// The page in the forms Custom Search can look at.
pub struct SearchTargets<'a> {
    pub html: &'a str,
    pub head_html: &'a str,
    pub body_html: &'a str,
    pub page_text: &'a str,
    pub page_text_no_anchors: &'a str,
    pub content_text: &'a str,
    pub xpath: Option<&'a crate::xpath::XPathDoc>,
}

/// Match count per search. For "Does Not Contain" the count is still the
/// number of matches; the filter checks for zero.
pub fn run_searches(rules: &CustomRules, t: &SearchTargets) -> Vec<u32> {
    rules
        .searches
        .iter()
        .map(|s| {
            let selected;
            let haystack = match s.search_in {
                SearchIn::Html => t.html,
                SearchIn::HtmlHead => t.head_html,
                SearchIn::HtmlBody => t.body_html,
                SearchIn::PageText => t.page_text,
                SearchIn::PageTextNoAnchors => t.page_text_no_anchors,
                SearchIn::ContentArea => t.content_text,
                SearchIn::Xpath => {
                    selected = match (t.xpath, s.xpath.as_deref()) {
                        (Some(doc), Some(x)) => doc.values(x, ExtractWhat::Element, None, usize::MAX).unwrap_or_default().join("\n"),
                        _ => String::new(),
                    };
                    &selected
                }
            };
            s.regex.find_iter(haystack).count() as u32
        })
        .collect()
}

pub fn run_extractors(rules: &CustomRules, doc: &Html, html: &str, xdoc: Option<&crate::xpath::XPathDoc>) -> Vec<Vec<String>> {
    let mut budget = MAX_VALUES_PER_PAGE;
    rules
        .extractors
        .iter()
        .map(|x| match &x.matcher {
            Matcher::Xpath { expression, attribute, what } => xdoc
                .and_then(|d| d.values(expression, *what, attribute.as_deref(), MAX_VALUES_PER_PAGE).ok())
                .unwrap_or_default(),
            Matcher::Css { selector, attribute, what } => doc
                .select(selector)
                .filter_map(|el| match attribute {
                    Some(a) => el.value().attr(a).map(|v| v.trim().to_string()),
                    None => Some(match what {
                        ExtractWhat::Element => el.html(),
                        ExtractWhat::InnerHtml => el.inner_html().trim().to_string(),
                        ExtractWhat::Text | ExtractWhat::FunctionValue => el.text().collect::<Vec<_>>().join(" ").split_whitespace().collect::<Vec<_>>().join(" "),
                    }),
                })
                .take(MAX_VALUES_PER_PAGE)
                .collect(),
            // The first capture group if there is one, otherwise the whole match.
            Matcher::Regex(re) => re
                .captures_iter(html)
                .filter_map(|c| c.get(1).or_else(|| c.get(0)).map(|m| m.as_str().trim().to_string()))
                .take(MAX_VALUES_PER_PAGE)
                .collect(),
        })
        .map(|mut values: Vec<String>| {
            values.truncate(budget);
            budget -= values.len();
            values
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CustomExtractor, CustomSearch};
    use crate::xpath::XPathDoc;

    #[test]
    fn search_and_extract() {
        let mut cfg = CrawlConfig::default();
        cfg.custom_search = vec![
            CustomSearch { name: "GA".into(), query: "gtag(".into(), ..Default::default() },
            CustomSearch { name: "Price".into(), kind: QueryKind::Regex, query: r"£\d+".into(), search_in: SearchIn::PageText, ..Default::default() },
            CustomSearch { name: "Toad no anchors".into(), query: "toad".into(), search_in: SearchIn::PageTextNoAnchors, ..Default::default() },
            CustomSearch { name: "In H3".into(), query: "sale".into(), search_in: SearchIn::Xpath, xpath: "//h3".into(), ..Default::default() },
        ];
        cfg.custom_extraction = vec![
            CustomExtractor { name: "SKU".into(), kind: ExtractorKind::Css, expression: "span.sku".into(), ..Default::default() },
            CustomExtractor { name: "Img".into(), kind: ExtractorKind::Css, expression: "img".into(), attribute: Some("src".into()), ..Default::default() },
            CustomExtractor { name: "Author".into(), kind: ExtractorKind::Regex, expression: r#""author":\s*"([^"]+)""#.into(), ..Default::default() },
            CustomExtractor { name: "H3 (XPath default)".into(), expression: "//h3".into(), ..Default::default() },
            CustomExtractor { name: "Links".into(), expression: "count(//a)".into(), extract: ExtractWhat::FunctionValue, ..Default::default() },
        ];
        let rules = CustomRules::compile(&cfg).unwrap();
        assert!(rules.needs_xpath());
        let html = r#"<html><head><script>GTAG(1)</script><script>{"author": "Ann"}</script></head><body><span class="sku"> A1 </span><span class="sku">B2</span><img src="/x.png"><p>£20 or £30</p><h3>Big <i>sale</i></h3><a href="/t">toad</a></body></html>"#;
        let doc = Html::parse_document(html);
        let xdoc = XPathDoc::from_html(&doc);
        let t = SearchTargets {
            html,
            head_html: "",
            body_html: "",
            page_text: "£20 or £30 Big sale toad",
            page_text_no_anchors: "£20 or £30 Big sale",
            content_text: "",
            xpath: Some(&xdoc),
        };
        assert_eq!(run_searches(&rules, &t), vec![1, 2, 0, 1]);
        assert_eq!(
            run_extractors(&rules, &doc, html, Some(&xdoc)),
            vec![
                vec!["A1".to_string(), "B2".to_string()],
                vec!["/x.png".to_string()],
                vec!["Ann".to_string()],
                vec!["Big <i>sale</i>".to_string()],
                vec!["1".to_string()],
            ]
        );
    }

    #[test]
    fn invalid_xpath_is_reported() {
        let cfg = CrawlConfig {
            custom_extraction: vec![CustomExtractor { name: "Bad".into(), expression: "//div[".into(), ..Default::default() }],
            ..CrawlConfig::default()
        };
        let err = CustomRules::compile(&cfg).unwrap_err().to_string();
        assert!(err.contains("Bad") && err.contains("XPath"), "{err}");
    }

    #[test]
    fn values_are_capped_per_page() {
        let many: String = (0..1500).map(|i| format!("<b>{i}</b>")).collect();
        let html = format!("<html><body>{many}</body></html>");
        let cfg = CrawlConfig {
            custom_extraction: vec![
                CustomExtractor { name: "A".into(), expression: "//b".into(), extract: ExtractWhat::Text, ..Default::default() },
                CustomExtractor { name: "B".into(), expression: "//b".into(), extract: ExtractWhat::Text, ..Default::default() },
            ],
            ..CrawlConfig::default()
        };
        let rules = CustomRules::compile(&cfg).unwrap();
        let doc = Html::parse_document(&html);
        let xdoc = XPathDoc::from_html(&doc);
        let out = run_extractors(&rules, &doc, &html, Some(&xdoc));
        assert_eq!((out[0].len(), out[1].len()), (1000, 0), "1,000 values per page in total");
    }
}
