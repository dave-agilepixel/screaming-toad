//! Every axe-core rule (v4.10), with the filter label Screaming Frog gives
//! it in the Accessibility tab. Rules Screaming Frog doesn't list (most are
//! experimental or deprecated, so off by default) get a label in the same
//! style. `impact` is axe's usual impact for the rule; `advisory` marks best
//! practice, AAA, experimental and deprecated rules, which are warnings
//! rather than WCAG A/AA failures.

pub struct AxeRule {
    pub id: &'static str,
    pub label: &'static str,
    pub impact: &'static str,
    pub advisory: bool,
}

const fn r(id: &'static str, label: &'static str, impact: &'static str, advisory: bool) -> AxeRule {
    AxeRule { id, label, impact, advisory }
}

pub const AXE_RULES: &[AxeRule] = &[
    r("accesskeys", "Accesskey Attribute Value Must Be Unique", "serious", true),
    r("area-alt", "Active <area> Elements Require Alternate Text", "critical", false),
    r("aria-allowed-attr", "Elements Must Use Allowed ARIA Attributes", "critical", false),
    r("aria-allowed-role", "ARIA Role Should Be Appropriate For Element", "minor", true),
    r("aria-braille-equivalent", "Aria-braille Require Non-braille Equivalent", "serious", false),
    r("aria-command-name", "ARIA Commands Require Accessible Name", "serious", false),
    r("aria-conditional-attr", "ARIA Attribute Must Be Used As Specified For Role", "serious", false),
    r("aria-deprecated-role", "Deprecated ARIA Roles Must Not Be Used", "minor", false),
    r("aria-dialog-name", "ARIA Dialog & Alertdialog Require Accessible Name", "serious", true),
    r("aria-hidden-body", "Aria-hidden=true Must Not Be Used In <body>", "critical", false),
    r("aria-hidden-focus", "Aria-hidden Elements Contains Focusable Elements", "serious", false),
    r("aria-input-field-name", "ARIA Input Fields Require Accessible Name", "serious", false),
    r("aria-meter-name", "ARIA Meter Nodes Require Accessible Name", "serious", false),
    r("aria-progressbar-name", "ARIA Progressbar Nodes Require Accessible Name", "serious", false),
    r("aria-prohibited-attr", "Elements Must Only Use Permitted ARIA Attributes", "serious", false),
    r("aria-required-attr", "Required ARIA Attributes Must Be Provided", "critical", false),
    r("aria-required-children", "Certain ARIA Roles Must Contain Specific Children", "critical", false),
    r("aria-required-parent", "ARIA Roles Must Be Contained By Required Parent", "critical", false),
    r("aria-roledescription", "Aria-roledescription Requires A Semantic Role", "serious", true),
    r("aria-roles", "ARIA Roles Require Valid Values", "critical", false),
    r("aria-text", "Role=text Should Have No Focusable Descendants", "serious", true),
    r("aria-toggle-field-name", "ARIA Toggle Fields Require Accessible Name", "serious", false),
    r("aria-tooltip-name", "ARIA Tooltip Nodes Require Accessible Name", "serious", false),
    r("aria-treeitem-name", "ARIA Treeitem Nodes Require Accessible Name", "serious", true),
    r("aria-valid-attr", "ARIA Attributes Require Valid Names", "critical", false),
    r("aria-valid-attr-value", "ARIA Attributes Require Valid Values", "critical", false),
    r("audio-caption", "<audio> Elements Require A Captions Track", "critical", true),
    r("autocomplete-valid", "Autocomplete Attribute Must Be Used Correctly", "serious", false),
    r("avoid-inline-spacing", "Inline Text Spacing Must Be Adjustable", "serious", false),
    r("blink", "<blink> Elements Deprecated & Must Not Be Used", "serious", false),
    r("button-name", "Buttons Require Discernible Text", "critical", false),
    r("bypass", "Page Requires Means To Bypass Repeated Blocks", "serious", false),
    r("color-contrast", "Text Requires Higher Color Contrast to Background", "serious", false),
    r("color-contrast-enhanced", "Text Requires Higher Color Contrast Ratio", "serious", true),
    r("css-orientation-lock", "CSS Must Not Lock Display Orientation", "serious", true),
    r("definition-list", "<dl> Must Only Have Ordered <dt> & <dd> Groups", "serious", false),
    r("dlitem", "<dt> & <dd> Elements Must Be Contained by <dl>", "serious", false),
    r("document-title", "Page Must Contain <title>", "serious", false),
    r("duplicate-id", "IDs Must Be Unique", "minor", true),
    r("duplicate-id-active", "IDs Of Active Elements Must Be Unique", "serious", true),
    r("duplicate-id-aria", "IDs Used In ARIA & Labels Must Be Unique", "critical", false),
    r("empty-heading", "Headings Should Not Be Empty", "minor", true),
    r("empty-table-header", "Table Headers Require Discernible Text", "minor", true),
    r("focus-order-semantics", "Focusable Elements Require An Appropriate Role", "minor", true),
    r("form-field-multiple-labels", "Form Field Must Not Have Multiple Label Elements", "moderate", false),
    r("frame-focusable-content", "Frames With Focusable Content Must Not Use tabindex=-1", "serious", false),
    r("frame-tested", "Frames Should Be Tested With axe-core", "critical", true),
    r("frame-title", "Frames Require Title Attribute", "serious", false),
    r("frame-title-unique", "Frames Require Unique Title Attribute", "serious", true),
    r("heading-order", "Heading Levels Should Only Increase By One", "moderate", true),
    r("hidden-content", "Hidden Content Should Be Tested", "minor", true),
    r("html-has-lang", "HTML Element Requires Lang Attribute", "serious", false),
    r("html-lang-valid", "HTML Element Lang Attribute Value Must Be Valid", "serious", false),
    r("html-xml-lang-mismatch", "HTML Lang & XML Lang Value Should Match", "moderate", false),
    r("identical-links-same-purpose", "Links With Same Accessible Name", "minor", true),
    r("image-alt", "Images Require Alternate Text", "critical", false),
    r("image-redundant-alt", "Alt Text Should Not Be Repeated As Text", "minor", true),
    r("input-button-name", "Input Buttons Require Discernible Text", "critical", false),
    r("input-image-alt", "Image Button Requires Alternate Text", "critical", false),
    r("label", "Form <input> Elements Require Labels", "critical", false),
    r("label-content-name-mismatch", "Visible Label Must Be Part Of Accessible Name", "serious", true),
    r("label-title-only", "Form Elements Should Have Visible Label", "serious", true),
    r("landmark-banner-is-top-level", "Banner Landmark Must Not Be In Another Landmark", "moderate", true),
    r("landmark-complementary-is-top-level", "Complementary Landmarks & Asides Must Be Top Level", "moderate", true),
    r("landmark-contentinfo-is-top-level", "Contentinfo Landmark Must Be Top Level Landmark", "moderate", true),
    r("landmark-main-is-top-level", "Main Landmark Must Not Be In Another Landmark", "moderate", true),
    r("landmark-no-duplicate-banner", "Page Must Not Have More Than One Banner Landmark", "moderate", true),
    r("landmark-no-duplicate-contentinfo", "Page Must Not Have Multiple Contentinfo Landmarks", "moderate", true),
    r("landmark-no-duplicate-main", "Page Requires At Most One Main Landmark", "moderate", true),
    r("landmark-one-main", "Page Requires One Main Landmark", "moderate", true),
    r("landmark-unique", "Landmarks Require Unique Role Or Accessible Name", "moderate", true),
    r("link-in-text-block", "Links Must Be Distinguishable", "serious", false),
    r("link-name", "Links Require Discernible Text", "serious", false),
    r("list", "Lists Must Only Contain <li> Content Elements", "serious", false),
    r("listitem", "List Items Must Be Contained In List Elements", "serious", false),
    r("marquee", "Deprecated <marquee> Element Must Not Be Used", "serious", false),
    r("meta-refresh", "Timed Meta Refresh Must Not Exist", "critical", false),
    r("meta-refresh-no-exceptions", "Delayed Meta Refresh Must Not Be Used", "minor", true),
    r("meta-viewport", "Meta Viewport Zoom & Scaling Disabled", "critical", false),
    r("meta-viewport-large", "Meta Viewport Should Allow Zoom & Scale Up to 500%", "minor", true),
    r("nested-interactive", "Interactive Controls Must Not Be Nested", "serious", false),
    r("no-autoplay-audio", "<video> or <audio> Elements Must Not Auto-play", "moderate", false),
    r("object-alt", "<object> Elements Require Alternate Text", "serious", false),
    r("p-as-heading", "Bold Paragraphs Must Not Be Used As Headings", "serious", true),
    r("page-has-heading-one", "Page Must Contain <h1>", "moderate", true),
    r("presentation-role-conflict", "Ensure Elements Marked Presentational Are Ignored", "minor", true),
    r("region", "All Page Content Must Be Contained By Landmarks", "moderate", true),
    r("role-img-alt", "Elements Marked role=img Require Alternate Text", "serious", false),
    r("scope-attr-valid", "Scope Attribute Should Be Used Correctly On Tables", "moderate", true),
    r("scrollable-region-focusable", "Scrollable Region Requires Keyboard Access", "serious", false),
    r("select-name", "Select Element Requires Accessible Name", "critical", false),
    r("server-side-image-map", "Server-Side Image Maps Must Not Be Used", "minor", false),
    r("skip-link", "Skip-link Target Should Exist & Be Focusable", "moderate", true),
    r("summary-name", "Summary Elements Require Discernible Text", "serious", false),
    r("svg-img-alt", "SVG Images & Graphics Require Accessible Text", "serious", false),
    r("tabindex", "Elements Must Not Have Tabindex Greater Than Zero", "serious", true),
    r("table-duplicate-name", "Table With Identical Summary & Caption Text", "minor", true),
    r("table-fake-caption", "Data Tables Must Use <caption> For Captions", "serious", true),
    r("target-size", "Touch Targets Require Sufficient Size & Spacing", "serious", false),
    r("td-has-header", "Large Table Data Cells Require Headers", "critical", true),
    r("td-headers-attr", "Table Header Attr Must Refer To Cell In Same Table", "serious", false),
    r("th-has-data-cells", "<th> Element Requires Associated Data Cells", "serious", false),
    r("valid-lang", "Lang Attribute Requires Valid Value", "serious", false),
    r("video-caption", "<video> Elements Require <track> For Captions", "critical", false),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_labels_are_unique() {
        let mut ids: Vec<&str> = AXE_RULES.iter().map(|r| r.id).collect();
        let mut labels: Vec<String> = AXE_RULES.iter().map(|r| r.label.to_lowercase()).collect();
        let n = ids.len();
        ids.sort();
        ids.dedup();
        labels.sort();
        labels.dedup();
        assert_eq!((ids.len(), labels.len()), (n, n));
        assert!(AXE_RULES.iter().all(|r| ["critical", "serious", "moderate", "minor"].contains(&r.impact)));
    }

    /// Every rule in the bundled axe-core has a filter.
    #[test]
    fn covers_the_bundled_axe_core() {
        let js = include_str!("../../toad-core/data/axe.min.js");
        let re = regex::Regex::new(r#"\{id:"([a-z0-9-]+)",(?:impact|selector|excludeHidden|tags|matches|enabled|reviewOnFail|actIds)"#).unwrap();
        let mut bundled: Vec<&str> = re.captures_iter(js).filter_map(|c| c.get(1)).map(|m| m.as_str()).collect();
        bundled.sort();
        bundled.dedup();
        let missing: Vec<&&str> = bundled.iter().filter(|id| !AXE_RULES.iter().any(|r| r.id == **id)).collect();
        assert!(missing.is_empty(), "rules without a filter: {missing:?}");
        assert_eq!(bundled.len(), AXE_RULES.len());
    }
}
