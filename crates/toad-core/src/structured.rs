//! Structured data: JSON-LD, Microdata and RDFa extraction, with two kinds
//! of validation, as Screaming Frog splits them:
//! - **Schema.org validation**: types and properties must exist in the
//!   schema.org vocabulary (bundled in `data/schemaorg.json`), and each
//!   property must be allowed on the item's type or one of its parents.
//! - **Google rich result validation**: required properties are errors and
//!   recommended ones warnings, per Google Search Central's docs.

use crate::config::StructuredDataConfig;
use scraper::{ElementRef, Html};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdItem {
    /// "JSON-LD", "Microdata" or "RDFa".
    pub format: String,
    pub types: Vec<String>,
    /// Property names of the item (not of nested items).
    pub properties: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,
    Warning,
}

/// Which validation found an issue.
pub const SCHEMA_ORG: &str = "Schema.org";
pub const GOOGLE: &str = "Google";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SdIssue {
    pub item_type: String,
    pub severity: Severity,
    pub property: String,
    pub message: String,
    /// "Schema.org" or "Google" (rich result features).
    pub source: String,
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

/// The rich result types Google validation knows about (for the report).
pub fn rich_result_types() -> Vec<&'static str> {
    RULES.iter().flat_map(|r| r.types.iter().copied()).collect()
}

// ------------------------------------------------------------------ vocabulary

struct Vocabulary {
    /// Type -> direct superclasses.
    types: HashMap<String, Vec<String>>,
    /// Property -> types it is defined on (domainIncludes).
    properties: HashMap<String, Vec<String>>,
    /// Lowercase name -> canonical name, for case-insensitive matching.
    type_case: HashMap<String, String>,
    property_case: HashMap<String, String>,
}

fn vocabulary() -> &'static Vocabulary {
    static V: OnceLock<Vocabulary> = OnceLock::new();
    V.get_or_init(|| {
        #[derive(Deserialize)]
        struct Raw {
            types: HashMap<String, Vec<String>>,
            properties: HashMap<String, Vec<String>>,
        }
        let raw: Raw = serde_json::from_str(include_str!("../data/schemaorg.json")).expect("bundled schema.org vocabulary");
        let type_case = raw.types.keys().map(|k| (k.to_lowercase(), k.clone())).collect();
        let property_case = raw.properties.keys().map(|k| (k.to_lowercase(), k.clone())).collect();
        Vocabulary { types: raw.types, properties: raw.properties, type_case, property_case }
    })
}

impl Vocabulary {
    /// The type and every ancestor.
    fn lineage(&self, t: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut stack = vec![t.to_string()];
        while let Some(x) = stack.pop() {
            if out.insert(x.clone()) {
                if let Some(parents) = self.types.get(&x) {
                    stack.extend(parents.iter().cloned());
                }
            }
        }
        out
    }
}

/// Look a name up exactly, or ignoring case (returning the canonical form).
fn resolve<'a>(name: &str, exact: &'a HashMap<String, Vec<String>>, case: &'a HashMap<String, String>, case_sensitive: bool) -> Option<&'a str> {
    if let Some((k, _)) = exact.get_key_value(name) {
        return Some(k.as_str());
    }
    if case_sensitive {
        return None;
    }
    case.get(&name.to_lowercase()).map(|s| s.as_str())
}

fn schema_org_validate(item: &SdItem, case_sensitive: bool, issues: &mut Vec<SdIssue>) {
    let v = vocabulary();
    let mut lineage = HashSet::new();
    for t in &item.types {
        match resolve(t, &v.types, &v.type_case, case_sensitive) {
            Some(canonical) => {
                if canonical != t {
                    issues.push(issue(t, Severity::Warning, "@type", &format!("'{t}' should be written '{canonical}'."), SCHEMA_ORG));
                }
                lineage.extend(v.lineage(canonical));
            }
            None => issues.push(issue(t, Severity::Error, "@type", &format!("'{t}' is not a schema.org type."), SCHEMA_ORG)),
        }
    }
    let item_type = item.types.first().cloned().unwrap_or_default();
    for p in &item.properties {
        if is_action_annotation(p, case_sensitive) {
            continue;
        }
        match resolve(p, &v.properties, &v.property_case, case_sensitive) {
            Some(canonical) => {
                if canonical != p {
                    issues.push(issue(&item_type, Severity::Warning, p, &format!("'{p}' should be written '{canonical}'."), SCHEMA_ORG));
                }
                let domains = &v.properties[canonical];
                if !lineage.is_empty() && !domains.is_empty() && !domains.iter().any(|d| lineage.contains(d)) {
                    issues.push(issue(
                        &item_type,
                        Severity::Error,
                        p,
                        &format!("The property '{canonical}' is not recognised by schema.org for an object of type '{item_type}'."),
                        SCHEMA_ORG,
                    ));
                }
            }
            None => issues.push(issue(&item_type, Severity::Error, p, &format!("'{p}' is not a schema.org property."), SCHEMA_ORG)),
        }
    }
}

/// Action input/output annotations (`query-input`, `name-output`): an
/// existing property name plus `-input` or `-output`. Schema.org's Actions
/// spec and Google's sitelinks search box use them on potentialAction, but
/// they aren't vocabulary properties in their own right.
fn is_action_annotation(p: &str, case_sensitive: bool) -> bool {
    let v = vocabulary();
    p.strip_suffix("-input")
        .or_else(|| p.strip_suffix("-output"))
        .is_some_and(|base| resolve(base, &v.properties, &v.property_case, case_sensitive).is_some())
}

fn issue(item_type: &str, severity: Severity, property: &str, message: &str, source: &str) -> SdIssue {
    SdIssue { item_type: item_type.into(), severity, property: property.into(), message: message.into(), source: source.into() }
}

// ------------------------------------------------------------------ extraction

pub fn extract(doc: &Html, cfg: &StructuredDataConfig) -> StructuredData {
    let mut out = StructuredData::default();
    let mut json_nodes: Vec<Value> = Vec::new();
    for el in doc.root_element().descendants().filter_map(ElementRef::wrap) {
        let v = el.value();
        if cfg.json_ld && v.name() == "script" && v.attr("type").is_some_and(|t| t.trim().eq_ignore_ascii_case("application/ld+json")) {
            let text: String = el.text().collect();
            match serde_json::from_str::<Value>(text.trim()) {
                Ok(json) => {
                    let has_context = json_has_schema_context(&json);
                    let mut nodes = Vec::new();
                    flatten(&json, &mut nodes);
                    if !nodes.is_empty() && !has_context && cfg.schema_org_validation {
                        out.issues.push(issue("JSON-LD", Severity::Warning, "@context", "No schema.org @context.", SCHEMA_ORG));
                    }
                    for node in nodes {
                        out.items.push(SdItem {
                            format: "JSON-LD".into(),
                            types: types_of(node),
                            properties: node
                                .as_object()
                                .map(|o| o.keys().filter(|k| !k.starts_with('@')).cloned().collect())
                                .unwrap_or_default(),
                        });
                        json_nodes.push(node.clone());
                    }
                }
                Err(e) => out.parse_errors.push(format!("JSON-LD: {e}")),
            }
        }
        // Top-level Microdata items: itemscope not itself a property of another item.
        if cfg.microdata && v.attr("itemscope").is_some() && v.attr("itemprop").is_none() {
            let types: Vec<String> = v.attr("itemtype").unwrap_or("").split_whitespace().map(short_type).filter(|t| !t.is_empty()).collect();
            let properties = scoped_properties(el, "itemscope", "itemprop");
            out.items.push(SdItem { format: "Microdata".into(), types, properties });
        }
        // Top-level RDFa items: typeof, not itself a property of another item.
        if cfg.rdfa && v.attr("typeof").is_some() && v.attr("property").is_none() {
            let types: Vec<String> = v.attr("typeof").unwrap_or("").split_whitespace().map(short_type).filter(|t| !t.is_empty()).collect();
            let properties = scoped_properties(el, "typeof", "property").into_iter().map(|p| short_type(&p)).collect();
            out.items.push(SdItem { format: "RDFa".into(), types, properties });
        }
    }

    if cfg.schema_org_validation {
        for item in &out.items {
            schema_org_validate(item, cfg.case_sensitive, &mut out.issues);
        }
    }
    if cfg.google_rich_results {
        let mut json_iter = json_nodes.iter();
        for item in &out.items {
            if item.format == "JSON-LD" {
                if let Some(node) = json_iter.next() {
                    validate_json(node, &mut out.issues);
                }
            } else {
                for t in &item.types {
                    validate(t, |p| item.properties.iter().any(|x| x == p), &mut out.issues);
                }
            }
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

/// `https://schema.org/Product`, `schema:Product` → `Product`.
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
            issues.push(issue(item_type, Severity::Error, p, &format!("Missing required property '{p}'."), GOOGLE));
        }
    }
    if !rule.any_of.is_empty() && !rule.any_of.iter().any(|p| has(p)) {
        issues.push(issue(item_type, Severity::Error, &rule.any_of.join(" | "), &format!("Needs at least one of: {}.", rule.any_of.join(", ")), GOOGLE));
    }
    for p in rule.recommended {
        if !has(p) {
            issues.push(issue(item_type, Severity::Warning, p, &format!("Missing recommended property '{p}'."), GOOGLE));
        }
    }
}

/// Property names belonging to this item (not to nested items): elements
/// with `prop_attr` whose nearest `scope_attr` ancestor is `scope` itself.
fn scoped_properties(scope: ElementRef, scope_attr: &str, prop_attr: &str) -> Vec<String> {
    let mut out = Vec::new();
    for el in scope.descendants().skip(1).filter_map(ElementRef::wrap) {
        let Some(props) = el.value().attr(prop_attr) else { continue };
        let owner = el.ancestors().filter_map(ElementRef::wrap).find(|a| a.value().attr(scope_attr).is_some());
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

    fn all_on() -> StructuredDataConfig {
        StructuredDataConfig { rdfa: true, ..StructuredDataConfig::default() }
    }

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
        let sd = extract(&Html::parse_document(html), &all_on());
        let types: Vec<String> = sd.items.iter().flat_map(|i| i.types.clone()).collect();
        assert_eq!(types, vec!["Product", "Offer", "BreadcrumbList", "Event"]);
        assert_eq!(sd.parse_errors.len(), 1);
        let google: Vec<(String, String)> = sd
            .issues
            .iter()
            .filter(|i| i.severity == Severity::Error && i.source == GOOGLE)
            .map(|i| (i.item_type.clone(), i.property.clone()))
            .collect();
        assert!(google.contains(&("Offer".into(), "priceCurrency".into())));
        assert!(google.contains(&("BreadcrumbList".into(), "itemListElement".into())), "empty array counts as missing");
        assert!(google.contains(&("Event".into(), "startDate".into())));
        assert!(!google.iter().any(|(t, p)| t == "Event" && p == "location"), "location is a nested item property");
        assert!(!google.iter().any(|(t, _)| t == "Product"), "Product has name and offers");
        assert!(!sd.issues.iter().any(|i| i.source == SCHEMA_ORG && i.severity == Severity::Error), "all valid schema.org: {:?}", sd.issues);
    }

    #[test]
    fn schema_org_vocabulary() {
        let html = r#"<script type="application/ld+json">{"@context":"https://schema.org","@type":"Person","name":"Ann","price":"1","favouriteToad":"Bob"}</script>
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"Prodcut","name":"X"}</script>
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"product","Name":"X"}</script>"#;
        let sd = extract(&Html::parse_document(html), &all_on());
        let msgs: Vec<String> = sd.issues.iter().filter(|i| i.source == SCHEMA_ORG).map(|i| format!("{:?}: {}", i.severity, i.message)).collect();
        assert!(msgs.iter().any(|m| m.starts_with("Error") && m.contains("'price' is not recognised") && m.contains("Person")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.starts_with("Error") && m.contains("'favouriteToad' is not a schema.org property")));
        assert!(msgs.iter().any(|m| m.starts_with("Error") && m.contains("'Prodcut' is not a schema.org type")));
        assert!(msgs.iter().any(|m| m.starts_with("Warning") && m.contains("'product' should be written 'Product'")));
        assert!(msgs.iter().any(|m| m.starts_with("Warning") && m.contains("'Name' should be written 'name'")));
        // Inherited: name is defined on Thing, so it's fine on Person.
        assert!(!msgs.iter().any(|m| m.contains("'name' is not")));
        // Case-sensitive: wrong case is an error.
        let strict = extract(&Html::parse_document(html), &StructuredDataConfig { case_sensitive: true, ..all_on() });
        assert!(strict.issues.iter().any(|i| i.severity == Severity::Error && i.message.contains("'product' is not a schema.org type")));
    }

    #[test]
    fn search_action_query_input_is_valid() {
        // Yoast / Rank Math sitelinks search box.
        let html = r#"<script type="application/ld+json">{"@context":"https://schema.org","@graph":[{"@type":"WebSite","url":"https://x.com/","name":"X",
            "potentialAction":[{"@type":"SearchAction","target":{"@type":"EntryPoint","urlTemplate":"https://x.com/?s={search_term_string}"},
            "query-input":{"@type":"PropertyValueSpecification","valueRequired":true,"valueName":"search_term_string"}}]}]}</script>
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"SearchAction","query-input":"required name=q","name-output":"x","bogus-input":"y"}</script>"#;
        let sd = extract(&Html::parse_document(html), &all_on());
        let errors: Vec<&str> = sd.issues.iter().filter(|i| i.severity == Severity::Error).map(|i| i.message.as_str()).collect();
        assert!(!errors.iter().any(|m| m.contains("query-input") || m.contains("name-output")), "{errors:?}");
        assert!(errors.iter().any(|m| m.contains("'bogus-input' is not a schema.org property")), "only real properties take -input: {errors:?}");
    }

    #[test]
    fn rdfa_and_format_toggles() {
        let html = r#"<div vocab="https://schema.org/" typeof="Product"><span property="name">Toad</span>
            <div property="offers" typeof="Offer"><span property="price">5</span></div></div>
            <script type="application/ld+json">{"@context":"https://schema.org","@type":"Thing","name":"T"}</script>"#;
        let doc = Html::parse_document(html);
        let sd = extract(&doc, &all_on());
        let rdfa: Vec<&SdItem> = sd.items.iter().filter(|i| i.format == "RDFa").collect();
        assert_eq!(rdfa.len(), 1);
        assert_eq!(rdfa[0].types, vec!["Product"]);
        assert_eq!(rdfa[0].properties, vec!["name", "offers"]);
        let off = extract(&doc, &StructuredDataConfig { rdfa: false, json_ld: false, ..all_on() });
        assert!(off.items.is_empty());
        let no_validation = extract(&doc, &StructuredDataConfig { schema_org_validation: false, google_rich_results: false, ..all_on() });
        assert!(no_validation.issues.is_empty() && !no_validation.items.is_empty());
    }
}
