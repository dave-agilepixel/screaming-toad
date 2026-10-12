//! HTML extraction: one pass over the parsed document.

use crate::config::{ContentArea, CrawlConfig};
use crate::custom::{run_extractors, run_searches, CustomRules, SearchTargets};
use crate::minhash;
use crate::model::{Link, LinkKind, PageData};
use crate::urlutil::{malformed, normalize, resolve, PathType};
use regex::Regex;
use scraper::{ElementRef, Html, Node};
use std::sync::OnceLock;
use url::Url;

/// Default link positions, matched in order against the link path.
/// Content (`/`) must stay last because it matches everything.
const LINK_POSITIONS: [(&str, &str); 6] = [
    ("Head", "/head/"),
    ("Navigation", "nav"),
    ("Header", "header"),
    ("Aside", "aside"),
    ("Footer", "footer"),
    ("Content", "/"),
];

/// Elements whose text is never page content.
const NON_TEXT_ELEMENTS: [&str; 7] = ["script", "style", "noscript", "template", "svg", "canvas", "head"];

pub fn extract(html: &str, page_url: &Url, cfg: &CrawlConfig, rules: &CustomRules) -> PageData {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    let mut p = PageData {
        language: root.value().attr("lang").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        ..Default::default()
    };

    let base = root
        .descendants()
        .filter_map(ElementRef::wrap)
        .find(|e| e.value().name() == "base")
        .and_then(|e| e.value().attr("href"))
        .and_then(|h| page_url.join(h).ok())
        .unwrap_or_else(|| page_url.clone());

    let robots_token = cfg.robots_user_agent.to_ascii_lowercase();
    let mut heading_levels: Vec<u8> = Vec::new();

    for el in root.descendants().filter_map(ElementRef::wrap) {
        let v = el.value();
        let name = v.name();
        match name {
            "title" => {
                if el.ancestors().any(|a| matches!(a.value(), Node::Element(e) if e.name() == "svg")) {
                    continue;
                }
                p.titles.push(clean_text(&el));
                p.title_outside_head |= !in_head(&el);
            }
            "meta" => {
                let content = v.attr("content").unwrap_or("").trim().to_string();
                let meta_name = v.attr("name").unwrap_or("").to_ascii_lowercase();
                match meta_name.as_str() {
                    "description" => {
                        p.meta_descriptions.push(content);
                        p.description_outside_head |= !in_head(&el);
                    }
                    "keywords" => p.meta_keywords.push(content),
                    "viewport" => p.has_viewport = true,
                    n if n == "robots" || (!robots_token.is_empty() && n == robots_token) => {
                        p.meta_robots.push(content);
                        p.robots_outside_head |= !in_head(&el);
                    }
                    _ => {}
                }
                if v.attr("http-equiv").is_some_and(|h| h.eq_ignore_ascii_case("refresh")) {
                    let raw = v.attr("content").unwrap_or("").to_string();
                    if let Some(target) = refresh_target(&raw) {
                        if let Some(link) = make_link(LinkKind::MetaRefresh, &target, &el, &base, cfg) {
                            p.meta_refresh_url = Some(link.to.clone());
                            p.links.push(link);
                        }
                    }
                    p.meta_refresh = Some(raw);
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let level = name.as_bytes()[1] - b'0';
                heading_levels.push(level);
                match level {
                    1 => {
                        p.h1.push(clean_text(&el));
                        p.h1_has_alt_text |= el
                            .descendants()
                            .filter_map(ElementRef::wrap)
                            .any(|i| i.value().name() == "img" && i.value().attr("alt").is_some_and(|a| !a.trim().is_empty()));
                    }
                    2 => p.h2.push(clean_text(&el)),
                    _ => {}
                }
            }
            "link" => {
                let rel = v.attr("rel").unwrap_or("").to_ascii_lowercase();
                let rels: Vec<&str> = rel.split_whitespace().collect();
                let Some(href) = v.attr("href") else { continue };
                if rels.contains(&"canonical") {
                    let outside = !in_head(&el);
                    p.canonical_outside_head |= outside;
                    p.canonical_is_relative |= PathType::of(href) != PathType::Absolute;
                    p.canonical_has_fragment |= href.contains('#');
                    p.canonical_invalid_attribute |= ["hreflang", "lang", "media", "type"].iter().any(|a| v.attr(a).is_some());
                    if let Some(link) = make_link(LinkKind::Canonical, href, &el, &base, cfg) {
                        p.canonicals.push(link.to.clone());
                        p.links.push(link);
                    }
                } else if rels.contains(&"next") || rels.contains(&"prev") || rels.contains(&"previous") {
                    let kind = if rels.contains(&"next") { LinkKind::RelNext } else { LinkKind::RelPrev };
                    if let Some(link) = make_link(kind, href, &el, &base, cfg) {
                        if kind == LinkKind::RelNext { p.rel_next.push(link.to.clone()) } else { p.rel_prev.push(link.to.clone()) }
                        p.links.push(link);
                    }
                } else if rels.contains(&"alternate") && v.attr("hreflang").is_some() {
                    let lang = v.attr("hreflang").unwrap_or("").trim().to_string();
                    p.hreflang_outside_head |= !in_head(&el);
                    if let Some(mut link) = make_link(LinkKind::Hreflang, href, &el, &base, cfg) {
                        link.hreflang = Some(lang.clone());
                        p.hreflang.push((lang, link.to.clone()));
                        p.links.push(link);
                    }
                } else if rels.contains(&"amphtml") {
                    if let Some(link) = make_link(LinkKind::Amp, href, &el, &base, cfg) {
                        p.links.push(link);
                    }
                } else if rels.contains(&"alternate") && v.attr("media").is_some() {
                    if let Some(link) = make_link(LinkKind::MobileAlternate, href, &el, &base, cfg) {
                        p.links.push(link);
                    }
                } else if rels.contains(&"stylesheet") {
                    if let Some(link) = make_link(LinkKind::Css, href, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "a" | "area" => {
                let href = v.attr("href");
                let js = href.is_some_and(|h| h.trim().to_ascii_lowercase().starts_with("javascript:"));
                if cfg.store_uncrawlable_links && (js || (href.is_none() && v.attr("onclick").is_some())) {
                    let raw = href.or(v.attr("onclick")).unwrap_or("");
                    p.links.push(uncrawlable_link(raw, &el, &base, cfg));
                    continue;
                }
                if let Some(href) = href {
                    if cfg.crawl_invalid_links {
                        if let Some(bad) = malformed(href) {
                            let mut link = uncrawlable_link(href, &el, &base, cfg);
                            link.kind = LinkKind::Hyperlink;
                            link.to = bad.to_string();
                            link.anchor = clean_text(&el);
                            p.links.push(link);
                            continue;
                        }
                    }
                    if let Some(mut link) = make_link(LinkKind::Hyperlink, href, &el, &base, cfg) {
                        link.anchor = clean_text(&el);
                        if link.anchor.is_empty() {
                            link.alt = el
                                .descendants()
                                .filter_map(ElementRef::wrap)
                                .find(|i| i.value().name() == "img")
                                .map(|i| i.value().attr("alt").unwrap_or("").trim().to_string());
                        }
                        p.links.push(link);
                    }
                }
            }
            "img" => {
                if let Some(src) = v.attr("src") {
                    if let Some(mut link) = make_link(LinkKind::Image, src, &el, &base, cfg) {
                        link.alt = v.attr("alt").map(|a| a.trim().to_string());
                        p.links.push(link);
                    }
                }
            }
            "script" => {
                if let Some(src) = v.attr("src") {
                    if let Some(link) = make_link(LinkKind::JavaScript, src, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "video" | "audio" => {
                if let Some(src) = v.attr("src") {
                    if let Some(link) = make_link(LinkKind::Media, src, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "source" => {
                let in_media = el.parent().and_then(ElementRef::wrap).is_some_and(|m| matches!(m.value().name(), "video" | "audio"));
                if let (true, Some(src)) = (in_media, v.attr("src")) {
                    if let Some(link) = make_link(LinkKind::Media, src, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "embed" | "object" => {
                if let Some(src) = v.attr(if v.name() == "embed" { "src" } else { "data" }) {
                    if let Some(link) = make_link(LinkKind::Swf, src, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "span" | "div" if cfg.store_uncrawlable_links && v.attr("href").is_some() => {
                p.links.push(uncrawlable_link(v.attr("href").unwrap_or(""), &el, &base, cfg));
            }
            "iframe" => {
                if let Some(src) = v.attr("src") {
                    if let Some(link) = make_link(LinkKind::Iframe, src, &el, &base, cfg) {
                        p.links.push(link);
                    }
                }
            }
            "form" => {
                let action = v.attr("action").unwrap_or("");
                let resolved = if action.trim().is_empty() {
                    Some(page_url.to_string())
                } else {
                    resolve(&base, action).map(|u| u.to_string())
                };
                if let Some(a) = resolved {
                    p.form_actions.push(a);
                }
            }
            _ => {}
        }
        if p.links.len() >= cfg.max_links_per_url {
            break;
        }
    }

    p.h1_non_sequential = heading_levels.first().is_some_and(|&l| l != 1);
    p.h2_non_sequential = heading_levels
        .iter()
        .position(|&l| l == 2)
        .is_some_and(|i| i == 0 || heading_levels[..i].iter().all(|&l| l > 2));

    // Content area text: word count, readability, duplicates.
    let mut content = String::new();
    let mut all_text = String::new();
    let body_el = root.descendants().filter_map(ElementRef::wrap).find(|e| e.value().name() == "body");
    if let Some(body) = body_el {
        let start_included = cfg.content_area.include_elements.is_empty()
            && cfg.content_area.include_classes.is_empty()
            && cfg.content_area.include_ids.is_empty();
        collect_text(body, &cfg.content_area, start_included, &mut content);

        collect_text(body, &ContentArea { exclude_elements: vec![], ..Default::default() }, true, &mut all_text);
        let visible_chars = all_text.split_whitespace().map(|w| w.chars().count() + 1).sum::<usize>();
        if !html.is_empty() {
            p.text_ratio = (visible_chars as f64 / html.len() as f64 * 100.0 * 10.0).round() / 10.0;
        }
    }
    let content = collapse_ws(&content);
    let words: Vec<&str> = content.split_whitespace().collect();
    p.word_count = words.len() as u32;
    p.sentence_count = content
        .split(|c| c == '.' || c == '!' || c == '?')
        .filter(|s| s.split_whitespace().count() > 0)
        .count() as u32;
    if p.word_count >= 1 && p.sentence_count >= 1 {
        let syllables: usize = words.iter().map(|w| syllables(w)).sum();
        let wps = p.word_count as f64 / p.sentence_count as f64;
        let spw = syllables as f64 / p.word_count as f64;
        let score = 206.835 - 1.015 * wps - 84.6 * spw;
        p.flesch_reading_ease = Some((score.clamp(0.0, 100.0) * 10.0).round() / 10.0);
    }
    p.lorem_ipsum = content.to_ascii_lowercase().contains("lorem ipsum");
    // The content signature feeds near duplicates and Compare > Content.
    if cfg.near_duplicates || cfg.store_html {
        p.minhash = minhash::signature(&content);
    }
    if cfg.extract_structured_data {
        p.structured_data = Some(crate::structured::extract(&doc));
    }

    if !rules.is_empty() {
        let head_html = root
            .descendants()
            .filter_map(ElementRef::wrap)
            .find(|e| e.value().name() == "head")
            .map(|h| h.html())
            .unwrap_or_default();
        let body_html = body_el.map(|b| b.html()).unwrap_or_default();
        let page_text = collapse_ws(&all_text);
        let targets = SearchTargets {
            html,
            head_html: &head_html,
            body_html: &body_html,
            page_text: &page_text,
            content_text: &content,
        };
        p.custom_search = run_searches(rules, &targets);
        p.custom_extraction = run_extractors(rules, &doc, html);
    }
    p.content_text = content;

    let (head_re, body_re) = tag_regexes();
    p.head_tags = head_re.find_iter(html).count() as u32;
    p.body_tags = body_re.find_iter(html).count() as u32;
    p
}

fn tag_regexes() -> &'static (Regex, Regex) {
    static RE: OnceLock<(Regex, Regex)> = OnceLock::new();
    RE.get_or_init(|| {
        (Regex::new(r"(?i)<head[\s>]").unwrap(), Regex::new(r"(?i)<body[\s>]").unwrap())
    })
}

fn make_link(kind: LinkKind, href: &str, el: &ElementRef, base: &Url, cfg: &CrawlConfig) -> Option<Link> {
    let to = normalize(resolve(base, href)?, cfg);
    let v = el.value();
    let rel = v.attr("rel").unwrap_or("").trim().to_string();
    let rel_lower = rel.to_ascii_lowercase();
    let follow = !["nofollow", "ugc", "sponsored"].iter().any(|r| rel_lower.split_whitespace().any(|t| t == *r));
    let path = link_path(el);
    let position = LINK_POSITIONS
        .iter()
        .find(|(_, needle)| path.contains(needle))
        .map(|(name, _)| name.to_string())
        .unwrap_or_else(|| "Content".to_string());
    Some(Link {
        kind,
        to: to.to_string(),
        href: href.trim().to_string(),
        anchor: String::new(),
        alt: None,
        follow,
        rel,
        target: v.attr("target").unwrap_or("").to_string(),
        path_type: PathType::of(href).as_str().to_string(),
        link_path: path,
        position,
        hreflang: None,
    })
}

/// A link the spider cannot follow: kept as written (resolved when it is a
/// URL), with the anchor text, for Links > Uncrawlable Internal Outlinks.
fn uncrawlable_link(raw: &str, el: &ElementRef, base: &Url, cfg: &CrawlConfig) -> Link {
    let to = resolve(base, raw).map(|u| normalize(u, cfg).to_string()).unwrap_or_else(|| raw.trim().to_string());
    Link {
        kind: LinkKind::Uncrawlable,
        to,
        href: raw.trim().to_string(),
        anchor: clean_text(el),
        alt: None,
        follow: false,
        rel: el.value().attr("rel").unwrap_or("").to_string(),
        target: String::new(),
        path_type: PathType::of(raw).as_str().to_string(),
        link_path: link_path(el),
        position: "Content".into(),
        hreflang: None,
    }
}

fn in_head(el: &ElementRef) -> bool {
    el.ancestors().any(|a| matches!(a.value(), Node::Element(e) if e.name() == "head"))
}

/// Tag-name path such as `/html/body/nav/ul/li/a`.
fn link_path(el: &ElementRef) -> String {
    let mut names: Vec<&str> = el
        .ancestors()
        .filter_map(|a| match a.value() {
            Node::Element(e) => Some(e.name()),
            _ => None,
        })
        .collect();
    names.reverse();
    names.push(el.value().name());
    format!("/{}", names.join("/"))
}

fn clean_text(el: &ElementRef) -> String {
    collapse_ws(&el.text().collect::<Vec<_>>().join(" "))
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_text(el: ElementRef, area: &ContentArea, included: bool, out: &mut String) {
    for child in el.children() {
        match child.value() {
            Node::Text(t) if included => {
                out.push_str(t);
                out.push(' ');
            }
            Node::Element(e) => {
                let name = e.name();
                if NON_TEXT_ELEMENTS.contains(&name)
                    || area.exclude_elements.iter().any(|x| x == name)
                    || e.id().is_some_and(|id| area.exclude_ids.iter().any(|x| x == id))
                    || e.classes().any(|c| area.exclude_classes.iter().any(|x| x == c))
                {
                    continue;
                }
                let now_included = included
                    || area.include_elements.iter().any(|x| x == name)
                    || e.id().is_some_and(|id| area.include_ids.iter().any(|x| x == id))
                    || e.classes().any(|c| area.include_classes.iter().any(|x| x == c));
                if let Some(child_el) = ElementRef::wrap(child) {
                    collect_text(child_el, area, now_included, out);
                }
            }
            _ => {}
        }
    }
}

/// `5; url=/next` → `/next`.
fn refresh_target(content: &str) -> Option<String> {
    let lower = content.to_ascii_lowercase();
    let i = lower.find("url=")?;
    let target = content[i + 4..].trim().trim_matches(|c| c == '\'' || c == '"');
    (!target.is_empty()).then(|| target.to_string())
}

/// Vowel-group heuristic, adequate for the Flesch score.
fn syllables(word: &str) -> usize {
    let w: Vec<char> = word.to_ascii_lowercase().chars().filter(|c| c.is_ascii_alphabetic()).collect();
    if w.is_empty() {
        return 0;
    }
    let is_vowel = |c: char| "aeiouy".contains(c);
    let mut count = 0;
    let mut prev = false;
    for &c in &w {
        let v = is_vowel(c);
        if v && !prev {
            count += 1;
        }
        prev = v;
    }
    if w.len() > 2 && w[w.len() - 1] == 'e' && !is_vowel(w[w.len() - 2]) && count > 1 {
        count -= 1;
    }
    count.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<!doctype html><html lang="en-GB"><head>
<title>Toad Home</title><meta name="description" content="All about toads.">
<meta name="robots" content="noindex, follow"><link rel="canonical" href="/home">
<link rel="alternate" hreflang="fr" href="https://x.com/fr/"><link rel="stylesheet" href="/s.css">
</head><body><nav><a href="/about">About</a></nav>
<h1>Toads</h1><p>Toads are amphibians. They hop a lot!</p>
<a href="/img-link"><img src="/t.png"></a><img src="/t2.png" alt="A toad">
<a href="https://other.com/" rel="nofollow sponsored" target="_blank">Other</a>
<a href="mailto:x@y.com">mail</a><footer><a href="/privacy">Privacy</a></footer></body></html>"#;

    #[test]
    fn extracts_new_link_kinds() {
        let html = r#"<html><head>
            <link rel="amphtml" href="/amp">
            <link rel="alternate" media="only screen and (max-width: 640px)" href="https://m.x.com/">
          </head><body>
            <video src="/v.mp4"></video><audio><source src="/a.mp3"></audio>
            <embed src="/f.swf"><object data="/o.swf"></object>
            <span href="/span-link">S</span><div href="/div-link">D</div>
            <a onclick="go()">Click</a><a href="javascript:void(0)">JS</a>
            <a href="hppts://x.com/typo">Typo</a><a href="/ok">OK</a>
          </body></html>"#;
        let url = Url::parse("https://x.com/").unwrap();
        let kinds = |cfg: &CrawlConfig| {
            let p = extract(html, &url, cfg, &CustomRules::default());
            p.links.iter().map(|l| (l.kind.as_str(), l.to.clone())).collect::<Vec<_>>()
        };
        let base = kinds(&CrawlConfig::default());
        let has = |v: &[(&str, String)], k: &str, to: &str| v.iter().any(|(kk, t)| *kk == k && t == to);
        assert!(has(&base, "AMP", "https://x.com/amp"));
        assert!(has(&base, "Mobile Alternate", "https://m.x.com/"));
        assert!(has(&base, "Media", "https://x.com/v.mp4") && has(&base, "Media", "https://x.com/a.mp3"));
        assert!(has(&base, "SWF", "https://x.com/f.swf") && has(&base, "SWF", "https://x.com/o.swf"));
        assert!(!base.iter().any(|(k, _)| *k == "Uncrawlable"), "uncrawlable links are off by default");
        assert!(!base.iter().any(|(_, t)| t.starts_with("hppts")), "invalid links are off by default");
        assert!(has(&base, "Hyperlink", "https://x.com/ok"));

        let on = CrawlConfig { store_uncrawlable_links: true, crawl_invalid_links: true, ..CrawlConfig::default() };
        let all = kinds(&on);
        let uncrawlable: Vec<&String> = all.iter().filter(|(k, _)| *k == "Uncrawlable").map(|(_, t)| t).collect();
        assert_eq!(uncrawlable.len(), 4, "{all:?}");
        assert!(uncrawlable.contains(&&"https://x.com/span-link".to_string()));
        assert!(uncrawlable.contains(&&"javascript:void(0)".to_string()));
        assert!(has(&all, "Hyperlink", "hppts://x.com/typo"));
    }

    #[test]
    fn extracts_core_elements() {
        let url = Url::parse("https://x.com/").unwrap();
        let p = extract(HTML, &url, &CrawlConfig::default(), &CustomRules::default());
        assert_eq!(p.titles, vec!["Toad Home"]);
        assert_eq!(p.meta_descriptions, vec!["All about toads."]);
        assert_eq!(p.h1, vec!["Toads"]);
        assert_eq!(p.canonicals, vec!["https://x.com/home"]);
        assert!(p.canonical_is_relative);
        assert_eq!(p.language.as_deref(), Some("en-GB"));
        assert_eq!(p.hreflang, vec![("fr".to_string(), "https://x.com/fr/".to_string())]);
        // nav and footer are excluded from the content area.
        assert!(!p.content_text.contains("About"));
        assert!(!p.content_text.contains("Privacy"));
        assert!(p.content_text.contains("Toads are amphibians"));

        let about = p.links.iter().find(|l| l.to == "https://x.com/about").unwrap();
        assert_eq!(about.position, "Navigation");
        assert_eq!(about.link_path, "/html/body/nav/a");
        let ext = p.links.iter().find(|l| l.to == "https://other.com/").unwrap();
        assert!(!ext.follow);
        let img_link = p.links.iter().find(|l| l.to == "https://x.com/img-link").unwrap();
        assert_eq!(img_link.alt.as_deref(), Some(""));
        let img = p.links.iter().find(|l| l.to == "https://x.com/t.png").unwrap();
        assert_eq!(img.alt, None);
        assert!(!p.links.iter().any(|l| l.href.starts_with("mailto")));
        assert_eq!(p.head_tags, 1);
    }

    #[test]
    fn title_in_body_is_flagged() {
        let url = Url::parse("https://x.com/").unwrap();
        let p = extract("<html><head><div>x</div><title>T</title></head><body></body></html>", &url, &CrawlConfig::default(), &CustomRules::default());
        assert!(p.title_outside_head);
    }
}
