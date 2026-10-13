//! XPath 1.0 for Custom Search (Search In: XPath) and Custom Extraction.
//!
//! The page is parsed as HTML (html5ever, via scraper) and copied into an
//! sxd-document tree, which sxd-xpath evaluates. Element names are the
//! lowercase HTML names with no namespace, so `//div[@class='x']` works as
//! people expect. XPath 2.0/3.1 functions are not available.

use crate::config::ExtractWhat;
use scraper::{ElementRef, Html, Node};
use sxd_document::dom::{ChildOfElement, Document, Element};
use sxd_document::Package;
use sxd_xpath::nodeset::Node as XNode;
use sxd_xpath::{Context, Factory, Value};

/// Check an expression compiles (the validity tick in the UI).
pub fn validate(expression: &str) -> Result<(), String> {
    match Factory::new().build(expression) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err("empty XPath expression".into()),
        Err(e) => Err(format!("invalid XPath: {e}")),
    }
}

/// An HTML page as an XPath-queryable document.
pub struct XPathDoc {
    package: Package,
}

const VOID: [&str; 14] = ["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];

impl XPathDoc {
    pub fn from_html(doc: &Html) -> Self {
        let package = Package::new();
        {
            let d = package.as_document();
            let el = copy_element(&d, doc.root_element());
            d.root().append_child(el);
        }
        Self { package }
    }

    /// Evaluate `expression` and turn the result into strings:
    /// - node sets: one value per node, as `what` asks (attribute nodes give
    ///   their value; `attribute` reads an attribute of each element);
    /// - strings, numbers and booleans (functions such as `count()`): one value.
    pub fn values(&self, expression: &str, what: ExtractWhat, attribute: Option<&str>, limit: usize) -> Result<Vec<String>, String> {
        let xpath = match Factory::new().build(expression) {
            Ok(Some(x)) => x,
            Ok(None) => return Ok(vec![]),
            Err(e) => return Err(format!("invalid XPath: {e}")),
        };
        let doc = self.package.as_document();
        let context = Context::new();
        let value = xpath.evaluate(&context, doc.root()).map_err(|e| format!("XPath error: {e}"))?;
        Ok(match value {
            Value::Nodeset(ns) => ns
                .document_order()
                .into_iter()
                .filter_map(|n| node_value(n, what, attribute))
                .filter(|v| !v.is_empty())
                .take(limit)
                .collect(),
            Value::String(s) => vec![s],
            Value::Number(n) => vec![if n.fract() == 0.0 && n.abs() < 1e15 { format!("{}", n as i64) } else { n.to_string() }],
            Value::Boolean(b) => vec![b.to_string()],
        })
    }
}

fn node_value(n: XNode, what: ExtractWhat, attribute: Option<&str>) -> Option<String> {
    match n {
        XNode::Element(e) => match attribute {
            Some(a) => e.attribute_value(a).map(|v| v.trim().to_string()),
            None => Some(match what {
                ExtractWhat::Element => outer_html(e),
                ExtractWhat::InnerHtml => inner_html(e).trim().to_string(),
                ExtractWhat::Text | ExtractWhat::FunctionValue => collapse(&n.string_value()),
            }),
        },
        XNode::Attribute(a) => Some(a.value().trim().to_string()),
        other => Some(collapse(&other.string_value())),
    }
}

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn copy_element<'d>(d: &Document<'d>, src: ElementRef) -> Element<'d> {
    let el = d.create_element(src.value().name());
    for (name, value) in src.value().attrs() {
        // Namespace declarations would change how names resolve; skip them.
        if name == "xmlns" || name.starts_with("xmlns:") || name.contains(':') {
            continue;
        }
        el.set_attribute_value(name, value);
    }
    for child in src.children() {
        match child.value() {
            Node::Element(_) => {
                if let Some(c) = ElementRef::wrap(child) {
                    el.append_child(copy_element(d, c));
                }
            }
            Node::Text(t) => el.append_child(d.create_text(t)),
            Node::Comment(c) => el.append_child(d.create_comment(c)),
            _ => {}
        }
    }
    el
}

fn escape(s: &str, attr: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn outer_html(e: Element) -> String {
    let name = e.name().local_part().to_string();
    let mut out = format!("<{name}");
    for a in e.attributes() {
        out.push_str(&format!(" {}=\"{}\"", a.name().local_part(), escape(a.value(), true)));
    }
    out.push('>');
    if VOID.contains(&name.as_str()) {
        return out;
    }
    out.push_str(&inner_html(e));
    out.push_str(&format!("</{name}>"));
    out
}

fn inner_html(e: Element) -> String {
    let raw_text = matches!(e.name().local_part(), "script" | "style");
    let mut out = String::new();
    for c in e.children() {
        match c {
            ChildOfElement::Element(child) => out.push_str(&outer_html(child)),
            ChildOfElement::Text(t) => out.push_str(&if raw_text { t.text().to_string() } else { escape(t.text(), false) }),
            ChildOfElement::Comment(c) => out.push_str(&format!("<!--{}-->", c.text())),
            ChildOfElement::ProcessingInstruction(_) => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r#"<html><head><title>T</title></head><body>
        <div class="price"><span>£</span>20</div>
        <h3>First</h3><h3>Second <b>bold</b></h3>
        <a href="/a">A</a><a href="/b">B</a>
        <script>var x = 1 < 2;</script>
    </body></html>"#;

    #[test]
    fn evaluates_xpath_over_html() {
        let doc = XPathDoc::from_html(&Html::parse_document(HTML));
        let v = |x: &str, w: ExtractWhat| doc.values(x, w, None, 100).unwrap();
        assert_eq!(v("//h3", ExtractWhat::Text), vec!["First", "Second bold"]);
        assert_eq!(v("//h3[2]", ExtractWhat::InnerHtml), vec!["Second <b>bold</b>"]);
        assert_eq!(v("//div[@class='price']", ExtractWhat::Element), vec![r#"<div class="price"><span>£</span>20</div>"#]);
        assert_eq!(v("//a/@href", ExtractWhat::Text), vec!["/a", "/b"]);
        assert_eq!(v("count(//a)", ExtractWhat::FunctionValue), vec!["2"]);
        assert_eq!(v("string-length(//title)", ExtractWhat::FunctionValue), vec!["1"]);
        assert_eq!(v("//h3[1] | //title", ExtractWhat::Text), vec!["T", "First"], "union, in document order");
        assert_eq!(doc.values("//a", ExtractWhat::Text, Some("href"), 100).unwrap(), vec!["/a", "/b"]);
        assert_eq!(v("//script", ExtractWhat::InnerHtml), vec!["var x = 1 < 2;"]);
        assert!(validate("//div[").is_err());
        assert!(validate("//div[@id='x']").is_ok());
    }
}
