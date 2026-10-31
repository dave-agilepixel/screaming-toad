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
        // Bookmark targets: any id, and the old <a name>.
        for target in [v.attr("id"), if name == "a" { v.attr("name") } else { None }].into_iter().flatten() {
            let target = target.trim();
            if !target.is_empty() {
                p.anchor_ids.push(target.to_string());
            }
        }
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
                    "fragment" => p.ajax_fragment_meta |= content == "!",
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
                        if let Some(fragment) = bookmark_fragment(href) {
                            let target = link.to.split('#').next().unwrap_or(&link.to).to_string();
                            p.bookmarks.push((target, fragment));
                        }
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
                let alt = v.attr("alt").map(|a| a.trim().to_string());
                if let Some(src) = image_src(&el) {
                    if let Some(mut link) = make_link(LinkKind::Image, src, &el, &base, cfg) {
                        link.alt = alt.clone();
                        if v.attr("width").is_none() || v.attr("height").is_none() {
                            p.unsized_images.push(link.to.clone());
                        }
                        p.links.push(link);
                    }
                }
                if cfg.extract_images_from_srcset {
                    for src in srcset_urls(image_srcset(&el)) {
                        if let Some(mut link) = make_link(LinkKind::Image, src, &el, &base, cfg) {
                            link.alt = alt.clone();
                            p.links.push(link);
                        }
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
            "source" if cfg.extract_images_from_srcset && el.parent().and_then(ElementRef::wrap).is_some_and(|m| m.value().name() == "picture") => {
                for src in srcset_urls(image_srcset(&el)) {
                    if let Some(link) = make_link(LinkKind::Image, src, &el, &base, cfg) {
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

    let (h1_non_sequential, h2_non_sequential) = heading_order(&heading_levels);
    p.h1_non_sequential = h1_non_sequential;
    p.h2_non_sequential = h2_non_sequential;

    // Content area text: word count, readability, duplicates.
    let mut content = String::new();
    let mut all_text = String::new();
    let body_el = root.descendants().filter_map(ElementRef::wrap).find(|e| e.value().name() == "body");
    if let Some(body) = body_el {
        let area = cfg.content_area.effective();
        let start_included = area.include_elements.is_empty() && area.include_classes.is_empty() && area.include_ids.is_empty();
        collect_text(body, &area, start_included, &mut content);

        collect_text(body, &ContentArea { exclude_elements: vec![], ..Default::default() }, true, &mut all_text);
        let visible_chars = all_text.split_whitespace().map(|w| w.chars().count() + 1).sum::<usize>();
        if !html.is_empty() {
            p.text_ratio = (visible_chars as f64 / html.len() as f64 * 100.0 * 10.0).round() / 10.0;
        }
    }
    let content = collapse_ws(&content);
    text_metrics(&mut p, &content);
    // The content signature feeds near duplicates and Compare > Content.
    if cfg.near_duplicates || cfg.store_html {
        p.minhash = minhash::signature(&content);
    }
    if cfg.structured_data.any_format() {
        p.structured_data = Some(crate::structured::extract(&doc, &cfg.structured_data));
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
        let mut no_anchors = String::new();
        if let Some(body) = body_el {
            collect_text(body, &ContentArea { exclude_elements: vec!["a".into()], ..Default::default() }, true, &mut no_anchors);
        }
        let page_text_no_anchors = collapse_ws(&no_anchors);
        let xdoc = rules.needs_xpath().then(|| crate::xpath::XPathDoc::from_html(&doc));
        let targets = SearchTargets {
            html,
            head_html: &head_html,
            body_html: &body_html,
            page_text: &page_text,
            page_text_no_anchors: &page_text_no_anchors,
            content_text: &content,
            xpath: xdoc.as_ref(),
        };
        p.custom_search = run_searches(rules, &targets);
        p.custom_extraction = run_extractors(rules, &doc, html, xdoc.as_ref());
    }
    p.content_text = content;
    if cfg.apis.needs_text().0 {
        p.page_text = collapse_ws(&all_text);
    }

    let raw = raw_structure(html);
    p.body_before_html = raw.body_before_html;
    p.head_not_first = raw.head_not_first;
    p.invalid_head_elements = raw.invalid_head_elements;

    let (head_re, body_re) = tag_regexes();
    p.head_tags = head_re.find_iter(html).count() as u32;
    p.body_tags = body_re.find_iter(html).count() as u32;
    p
}

/// Word count, sentence count, Flesch reading ease and lorem ipsum for a
/// text (HTML content area or PDF text).
pub fn text_metrics(p: &mut PageData, content: &str) {
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
}

/// The fragment of an href that points at a bookmark on a page: not empty,
/// "top" (which always scrolls to the top), a hashbang or SPA route, a text
/// fragment, or a key=value state string.
fn bookmark_fragment(href: &str) -> Option<String> {
    let (_, fragment) = href.trim().split_once('#')?;
    let fragment = percent_decoded(fragment);
    let skip = fragment.is_empty()
        || fragment.eq_ignore_ascii_case("top")
        || fragment.starts_with(['!', '/', ':'])
        || fragment.contains('=');
    (!skip).then_some(fragment)
}

/// `%20` and friends decoded (ids are matched as written).
fn percent_decoded(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        match (b[i], b.get(i + 1).copied().and_then(hex), b.get(i + 2).copied().and_then(hex)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, ..) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Elements Google accepts in the `<head>`; anything else ends it early.
const HEAD_ELEMENTS: [&str; 8] = ["title", "meta", "link", "script", "style", "base", "noscript", "template"];

#[derive(Debug, Default, PartialEq)]
struct RawStructure {
    body_before_html: bool,
    head_not_first: bool,
    invalid_head_elements: Vec<String>,
}

/// Validation checks on the source as written (the parser repairs these, so
/// they can't be seen in the parsed document): the order of the html, head
/// and body tags, and elements in the head that don't belong there.
fn raw_structure(html: &str) -> RawStructure {
    // (closing, lowercase name) of every tag, skipping comments, doctypes
    // and the contents of raw-text elements.
    let mut tags: Vec<(bool, String)> = Vec::new();
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut i = 0;
    while let Some(off) = lower[i..].find('<') {
        let at = i + off;
        let rest = &lower[at..];
        if rest.starts_with("<!--") {
            i = rest.find("-->").map_or(lower.len(), |e| at + e + 3);
            continue;
        }
        let closing = rest.starts_with("</");
        let start = at + if closing { 2 } else { 1 };
        let len = bytes[start..].iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'-' || **b == b':').count();
        if len == 0 || !bytes[start].is_ascii_alphabetic() {
            i = at + 1;
            continue;
        }
        let name = &lower[start..start + len];
        // The end of the tag, past any quoted attribute values.
        let mut j = start + len;
        let mut quote = None;
        while j < bytes.len() {
            match (quote, bytes[j]) {
                (None, b'>') => break,
                (None, q @ (b'"' | b'\'')) => quote = Some(q),
                (Some(q), c) if c == q => quote = None,
                _ => {}
            }
            j += 1;
        }
        tags.push((closing, name.to_string()));
        i = (j + 1).min(lower.len());
        if !closing && matches!(name, "script" | "style" | "textarea" | "title" | "noscript" | "template" | "xmp") {
            i = lower[i..].find(&format!("</{name}")).map_or(lower.len(), |e| i + e);
        }
    }
    let first = |n: &str| tags.iter().position(|(c, t)| !c && t == n);
    let mut out = RawStructure::default();
    if let (Some(h), Some(b)) = (first("html"), first("body")) {
        out.body_before_html = b < h;
    }
    if let (Some(h), Some(head)) = (first("html"), first("head")) {
        out.head_not_first = tags[h + 1..].iter().find(|(c, _)| !c).is_some_and(|(_, t)| t != "head") && head > h;
    }
    if let Some(head) = first("head") {
        for (closing, t) in &tags[head + 1..] {
            if t == "body" || (*closing && t == "head") {
                break;
            }
            if !closing && !HEAD_ELEMENTS.contains(&t.as_str()) && !out.invalid_head_elements.contains(t) {
                out.invalid_head_elements.push(t.clone());
            }
        }
    }
    out
}

/// H1 and H2 Non-Sequential from the heading levels in document order.
/// H1: the page has an H1, but it isn't the first heading. H2: the page has
/// an H2, but the heading level after the H1 isn't 2: an H2 comes before
/// any H1, or the H1 is followed by an H3-H6 before the first H2.
fn heading_order(levels: &[u8]) -> (bool, bool) {
    let h1 = levels.iter().position(|&l| l == 1);
    let h2 = levels.iter().position(|&l| l == 2);
    let h1_non_sequential = h1.is_some_and(|i| i > 0);
    let h2_non_sequential = match (h1, h2) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(i), Some(j)) => j < i || levels[i + 1..j].iter().any(|&l| l > 2),
    };
    (h1_non_sequential, h2_non_sequential)
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
    // Custom > Link Positions: off skips the path entirely (saves memory).
    let (path, position) = if cfg.link_positions_enabled {
        let path = link_path(el);
        let position = crate::config::LinkPosition::classify(&cfg.link_positions, &path).unwrap_or("").to_string();
        (path, position)
    } else {
        (String::new(), String::new())
    };
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
        origin: String::new(),
    })
}

/// Lazy-load plugins (LiteSpeed, WP Rocket, lazysizes) keep the real image
/// in data-src or data-lazy-src and put a placeholder (often a data: URI) in src.
const LAZY_SRC: [&str; 2] = ["data-src", "data-lazy-src"];
const LAZY_SRCSET: [&str; 2] = ["data-srcset", "data-lazy-srcset"];

/// An image's real src: src, unless it is missing or a data: placeholder
/// and a lazy-load attribute holds a URL.
fn image_src<'a>(el: &ElementRef<'a>) -> Option<&'a str> {
    let v = el.value();
    let src = v.attr("src").filter(|s| !s.trim().is_empty());
    let placeholder = src.is_none_or(|s| s.trim_start().get(..5).is_some_and(|p| p.eq_ignore_ascii_case("data:")));
    let lazy = LAZY_SRC.iter().find_map(|a| v.attr(a)).filter(|s| !s.trim().is_empty());
    if placeholder { lazy.or(src) } else { src }
}

/// An image's srcset, falling back to the lazy-load attributes.
fn image_srcset<'a>(el: &ElementRef<'a>) -> &'a str {
    let v = el.value();
    v.attr("srcset")
        .filter(|s| !s.trim().is_empty())
        .or_else(|| LAZY_SRCSET.iter().find_map(|a| v.attr(a)))
        .unwrap_or("")
}

/// The URLs in a srcset attribute ("a.jpg 1x, b.jpg 2x" or "a.jpg 480w, ...").
fn srcset_urls(srcset: &str) -> Vec<&str> {
    srcset.split(',').filter_map(|c| c.split_whitespace().next()).filter(|u| !u.is_empty()).collect()
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
        link_path: if cfg.link_positions_enabled { link_path(el) } else { String::new() },
        position: if cfg.link_positions_enabled {
            crate::config::LinkPosition::classify(&cfg.link_positions, &link_path(el)).unwrap_or("").to_string()
        } else {
            String::new()
        },
        hreflang: None,
        origin: String::new(),
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
                if name == "img" && included && area.check_alt_text {
                    if let Some(alt) = e.attr("alt").map(str::trim).filter(|a| !a.is_empty()) {
                        out.push_str(alt);
                        out.push(' ');
                    }
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
pub(crate) fn refresh_target(content: &str) -> Option<String> {
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
    fn content_area_modes_and_alt_text() {
        use crate::config::{AreaMode, ContentArea};
        let html = r#"<html><body><nav>Menu words</nav><main>Main copy <img src="/a.png" alt="A toad"></main><footer>Foot text</footer></body></html>"#;
        let url = Url::parse("https://x.com/").unwrap();
        let text = |area: ContentArea| extract(html, &url, &CrawlConfig { content_area: area, ..CrawlConfig::default() }, &CustomRules::default()).content_text;
        // Exclude (default): nav and footer left out; include lists ignored.
        let ex = text(ContentArea { include_elements: vec!["footer".into()], ..ContentArea::default() });
        assert_eq!(ex, "Main copy");
        // Include: only the include lists apply.
        let inc = text(ContentArea { mode: AreaMode::Include, include_elements: vec!["main".into()], ..ContentArea::default() });
        assert_eq!(inc, "Main copy");
        // Both: include main, exclude nothing else of note.
        let both = text(ContentArea { mode: AreaMode::Both, include_elements: vec!["nav".into()], ..ContentArea::default() });
        assert_eq!(both, "", "nav is excluded as well as included, so nothing is left");
        // Check Alt Text adds image alt text.
        let alt = text(ContentArea { check_alt_text: true, ..ContentArea::default() });
        assert_eq!(alt, "Main copy A toad");
    }

    #[test]
    fn srcset_images_when_enabled() {
        let html = r#"<html><body><img src="/a.jpg" srcset="/a-2x.jpg 2x, /a-480.jpg 480w" alt="A">
            <picture><source srcset="/p.webp 1x"><img src="/p.jpg"></picture></body></html>"#;
        let url = Url::parse("https://x.com/").unwrap();
        let images = |on: bool| {
            let cfg = CrawlConfig { extract_images_from_srcset: on, ..CrawlConfig::default() };
            let mut v: Vec<String> = extract(html, &url, &cfg, &CustomRules::default())
                .links
                .into_iter()
                .filter(|l| l.kind == LinkKind::Image)
                .map(|l| l.to)
                .collect();
            v.sort();
            v
        };
        assert_eq!(images(false), vec!["https://x.com/a.jpg", "https://x.com/p.jpg"]);
        assert_eq!(images(true).len(), 5);
        assert!(images(true).contains(&"https://x.com/p.webp".to_string()));
    }

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
    fn lazy_loaded_images_use_the_real_url() {
        let html = r#"<html><body>
            <img src="data:image/svg+xml;base64,PHN2Zz4=" data-src="/real.webp" alt="">
            <img src="data:image/gif;base64,R0lGOD" data-lazy-src="/wp.jpg" data-lazy-srcset="/wp-2x.jpg 2x" alt="WP Rocket">
            <img data-src="/no-src.png">
            <img src="/plain.png" data-src="/ignored.png" alt="Plain">
            <nav><img src="data:image/svg+xml,x" data-src="/logo.png" data-srcset="/logo-2x.png 2x"></nav>
          </body></html>"#;
        let url = Url::parse("https://x.com/").unwrap();
        let p = extract(html, &url, &CrawlConfig { extract_images_from_srcset: true, ..CrawlConfig::default() }, &CustomRules::default());
        let img = |to: &str| p.links.iter().find(|l| l.kind == LinkKind::Image && l.to == format!("https://x.com{to}"));
        assert_eq!(img("/real.webp").unwrap().alt.as_deref(), Some(""), "empty alt kept: Missing Alt Text");
        assert_eq!(img("/wp.jpg").unwrap().alt.as_deref(), Some("WP Rocket"));
        assert!(img("/wp-2x.jpg").is_some(), "data-lazy-srcset");
        assert_eq!(img("/no-src.png").unwrap().alt, None, "no alt attribute: Missing Alt Attribute");
        assert!(img("/plain.png").is_some() && img("/ignored.png").is_none(), "a real src wins");
        assert_eq!(img("/logo.png").unwrap().position, "Navigation");
        assert!(img("/logo-2x.png").is_some(), "data-srcset");
        assert!(!p.links.iter().any(|l| l.to.starts_with("data:")));
    }

    #[test]
    fn heading_order_matches_screaming_frog() {
        // (levels, H1 Non-Sequential, H2 Non-Sequential)
        let cases: [(&[u8], bool, bool); 9] = [
            (&[1, 2, 3, 2], false, false),
            (&[1, 3, 2], false, true),   // H3 between the H1 and the first H2
            (&[1, 4, 4, 2], false, true),
            (&[2, 1, 2], true, true),    // H2 before the H1
            (&[3, 1, 2], true, false),   // H1 not first, but H2 follows it
            (&[2, 3], false, true),      // H2 with no H1
            (&[1, 3, 4], false, false),  // no H2: H2 Missing, not Non-Sequential
            (&[3, 4], false, false),     // no H1: H1 Missing, not Non-Sequential
            (&[1, 1, 2], false, false),
        ];
        for (levels, h1, h2) in cases {
            assert_eq!(heading_order(levels), (h1, h2), "{levels:?}");
        }
        let url = Url::parse("https://x.com/").unwrap();
        let p = extract("<html><body><h1>A</h1><h3>B</h3><h2>C</h2></body></html>", &url, &CrawlConfig::default(), &CustomRules::default());
        assert!(!p.h1_non_sequential && p.h2_non_sequential);
    }

    #[test]
    fn raw_source_validation() {
        let ok = raw_structure("<!doctype html><!-- <body> --><html lang=en><head><title>T</title><meta charset=utf-8><script>if (a<b) document.write('<div>')</script><noscript><img src=x></noscript></head><body><div>x</div></body></html>");
        assert_eq!(ok, RawStructure::default());
        let bad = raw_structure("<body><html><div>early</div><head><title>T</title><div class='a>b'>x</div><span>y</span><div>z</div></head></html>");
        assert!(bad.body_before_html && bad.head_not_first);
        assert_eq!(bad.invalid_head_elements, vec!["div", "span"]);
        // The head ends at the body tag even if it is never closed.
        let unclosed = raw_structure("<html><head><title>T</title><body><div>x</div></body></html>");
        assert!(unclosed.invalid_head_elements.is_empty() && !unclosed.head_not_first);
        // No head at all is Missing <head> Tag, not Not First.
        assert!(!raw_structure("<html><body></body></html>").head_not_first);

        let url = Url::parse("https://x.com/").unwrap();
        let p = extract("<html><head><title>T</title><p>Oops</p></head><body></body></html>", &url, &CrawlConfig::default(), &CustomRules::default());
        assert_eq!(p.invalid_head_elements, vec!["p"]);
    }

    #[test]
    fn bookmarks_ids_and_unsized_images() {
        let html = r##"<html><body><h2 id="rates">Rates</h2><a name="old-style"></a>
            <a href="#rates">Rates</a><a href="/other#Book%20now">Book</a><a href="#top">Top</a><a href="#">Nothing</a>
            <a href="/app#/route">SPA</a><a href="/p#!x">Bang</a><a href="/p#page=2">State</a>
            <img src="/a.png" width="10" height="10"><img src="/b.png" width="10"><img src="/c.png">
            <img src="data:image/svg+xml,x" data-src="/d.png"></body></html>"##;
        let url = Url::parse("https://x.com/page").unwrap();
        let p = extract(html, &url, &CrawlConfig::default(), &CustomRules::default());
        assert_eq!(p.anchor_ids, vec!["rates", "old-style"]);
        assert_eq!(
            p.bookmarks,
            vec![("https://x.com/page".to_string(), "rates".to_string()), ("https://x.com/other".to_string(), "Book now".to_string())]
        );
        assert_eq!(p.unsized_images, vec!["https://x.com/b.png", "https://x.com/c.png", "https://x.com/d.png"]);
    }

    #[test]
    fn title_in_body_is_flagged() {
        let url = Url::parse("https://x.com/").unwrap();
        let p = extract("<html><head><div>x</div><title>T</title></head><body></body></html>", &url, &CrawlConfig::default(), &CustomRules::default());
        assert!(p.title_outside_head);
    }
}
