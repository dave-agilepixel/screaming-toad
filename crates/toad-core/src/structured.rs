//! Structured data: JSON-LD and Microdata extraction, plus Google rich
//! result validation (required properties are errors, recommended ones are
//! warnings), following Google Search Central's structured data docs.

use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdItem {
    /// "JSON-LD" or "Microdata".
    pub format: String,
    pub types: Vec<String>,
    /// Top-level property names.
    pub properties: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdIssue {
    pub item_type: String,
    pub severity: Severity,
    pub property: String,
    pub message: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuredData {
    pub items: Vec<SdItem>,
    pub parse_errors: Vec<String>,
    pub issues: Vec<SdIssue>,
}

/// One rule per rich result type. `any_of` lists alternatives where at
/// least one must be present (for example Product needs offers, review or
/// aggregateRating).
struct Rule {
    types: &'static [&'static str],
    required: &'static [&'static str],
    any_of: &'static [&'static str],
    recommended: &'static [&'static str],
}

const RULES: &[Rule] = &[
    Rule { types: &["Article", "NewsArticle", "BlogPosting"], required: &[], any_of: &[], recommended: &["headline", "image", "datePublished", "dateModified", "author"] },
    Rule { types: &["Product"], required: &["name"], any_of: &["offers", "review", "aggregateRating"], recommended: &["image", "description", "sku", "brand"] },
    Rule { types: &["Offer"], required: &["price", "priceCurrency"], any_of: &[], recommended: &["availability", "url"] },
    Rule { types: &["AggregateRating"], required: &["ratingValue"], any_of: &["ratingCount", "reviewCount"], recommended: &["bestRating", "worstRating"] },
    Rule { types: &["Review"], required: &["author", "itemReviewed"], any_of: &[], recommended: &["reviewRating", "datePublished"] },
    Rule { types: &["BreadcrumbList"], required: &["itemListElement"], any_of: &[], recommended: &[] },
    Rule { types: &["FAQPage"], required: &["mainEntity"], any_of: &[], recommended: &[] },
    Rule { types: &["Organization", "Corporation"], required: &[], any_of: &[], recommended: &["name", "url", "logo"] },
    Rule { types: &["LocalBusiness", "Restaurant", "Store"], required: &["name", "address"], any_of: &[], recommended: &["telephone", "geo", "openingHoursSpecification", "url", "priceRange"] },
    Rule { types: &["Recipe"], required: &["name", "image"], any_of: &[], recommended: &["author", "datePublished", "description", "recipeIngredient", "recipeInstructions", "totalTime", "aggregateRating"] },
    Rule { types: &["Event"], required: &["name", "startDate", "location"], any_of: &[], recommended: &["description", "endDate", "image", "offers", "organizer", "eventStatus"] },
    Rule { types: &["VideoObject"], required: &["name", "thumbnailUrl", "uploadDate"], any_of: &[], recommended: &["description", "contentUrl", "duration", "embedUrl"] },
    Rule { types: &["JobPosting"], required: &["title", "description", "datePosted", "hiringOrganization"], any_of: &["jobLocation", "applicantLocationRequirements"], recommended: &["validThrough", "employmentType", "baseSalary"] },
    Rule { types: &["Course"], required: &["name", "description"], any_of: &[], recommended: &["provider"] },
    Rule { types: &["SoftwareApplication", "MobileApplication", "WebApplication"], required: &["name"], any_of: &["offers"], recommended: &["aggregateRating", "operatingSystem", "applicationCategory"] },
    Rule { types: &["Book"], required: &["name", "author"], any_of: &[], recommended: &["url", "sameAs", "isbn"] },
    Rule { types: &["Dataset"], required: &["name", "description"], any_of: &[], recommended: &["url", "license", "creator"] },
];

pub fn extract(doc: &Html) -> StructuredData {
    let mut out = StructuredData::default();
    for el in doc.root_element().descendants().filter_map(ElementRef::wrap) {
        let v = el.value();
        if v.name() == "script" && v.attr("type").is_some_and(|t| t.trim().eq_ignore_ascii_case("application/ld+json")) {
            let text: String = el.text().collect();
            match serde_json::from_str::<Value>(text.trim()) {
                Ok(json) => {
                    let has_context = json_has_schema_context(&json);
                    let mut nodes = Vec::new();
                    flatten(&json, &mut nodes);
                    if !nodes.is_empty() && !has_context {
                        out.issues.push(SdIssue {
                            item_type: "JSON-LD".into(),
                            severity: Severity::Warning,
                            property: "@context".into(),
                            message: "No schema.org @context.".into(),
                        });
                    }
                    for node in nodes {
                        validate_json(node, &mut out.issues);
                        out.items.push(SdItem {
                            format: "JSON-LD".into(),
                            types: types_of(node),
                            properties: node.as_object().map(|o| o.keys().filter(|k| !k.starts_with('@')).cloned().collect()).unwrap_or_default(),
                        });
                    }
                }
                Err(e) => out.parse_errors.push(format!("JSON-LD: {e}")),
            }
        }
        // Top-level Microdata items: itemscope not itself a property of another item.
        if v.attr("itemscope").is_some() && v.attr("itemprop").is_none() {
            let types: Vec<String> = v
                .attr("itemtype")
                .unwrap_or("")
                .split_whitespace()
                .map(short_type)
                .filter(|t| !t.is_empty())
                .collect();
            let properties = microdata_properties(el);
            for t in &types {
                validate(t, |p| properties.iter().any(|x| x == p), &mut out.issues);
            }
            out.items.push(SdItem { format: "Microdata".into(), types, properties });
        }
    }
    out
}

/// Every typed node in a JSON-LD document: top level, arrays, @graph and
/// nested objects (an Offer inside a Product is validated too).
fn flatten<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    match v {
        Value::Array(items) => items.iter().for_each(|i| flatten(i, out)),
        Value::Object(map) => {
            if map.contains_key("@type") {
                out.push(v);
            }
            for (k, child) in map {
                if k != "@context" {
                    flatten(child, out);
                }
            }
        }
        _ => {}
    }
}

fn json_has_schema_context(v: &Value) -> bool {
    match v {
        Value::Array(items) => items.iter().all(json_has_schema_context),
        Value::Object(map) => map.get("@context").is_some_and(|c| c.to_string().contains("schema.org")),
        _ => false,
    }
}

fn types_of(node: &Value) -> Vec<String> {
    match node.get("@type") {
        Some(Value::String(s)) => vec![short_type(s)],
        Some(Value::Array(a)) => a.iter().filter_map(|t| t.as_str()).map(short_type).collect(),
        _ => vec![],
    }
}

/// `https://schema.org/Product` → `Product`.
fn short_type(t: &str) -> String {
    t.trim().rsplit(['/', '#', ':']).next().unwrap_or("").to_string()
}

fn validate_json(node: &Value, issues: &mut Vec<SdIssue>) {
    let has = |p: &str| match node.get(p) {
        None | Some(Value::Null) => false,
        Some(Value::String(s)) => !s.trim().is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(_) => true,
    };
    for t in types_of(node) {
        validate(&t, has, issues);
    }
}

fn validate(item_type: &str, has: impl Fn(&str) -> bool, issues: &mut Vec<SdIssue>) {
    let Some(rule) = RULES.iter().find(|r| r.types.contains(&item_type)) else { return };
    for p in rule.required {
        if !has(p) {
            issues.push(SdIssue {
                item_type: item_type.into(),
                severity: Severity::Error,
                property: p.to_string(),
                message: format!("Missing required property '{p}'."),
            });
        }
    }
    if !rule.any_of.is_empty() && !rule.any_of.iter().any(|p| has(p)) {
        issues.push(SdIssue {
            item_type: item_type.into(),
            severity: Severity::Error,
            property: rule.any_of.join(" | "),
            message: format!("Needs at least one of: {}.", rule.any_of.join(", ")),
        });
    }
    for p in rule.recommended {
        if !has(p) {
            issues.push(SdIssue {
                item_type: item_type.into(),
                severity: Severity::Warning,
                property: p.to_string(),
                message: format!("Missing recommended property '{p}'."),
            });
        }
    }
}

/// itemprop names belonging to this item (not to nested items).
fn microdata_properties(scope: ElementRef) -> Vec<String> {
    let mut out = Vec::new();
    for el in scope.descendants().skip(1).filter_map(ElementRef::wrap) {
        let Some(props) = el.value().attr("itemprop") else { continue };
        // The nearest itemscope ancestor must be `scope` itself.
        let owner = el
            .ancestors()
            .filter_map(ElementRef::wrap)
            .find(|a| a.value().attr("itemscope").is_some());
        if owner.is_some_and(|o| o.id() == scope.id()) {
            out.extend(props.split_whitespace().map(str::to_string));
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_ld_and_microdata() {
        let html = r#"<html><head>
<script type="application/ld+json">{"@context":"https://schema.org","@graph":[
  {"@type":"Product","name":"Toad","offers":{"@type":"Offer","price":"9.99"}},
  {"@type":"BreadcrumbList","itemListElement":[]}
]}</script>
<script type="application/ld+json">{ broken json</script>
</head><body>
<div itemscope itemtype="https://schema.org/Event"><span itemprop="name">Toad Day</span>
  <div itemprop="location" itemscope itemtype="https://schema.org/Place"><span itemprop="name">Pond</span></div>
</div></body></html>"#;
        let sd = extract(&Html::parse_document(html));
        let types: Vec<String> = sd.items.iter().flat_map(|i| i.types.clone()).collect();
        assert_eq!(types, vec!["Product", "Offer", "BreadcrumbList", "Event"]);
        assert_eq!(sd.parse_errors.len(), 1);
        let errors: Vec<(String, String)> = sd
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| (i.item_type.clone(), i.property.clone()))
            .collect();
        assert!(errors.contains(&("Offer".into(), "priceCurrency".into())));
        assert!(errors.contains(&("BreadcrumbList".into(), "itemListElement".into())), "empty array counts as missing");
        assert!(errors.contains(&("Event".into(), "startDate".into())));
        assert!(!errors.iter().any(|(t, p)| t == "Event" && p == "location"), "location is a nested item property");
        assert!(!errors.iter().any(|(t, _)| t == "Product"), "Product has name and offers");
    }
}
