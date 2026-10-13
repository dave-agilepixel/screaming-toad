// The Configuration dialog is driven by this schema: one tree node per
// Screaming Frog menu entry (in the reference's menu order), each with the
// controls Toad implements. The tree, the search box and the
// "changed from default" view all read from here, so a new setting is one
// entry below plus its field in `CrawlConfig` (crates/toad-core/src/config.rs).

export type RobotsMode = "respect" | "ignore" | "ignore_but_report";

export interface CustomSearch {
  name: string;
  mode: "contains" | "does_not_contain";
  kind: "text" | "regex";
  query: string;
  case_sensitive: boolean;
  search_in: "html" | "html_head" | "html_body" | "page_text" | "page_text_no_anchors" | "content_area" | "xpath";
  xpath: string;
}

export interface CustomExtractor {
  name: string;
  kind: "xpath" | "css" | "regex";
  expression: string;
  attribute: string | null;
  extract: "element" | "inner_html" | "text" | "function_value";
}

export interface CustomJavaScript {
  name: string;
  kind: "extraction" | "action";
  code: string;
  timeout_secs: number;
  content_types: string;
}

export interface ContentArea {
  mode: "exclude" | "include" | "both";
  check_alt_text: boolean;
  include_elements: string[];
  exclude_elements: string[];
  include_classes: string[];
  exclude_classes: string[];
  include_ids: string[];
  exclude_ids: string[];
}

export interface StructuredDataConfig {
  json_ld: boolean;
  microdata: boolean;
  rdfa: boolean;
  schema_org_validation: boolean;
  google_rich_results: boolean;
  case_sensitive: boolean;
}

export interface RenderingConfig {
  mode: "text_only" | "javascript";
  screenshots: boolean;
  js_error_reporting: boolean;
  flatten_shadow_dom: boolean;
  flatten_iframes: boolean;
  ajax_timeout_secs: number;
  window_preset: string;
  width: number;
  height: number;
  scaling_factor: number;
  mobile: boolean;
  touch: boolean;
  resize_to_content: boolean;
  resize_time_secs: number;
  rotate: boolean;
  store_rendered_html: boolean;
  accessibility: boolean;
  browser_path: string | null;
}

export interface SpellingConfig {
  spell_check: boolean;
  grammar_check: boolean;
  language_mode: "auto" | "manual";
  language: string;
  grammar_server: string;
  disabled_grammar_rules: string[];
  ignore_words: string[];
}

export interface Extraction {
  page_title: boolean;
  meta_description: boolean;
  meta_keywords: boolean;
  h1: boolean;
  h2: boolean;
  word_count: boolean;
  readability: boolean;
  text_to_code_ratio: boolean;
  hash: boolean;
  page_size: boolean;
  forms: boolean;
  response_time: boolean;
  last_modified: boolean;
  http_headers: boolean;
  cookies: boolean;
  meta_robots: boolean;
  x_robots_tag: boolean;
}

export interface Preferences {
  title_min_chars: number;
  title_max_chars: number;
  title_min_px: number;
  title_max_px: number;
  description_min_chars: number;
  description_max_chars: number;
  description_min_px: number;
  description_max_px: number;
  url_max_chars: number;
  h1_max_chars: number;
  h2_max_chars: number;
  alt_text_max_chars: number;
  image_max_kb: number;
  low_content_words: number;
  high_crawl_depth: number;
  high_internal_outlinks: number;
  high_external_outlinks: number;
  html_max_mb: number;
  resource_max_mb: number;
  non_descriptive_anchor_text: string[];
  soft_404_phrases: string[];
}

/** Mirrors toad_core::config::CrawlConfig field for field. */
export interface CrawlConfig {
  mode: "spider" | "list";
  store_images: boolean;
  crawl_images: boolean;
  store_css: boolean;
  crawl_css: boolean;
  store_js: boolean;
  crawl_js: boolean;
  store_internal_links: boolean;
  crawl_internal_links: boolean;
  store_media: boolean;
  crawl_media: boolean;
  store_swf: boolean;
  crawl_swf: boolean;
  store_amp: boolean;
  crawl_amp: boolean;
  store_meta_refresh: boolean;
  store_iframes: boolean;
  store_mobile_alternate: boolean;
  crawl_mobile_alternate: boolean;
  store_uncrawlable_links: boolean;
  crawl_invalid_links: boolean;
  store_external_links: boolean;
  crawl_external_links: boolean;
  store_canonicals: boolean;
  crawl_canonicals: boolean;
  store_pagination: boolean;
  crawl_pagination: boolean;
  store_hreflang: boolean;
  crawl_hreflang: boolean;
  crawl_meta_refresh: boolean;
  crawl_iframes: boolean;
  check_links_outside_start_folder: boolean;
  crawl_outside_start_folder: boolean;
  crawl_all_subdomains: boolean;
  follow_internal_nofollow: boolean;
  follow_external_nofollow: boolean;
  crawl_fragment_identifiers: boolean;
  crawl_linked_sitemaps: boolean;
  auto_discover_sitemaps_via_robots: boolean;
  sitemap_urls: string[];
  max_urls: number;
  max_depth: number | null;
  max_folder_depth: number | null;
  max_query_params: number | null;
  max_redirects: number;
  max_url_length: number;
  max_links_per_url: number;
  max_page_size_kb: number;
  max_urls_per_subdomain: number | null;
  max_urls_per_depth: number | null;
  path_limits: { pattern: string; max: number }[];
  cookie_storage: "session_only" | "persistent" | "do_not_store";
  ignore_non_indexable_for_issues: boolean;
  ignore_paginated_for_duplicates: boolean;
  always_follow_redirects: boolean;
  always_follow_canonicals: boolean;
  respect_noindex: boolean;
  respect_canonical: boolean;
  respect_next_prev: boolean;
  respect_hsts: boolean;
  respect_self_referencing_meta_refresh: boolean;
  extract_images_from_srcset: boolean;
  perform_html_validation: boolean;
  response_timeout_secs: number;
  retries_5xx: number;
  assume_pages_are_html: boolean;
  threads: number;
  max_urls_per_sec: number | null;
  robots_mode: RobotsMode;
  show_internal_blocked_by_robots: boolean;
  show_external_blocked_by_robots: boolean;
  custom_robots: { host: string; enabled: boolean; txt: string }[];
  user_agent: string;
  robots_user_agent: string;
  custom_headers: [string, string][];
  include: string[];
  exclude: string[];
  remove_all_parameters: boolean;
  remove_parameters: string[];
  url_rewrite_rules: { regex: string; replace: string }[];
  lowercase_urls: boolean;
  percent_encoding_case: "uppercase" | "lowercase" | "none";
  cdns: string[];
  content_area: ContentArea;
  structured_data: StructuredDataConfig;
  store_html: boolean;
  store_pdf: boolean;
  rendering: RenderingConfig;
  extract_pdf_properties: boolean;
  extract_pdf_links: boolean;
  extraction: Extraction;
  near_duplicates: boolean;
  near_duplicate_threshold: number;
  near_duplicates_indexable_only: boolean;
  spelling: SpellingConfig;
  custom_search: CustomSearch[];
  custom_extraction: CustomExtractor[];
  custom_javascript: CustomJavaScript[];
  link_positions_enabled: boolean;
  link_positions: { name: string; contains: string }[];
  preferences: Preferences;
}

type Pred = (cfg: CrawlConfig) => boolean;

export type Field =
  | { kind: "bool"; key: string; label: string; help?: string; disabledIf?: Pred; pending?: string }
  | { kind: "number"; key: string; label: string; help?: string; min?: number; max?: number; step?: number; unit?: string; disabledIf?: Pred }
  /** A "Limit X" checkbox with a value: null means off; ticking it uses `whenOn`. */
  | { kind: "limit"; key: string; label: string; help?: string; whenOn: number; min?: number; step?: number; unit?: string }
  /** A fraction stored 0–1, shown as a percentage. */
  | { kind: "percent"; key: string; label: string; help?: string; min?: number; disabledIf?: Pred }
  | { kind: "text"; key: string; label: string; help?: string; placeholder?: string }
  | { kind: "select"; key: string; label: string; help?: string; options: [string, string][] }
  | { kind: "lines"; key: string; label: string; help?: string; placeholder?: string; rows?: number; disabledIf?: Pred }
  /** A Store / Crawl pair, as in Spider > Crawl. */
  /** Store / Crawl, as in Spider > Crawl. Crawl implies Store; storeOnly rows have no Crawl box. */
  | { kind: "pair"; label: string; store?: string; crawl?: string; help?: string; storeOnly?: boolean }
  | { kind: "headers"; key: string; label: string; help?: string }
  | { kind: "path-limits"; key: string; label: string; help?: string }
  | { kind: "custom-robots"; key: string; label: string; help?: string }
  | { kind: "ua-preset"; label: string }
  | { kind: "window-preset"; label: string; disabledIf?: Pred }
  | { kind: "link-positions"; key: string; label: string }
  | { kind: "spelling-language"; key: string; label: string; help?: string }
  | { kind: "global-dictionary"; label: string; help?: string }
  | { kind: "rewrite-rules"; key: string; label: string; help?: string }
  /** A live test box: how one URL is rewritten / classified / matched. */
  | { kind: "url-test"; label: string; show: "rewrite" | "cdn" | "include" | "exclude" }
  | { kind: "custom-search" }
  | { kind: "custom-extraction" }
  | { kind: "custom-javascript" }
  | { kind: "authentication"; label: string }
  | { kind: "note"; text: string };

export interface Group {
  title?: string;
  fields: Field[];
}

export interface Node {
  id: string;
  label: string;
  /** Parent node id, for the nested entries (Spider ▸ Crawl, Content ▸ Area…). */
  parent?: string;
  groups: Group[];
  /** A section that is not built yet; shown so the menu matches, with a note. */
  planned?: string;
}

export const NODES: Node[] = [
  { id: "spider", label: "Spider", groups: [] },
  {
    id: "crawl",
    label: "Crawl",
    parent: "spider",
    groups: [
      {
        title: "Resource Links",
        fields: [
          { kind: "pair", label: "Images", store: "store_images", crawl: "crawl_images" },
          { kind: "pair", label: "Media", store: "store_media", crawl: "crawl_media", help: "<video>, <audio> and <source> files." },
          { kind: "pair", label: "CSS", store: "store_css", crawl: "crawl_css" },
          { kind: "pair", label: "JavaScript", store: "store_js", crawl: "crawl_js" },
          { kind: "pair", label: "SWF", store: "store_swf", crawl: "crawl_swf", help: "<embed> and <object> files." },
        ],
      },
      {
        title: "Page Links",
        fields: [
          { kind: "pair", label: "Internal Hyperlinks", store: "store_internal_links", crawl: "crawl_internal_links" },
          { kind: "pair", label: "External Links", store: "store_external_links", crawl: "crawl_external_links", help: "Also governs external images, CSS, JavaScript, hreflang and canonicals." },
          { kind: "pair", label: "Canonicals", store: "store_canonicals", crawl: "crawl_canonicals" },
          { kind: "pair", label: "Pagination (Rel Next/Prev)", store: "store_pagination", crawl: "crawl_pagination" },
          { kind: "pair", label: "Hreflang", store: "store_hreflang", crawl: "crawl_hreflang" },
          { kind: "pair", label: "AMP", store: "store_amp", crawl: "crawl_amp", help: '<link rel="amphtml">.' },
          { kind: "pair", label: "Meta Refresh", store: "store_meta_refresh", crawl: "crawl_meta_refresh" },
          { kind: "pair", label: "iframes", store: "store_iframes", crawl: "crawl_iframes" },
          { kind: "pair", label: "Mobile Alternate", store: "store_mobile_alternate", crawl: "crawl_mobile_alternate", help: '<link rel="alternate" media="…"> separate mobile URLs.' },
          { kind: "pair", label: "Uncrawlable Links", store: "store_uncrawlable_links", storeOnly: true, help: "span/div hrefs, onclick navigation and javascript: hrefs: Links > Pages With Uncrawlable Internal Outlinks." },
        ],
      },
      {
        title: "Crawl Behaviour",
        fields: [
          { kind: "bool", key: "check_links_outside_start_folder", label: "Check Links Outside of Start Folder" },
          { kind: "bool", key: "crawl_outside_start_folder", label: "Crawl Outside of Start Folder" },
          { kind: "bool", key: "crawl_all_subdomains", label: "Crawl All Subdomains" },
          { kind: "bool", key: "follow_internal_nofollow", label: 'Follow Internal "nofollow"' },
          { kind: "bool", key: "follow_external_nofollow", label: 'Follow External "nofollow"' },
          { kind: "bool", key: "crawl_invalid_links", label: "Crawl Invalid Links", help: "Report malformed hrefs such as hppts:// under Response Codes > No Response." },
        ],
      },
      {
        title: "XML Sitemaps",
        fields: [
          { kind: "bool", key: "crawl_linked_sitemaps", label: "Crawl Linked XML Sitemaps" },
          {
            kind: "bool",
            key: "auto_discover_sitemaps_via_robots",
            label: "Auto Discover XML Sitemaps via robots.txt",
            disabledIf: (c) => !c.crawl_linked_sitemaps,
          },
          {
            kind: "lines",
            key: "sitemap_urls",
            label: "Crawl These Sitemaps",
            placeholder: "https://www.example.com/sitemap.xml",
            help: "One sitemap URL per line. Sitemap URLs feed the Sitemaps tab: orphans, URLs missing from sitemaps and non-indexable URLs in them.",
            disabledIf: (c) => !c.crawl_linked_sitemaps,
          },
        ],
      },
    ],
  },
  {
    id: "extraction",
    label: "Extraction",
    parent: "spider",
    groups: [
      {
        title: "Page Details",
        fields: [
          { kind: "bool", key: "extraction.page_title", label: "Page Title" },
          { kind: "bool", key: "extraction.meta_description", label: "Meta Description" },
          { kind: "bool", key: "extraction.meta_keywords", label: "Meta Keywords" },
          { kind: "bool", key: "extraction.h1", label: "H1" },
          { kind: "bool", key: "extraction.h2", label: "H2" },
          { kind: "note", text: "Indexability is always extracted: every tab's filters depend on it." },
          { kind: "bool", key: "extraction.word_count", label: "Word Count" },
          { kind: "bool", key: "extraction.readability", label: "Readability" },
          { kind: "bool", key: "extraction.text_to_code_ratio", label: "Text to Code Ratio" },
          { kind: "bool", key: "extraction.hash", label: "Hash Value", help: "MD5 of the page; needed for Content > Exact Duplicates." },
          { kind: "bool", key: "extraction.page_size", label: "Page Size" },
          { kind: "bool", key: "extraction.forms", label: "Forms", help: "Security > Form URL Insecure and Form on HTTP URL." },
        ],
      },
      {
        title: "URL Details",
        fields: [
          { kind: "bool", key: "extraction.response_time", label: "Response Time" },
          { kind: "bool", key: "extraction.last_modified", label: "Last Modified" },
          { kind: "bool", key: "extraction.http_headers", label: "HTTP Headers", help: "HTTP Headers lower tab, Reports > HTTP Header Summary, Bulk Export > All HTTP Response Headers." },
          { kind: "bool", key: "extraction.cookies", label: "Cookies", help: "Cookies lower tab and column, Reports > Cookie Summary, Bulk Export > All Cookies." },
        ],
      },
      {
        title: "Directives",
        fields: [
          { kind: "bool", key: "extraction.meta_robots", label: "Meta Robots", help: "When off, meta robots directives don't affect indexability or nofollow." },
          { kind: "bool", key: "extraction.x_robots_tag", label: "X-Robots-Tag" },
        ],
      },
      {
        title: "Structured Data",
        fields: [
          { kind: "bool", key: "structured_data.json_ld", label: "JSON-LD" },
          { kind: "bool", key: "structured_data.microdata", label: "Microdata" },
          { kind: "bool", key: "structured_data.rdfa", label: "RDFa" },
          {
            kind: "bool",
            key: "structured_data.schema_org_validation",
            label: "Schema.org Validation",
            help: "Types and properties must exist in schema.org and suit the item's type: Structured Data > Validation Errors / Warnings.",
            disabledIf: (c) => !(c.structured_data.json_ld || c.structured_data.microdata || c.structured_data.rdfa),
          },
          {
            kind: "bool",
            key: "structured_data.google_rich_results",
            label: "Google Rich Result Feature Validation",
            help: "Required and recommended properties for Google rich results: Rich Result Validation Errors / Warnings.",
            disabledIf: (c) => !(c.structured_data.json_ld || c.structured_data.microdata || c.structured_data.rdfa),
          },
          {
            kind: "bool",
            key: "structured_data.case_sensitive",
            label: "Case-Sensitive",
            help: "Schema.org is case-sensitive; Google is not. Off: wrong case is a warning rather than an error.",
            disabledIf: (c) => !c.structured_data.schema_org_validation,
          },
          { kind: "note", text: "JSON-LD, Microdata and validation are on by default in Toad (Screaming Frog has them off)." },
        ],
      },
      {
        title: "PDF",
        fields: [
          { kind: "bool", key: "store_pdf", label: "Store PDF", help: "Read PDF text: word count, readability, Custom Search and spelling on PDFs, and Bulk Export > All PDF Content." },
          { kind: "bool", key: "extract_pdf_properties", label: "Extract PDF Properties", help: "Subject, Author, Creation/Modification Date and Page Count columns. The PDF Title and Keywords are always read into Title and Meta Keywords." },
          { kind: "bool", key: "extract_pdf_links", label: "Extract PDF Links", help: "Follow and report links inside PDFs (Link Position shows the PDF page). Link anchor text isn't read." },
        ],
      },
      {
        title: "HTML",
        fields: [
          {
            kind: "bool",
            key: "store_html",
            label: "Store HTML",
            help: "Keep the HTML of internal pages: View Source in the lower window, Bulk Export > All Page Source, and Content / View Source in Compare mode.",
          },
          {
            kind: "bool",
            key: "rendering.store_rendered_html",
            label: "Store Rendered HTML",
            help: "Keep the HTML after JavaScript runs: View Source > Rendered HTML. Needs Rendering: JavaScript.",
            disabledIf: (c) => c.rendering.mode !== "javascript",
          },
        ],
      },
      {
        title: "Accessibility",
        fields: [
          {
            kind: "bool",
            key: "rendering.accessibility",
            label: "Accessibility",
            help: "Audit rendered pages with axe-core against WCAG 2.0–2.2 and best practices: the Accessibility tab, Accessibility Details lower tab and Reports > Accessibility Violations Summary. Needs Rendering: JavaScript.",
            disabledIf: (c) => c.rendering.mode !== "javascript",
          },
        ],
      },
      {
        fields: [
          {
            kind: "note",
            text: "Page details (title, meta description, H1/H2, indexability, word count, readability, hash, size, response time, meta robots, X-Robots-Tag) are always extracted. Indexability is always extracted.",
          },
        ],
      },
    ],
  },
  {
    id: "limits",
    label: "Limits",
    parent: "spider",
    groups: [
      {
        fields: [
          { kind: "number", key: "max_urls", label: "Limit Crawl Total", min: 1, help: "Maximum URLs to crawl." },
          { kind: "limit", key: "max_urls_per_subdomain", label: "Limit Crawl Total Per Subdomain", whenOn: 10000, min: 1 },
          { kind: "limit", key: "max_depth", label: "Limit Crawl Depth", whenOn: 3, min: 0, help: "Clicks from the start URL. A redirect counts as one level." },
          { kind: "limit", key: "max_urls_per_depth", label: "Limit URLs Per Crawl Depth", whenOn: 1000, min: 1 },
          { kind: "limit", key: "max_folder_depth", label: "Limit Max Folder Depth", whenOn: 5, min: 0 },
          { kind: "limit", key: "max_query_params", label: "Limit Number of Query Strings", whenOn: 5, min: 0 },
          { kind: "number", key: "max_redirects", label: "Max Redirects to Follow", min: 0, max: 20 },
          { kind: "number", key: "max_url_length", label: "Max URL Length to Crawl", min: 1, unit: "characters" },
          { kind: "number", key: "max_links_per_url", label: "Max Links per URL to Crawl", min: 1 },
          { kind: "number", key: "max_page_size_kb", label: "Max Page Size", min: 1, unit: "KB", help: "Larger pages are reported as not processed." },
          { kind: "path-limits", key: "path_limits", label: "Limit by URL Path", help: "At most N URLs matching each regex, e.g. /products/ → 1,000 or \\?search= → 100." },
        ],
      },
    ],
  },
  {
    id: "rendering",
    label: "Rendering",
    parent: "spider",
    groups: [
      {
        fields: [
          {
            kind: "select",
            key: "rendering.mode",
            label: "Rendering",
            options: [
              ["text_only", "Text Only"],
              ["javascript", "JavaScript"],
            ],
            help: "JavaScript renders internal HTML pages in headless Chrome or Edge and extracts from the rendered HTML; the original HTML is kept for the JavaScript tab.",
          },
        ],
      },
      {
        title: "Rendered Page",
        fields: [
          { kind: "bool", key: "rendering.screenshots", label: "Enable Rendered Page Screenshots", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.js_error_reporting", label: "Enable JavaScript Error Reporting", help: "Chrome console messages: the Chrome Console Log lower tab and JavaScript > Pages With JavaScript Errors.", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.flatten_shadow_dom", label: "Flatten Shadow DOM", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.flatten_iframes", label: "Flatten iframes", help: "Same-origin iframe content is inlined into the rendered HTML.", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "number", key: "rendering.ajax_timeout_secs", label: "AJAX Timeout", min: 0, step: 0.5, unit: "secs", help: "How long to wait after the page loads before taking the snapshot.", disabledIf: (c) => c.rendering.mode !== "javascript" },
        ],
      },
      {
        title: "Window Size",
        fields: [
          { kind: "window-preset", label: "Window Size", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "number", key: "rendering.width", label: "Width", min: 100, unit: "px", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "number", key: "rendering.height", label: "Height", min: 100, unit: "px", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "number", key: "rendering.scaling_factor", label: "Scaling Factor", min: 0.5, step: 0.25, disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.mobile", label: "Mobile", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.touch", label: "Touch", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.rotate", label: "Rotate", help: "Swap width and height (landscape).", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "bool", key: "rendering.resize_to_content", label: "Resize to Content", help: "Full-page screenshots.", disabledIf: (c) => c.rendering.mode !== "javascript" },
          { kind: "number", key: "rendering.resize_time_secs", label: "Resize Time", min: 0, step: 0.5, unit: "secs", disabledIf: (c) => c.rendering.mode !== "javascript" },
        ],
      },
      {
        title: "Browser",
        fields: [
          {
            kind: "text",
            key: "rendering.browser_path",
            label: "Browser Executable",
            placeholder: "Auto-detect Chrome, Edge or Chromium",
            help: "Toad drives an installed Chrome-family browser rather than bundling one. Leave blank to find it automatically.",
          },
        ],
      },
    ],
  },
  {
    id: "advanced",
    label: "Advanced",
    parent: "spider",
    groups: [
      {
        fields: [
          {
            kind: "select",
            key: "cookie_storage",
            label: "Cookie Storage",
            options: [
              ["session_only", "Session Only"],
              ["persistent", "Persistent"],
              ["do_not_store", "Do Not Store"],
            ],
            help: "Session Only keeps cookies along a redirect chain; Persistent shares one cookie jar across the whole crawl.",
          },
          { kind: "bool", key: "ignore_non_indexable_for_issues", label: "Ignore Non-Indexable URLs for Issues" },
          { kind: "bool", key: "ignore_paginated_for_duplicates", label: "Ignore Paginated URLs for Duplicate Filters", help: "Pages with rel=prev are left out of the Duplicate filters." },
          { kind: "bool", key: "always_follow_redirects", label: "Always Follow Redirects", help: "Follow redirect chains past the crawl depth limit (useful in List mode), up to Max Redirects." },
          { kind: "bool", key: "always_follow_canonicals", label: "Always Follow Canonicals", help: "Follow canonical targets past the depth limit. See Reports > Canonical Chains." },
          { kind: "bool", key: "respect_noindex", label: "Respect noindex", help: "Noindex pages are crawled but hidden from every tab." },
          { kind: "bool", key: "respect_canonical", label: "Respect Canonical", help: "Canonicalised pages are crawled but hidden." },
          { kind: "bool", key: "respect_next_prev", label: "Respect Next/Prev", help: "rel=prev pages are crawled but hidden." },
          { kind: "bool", key: "respect_hsts", label: "Respect HSTS Policy", help: "http:// links to a host that sent Strict-Transport-Security are reported as 307 'HSTS Policy'." },
          { kind: "bool", key: "respect_self_referencing_meta_refresh", label: "Respect Self Referencing Meta Refresh", help: "A page that meta-refreshes to itself is Non-Indexable." },
          { kind: "bool", key: "extract_images_from_srcset", label: "Extract Images From IMG SRCSET Attribute", help: "Also <picture><source srcset>." },
          { kind: "bool", key: "crawl_fragment_identifiers", label: "Crawl Fragment Identifiers" },
          { kind: "bool", key: "perform_html_validation", label: "Perform HTML Validation", help: "The Validation tab." },
          { kind: "bool", key: "assume_pages_are_html", label: "Assume Pages are HTML", help: "Treat responses with no Content-Type as HTML." },
          { kind: "number", key: "response_timeout_secs", label: "Response Timeout", min: 1, unit: "secs" },
          { kind: "number", key: "retries_5xx", label: "5XX Response Retries", min: 0, max: 10 },
        ],
      },
    ],
  },
  {
    id: "preferences",
    label: "Preferences",
    parent: "spider",
    groups: [
      {
        title: "Page Title Width",
        fields: [
          { kind: "number", key: "preferences.title_min_chars", label: "Min characters", min: 0 },
          { kind: "number", key: "preferences.title_max_chars", label: "Max characters", min: 1 },
          { kind: "number", key: "preferences.title_min_px", label: "Min pixels", min: 0 },
          { kind: "number", key: "preferences.title_max_px", label: "Max pixels", min: 1 },
        ],
      },
      {
        title: "Meta Description Width",
        fields: [
          { kind: "number", key: "preferences.description_min_chars", label: "Min characters", min: 0 },
          { kind: "number", key: "preferences.description_max_chars", label: "Max characters", min: 1 },
          { kind: "number", key: "preferences.description_min_px", label: "Min pixels", min: 0 },
          { kind: "number", key: "preferences.description_max_px", label: "Max pixels", min: 1 },
        ],
      },
      {
        title: "Other",
        fields: [
          { kind: "number", key: "preferences.url_max_chars", label: "URL max characters", min: 1 },
          { kind: "number", key: "preferences.h1_max_chars", label: "H1 max characters", min: 1 },
          { kind: "number", key: "preferences.h2_max_chars", label: "H2 max characters", min: 1 },
          { kind: "number", key: "preferences.alt_text_max_chars", label: "Image alt text max characters", min: 1 },
          { kind: "number", key: "preferences.image_max_kb", label: "Max image size", min: 1, unit: "KB" },
          { kind: "number", key: "preferences.low_content_words", label: "Low content word count", min: 0 },
          { kind: "number", key: "preferences.high_crawl_depth", label: "High crawl depth", min: 0 },
          { kind: "number", key: "preferences.high_internal_outlinks", label: "High internal outlinks", min: 0 },
          { kind: "number", key: "preferences.high_external_outlinks", label: "High external outlinks", min: 0 },
          { kind: "number", key: "preferences.html_max_mb", label: "HTML document size limit", min: 1, unit: "MB" },
          { kind: "number", key: "preferences.resource_max_mb", label: "Resource size limit", min: 1, unit: "MB", help: "Validation > Resource Over X MB (CSS, JavaScript, images and other files)." },
          { kind: "lines", key: "preferences.non_descriptive_anchor_text", label: "Non-Descriptive Anchor Text", rows: 5, help: "One phrase per line." },
          { kind: "lines", key: "preferences.soft_404_phrases", label: "Soft 404 error text", rows: 3, help: "Pages whose title or H1 contains one of these phrases (any case) are flagged as soft 404s." },
        ],
      },
    ],
  },
  { id: "content", label: "Content", groups: [] },
  {
    id: "content-area",
    label: "Area",
    parent: "content",
    groups: [
      {
        fields: [
          {
            kind: "note",
            text: "The content area is the text used for word count, readability, near duplicates, Compare > Content and Custom Search > Content Area. It does not affect link discovery.",
          },
          {
            kind: "select",
            key: "content_area.mode",
            label: "Mode",
            options: [
              ["exclude", "Exclude (the exclude lists apply)"],
              ["include", "Include (only the include lists apply)"],
              ["both", "Both (include and exclude lists together)"],
            ],
            help: "With Include and empty include lists, the whole body counts.",
          },
          { kind: "bool", key: "content_area.check_alt_text", label: "Check Alt Text", help: "Count image alt text as content." },
          { kind: "lines", key: "content_area.include_elements", label: "Include HTML elements", rows: 2, placeholder: "main" },
          { kind: "lines", key: "content_area.exclude_elements", label: "Exclude HTML elements", rows: 3 },
          { kind: "lines", key: "content_area.include_classes", label: "Include classes", rows: 2 },
          { kind: "lines", key: "content_area.exclude_classes", label: "Exclude classes", rows: 2 },
          { kind: "lines", key: "content_area.include_ids", label: "Include IDs", rows: 2 },
          { kind: "lines", key: "content_area.exclude_ids", label: "Exclude IDs", rows: 2 },
        ],
      },
    ],
  },
  {
    id: "duplicates",
    label: "Duplicates",
    parent: "content",
    groups: [
      {
        fields: [
          { kind: "bool", key: "near_duplicates_indexable_only", label: "Only Check Indexable Pages for Duplicates" },
          { kind: "bool", key: "near_duplicates", label: "Enable Near Duplicates", help: "Also needed for Content change detection in Compare mode." },
          {
            kind: "percent",
            key: "near_duplicate_threshold",
            label: "Near Duplicate Similarity Threshold",
            min: 1,
            disabledIf: (c) => !c.near_duplicates,
          },
        ],
      },
    ],
  },
  {
    id: "spelling",
    label: "Spelling & Grammar",
    parent: "content",
    groups: [
      {
        title: "Spelling & Grammar",
        fields: [
          { kind: "bool", key: "spelling.spell_check", label: "Enable Spell Check", help: "Checks titles, meta descriptions and the content area. A language's dictionary is downloaded the first time it's needed." },
          { kind: "bool", key: "spelling.grammar_check", label: "Enable Grammar Check" },
          {
            kind: "text",
            key: "spelling.grammar_server",
            label: "LanguageTool server",
            placeholder: "http://localhost:8081",
            help: "Grammar is checked by a LanguageTool server. Run one locally to keep page text on this computer; a remote address sends page text to that server.",
          },
          {
            kind: "select",
            key: "spelling.language_mode",
            label: "Language",
            options: [
              ["auto", "Auto (each page's lang attribute)"],
              ["manual", "Manual (one language for every page)"],
            ],
          },
          {
            kind: "spelling-language",
            key: "spelling.language",
            label: "Language / dialect",
            help: "Manual: the language for every page. Auto: the dialect for a bare lang such as en, and the language for pages with no lang.",
          },
        ],
      },
      {
        title: "Grammar Rules",
        fields: [
          { kind: "lines", key: "spelling.disabled_grammar_rules", label: "Disabled grammar rules", rows: 3, placeholder: "EN_A_VS_AN", help: "LanguageTool rule ids, one per line. Saved with the profile." },
        ],
      },
      {
        title: "Ignore",
        fields: [{ kind: "lines", key: "spelling.ignore_words", label: "Ignore", rows: 3, help: "Words skipped in this crawl's configuration, one per line." }],
      },
      {
        title: "Dictionary",
        fields: [{ kind: "global-dictionary", label: "Dictionary", help: "Your words for every crawl and profile (kept by the app, not in profiles). Saved straight away." }],
      },
    ],
  },
  {
    id: "robots",
    label: "robots.txt",
    groups: [
      {
        fields: [
          {
            kind: "select",
            key: "robots_mode",
            label: "robots.txt",
            options: [
              ["respect", "Respect robots.txt"],
              ["ignore", "Ignore robots.txt"],
              ["ignore_but_report", "Ignore robots.txt but report status"],
            ],
            help: "List mode defaults to Ignore.",
          },
          {
            kind: "bool",
            key: "show_internal_blocked_by_robots",
            label: "Show Internal URLs Blocked by robots.txt",
            disabledIf: (c) => c.robots_mode === "ignore",
          },
          {
            kind: "bool",
            key: "show_external_blocked_by_robots",
            label: "Show External URLs Blocked by robots.txt",
            disabledIf: (c) => c.robots_mode === "ignore",
          },
          {
            kind: "custom-robots",
            key: "custom_robots",
            label: "Custom Robots",
            help: "Override the live robots.txt of a subdomain, e.g. to test changes before they go live. Matching uses the Robots User-Agent: its own group, otherwise *; the longest rule wins and Allow wins a tie.",
          },
        ],
      },
    ],
  },
  {
    id: "rewriting",
    label: "URL Rewriting",
    groups: [
      {
        fields: [
          { kind: "bool", key: "remove_all_parameters", label: "Remove all parameters" },
          {
            kind: "lines",
            key: "remove_parameters",
            label: "Remove Parameters",
            placeholder: "sessionid",
            help: "Query parameter names to strip from discovered URLs, one per line.",
            disabledIf: (c) => c.remove_all_parameters,
          },
          {
            kind: "rewrite-rules",
            key: "url_rewrite_rules",
            label: "Regex Replace",
            help: "Applied in order to the whole URL. Use $1 for capture groups. Rust regex syntax: no look-around or back-references inside the pattern.",
          },
          { kind: "bool", key: "lowercase_urls", label: "Lowercase discovered URLs" },
          {
            kind: "select",
            key: "percent_encoding_case",
            label: "Percent-encoding case",
            options: [
              ["uppercase", "Uppercase (%C3%A9)"],
              ["lowercase", "Lowercase (%c3%a9)"],
              ["none", "None (as found)"],
            ],
          },
          { kind: "url-test", label: "Test", show: "rewrite" },
          { kind: "note", text: "Rewriting applies to discovered URLs, not to the start URL or List mode URLs." },
        ],
      },
    ],
  },
  {
    id: "cdns",
    label: "CDNs",
    groups: [
      {
        fields: [
          {
            kind: "lines",
            key: "cdns",
            label: "CDNs",
            rows: 6,
            placeholder: "cdn.example.com\nassets.example.net/images/",
            help: "Hosts (or host/path prefixes) treated as Internal, e.g. an image CDN or the other ccTLDs of an hreflang set. One per line.",
          },
          { kind: "url-test", label: "Test", show: "cdn" },
        ],
      },
    ],
  },
  {
    id: "include",
    label: "Include",
    groups: [
      {
        fields: [
          {
            kind: "lines",
            key: "include",
            label: "Include",
            rows: 6,
            placeholder: "https://www.example.com/blog/.*",
            help: "Only internal URLs matching one of these regexes are crawled (partial match, case-sensitive). The start URL is always crawled; in List mode every uploaded URL must match.",
          },
          { kind: "url-test", label: "Test", show: "include" },
        ],
      },
    ],
  },
  {
    id: "exclude",
    label: "Exclude",
    groups: [
      {
        fields: [
          {
            kind: "lines",
            key: "exclude",
            label: "Exclude",
            rows: 6,
            placeholder: ".*\\?sort=.*",
            help: "URLs matching one of these regexes are never requested (partial match, case-sensitive), including robots.txt files and List mode URLs. Changes made during a crawl apply to URLs not yet requested.",
          },
          { kind: "url-test", label: "Test", show: "exclude" },
        ],
      },
    ],
  },
  {
    id: "speed",
    label: "Speed",
    groups: [
      {
        fields: [
          { kind: "number", key: "threads", label: "Max Threads", min: 1, max: 200 },
          { kind: "limit", key: "max_urls_per_sec", label: "Limit URL/s", whenOn: 1, min: 0.1, step: 0.1, unit: "URL/s" },
        ],
      },
    ],
  },
  {
    id: "user-agent",
    label: "User-Agent",
    groups: [
      {
        fields: [
          { kind: "ua-preset", label: "Preset User Agents" },
          { kind: "text", key: "user_agent", label: "HTTP Request User-Agent" },
          { kind: "text", key: "robots_user_agent", label: "Robots User-Agent", help: "The token matched against robots.txt groups and robots meta names." },
        ],
      },
    ],
  },
  {
    id: "http-header",
    label: "HTTP Header",
    groups: [{ fields: [{ kind: "headers", key: "custom_headers", label: "Request headers", help: "Sent with every request. The User-Agent comes from Configuration > User-Agent." }] }],
  },
  { id: "custom", label: "Custom", groups: [] },
  { id: "custom-search", label: "Search", parent: "custom", groups: [{ fields: [{ kind: "custom-search" }] }] },
  { id: "custom-extraction", label: "Extraction", parent: "custom", groups: [{ fields: [{ kind: "custom-extraction" }] }] },
  { id: "custom-javascript", label: "Custom JavaScript", parent: "custom", groups: [{ fields: [{ kind: "custom-javascript" }] }] },
  {
    id: "link-positions",
    label: "Link Positions",
    parent: "custom",
    groups: [
      {
        fields: [
          { kind: "bool", key: "link_positions_enabled", label: "Enable", help: "Off: the Link Path and Link Position columns are left empty, which saves memory on very large crawls." },
          { kind: "link-positions", key: "link_positions", label: "Link Positions" },
        ],
      },
    ],
  },
  {
    id: "authentication",
    label: "Authentication",
    groups: [{ fields: [{ kind: "authentication", label: "Authentication login password Basic Digest forms cookies" }] }],
  },
];

// ------------------------------------------------------------------ helpers

export function getPath(obj: unknown, path: string): unknown {
  return path.split(".").reduce<unknown>((o, k) => (o as Record<string, unknown> | undefined)?.[k], obj);
}

export function setPath<T>(obj: T, path: string, value: unknown): T {
  const [head, ...rest] = path.split(".");
  const o = obj as Record<string, unknown>;
  return { ...o, [head]: rest.length ? setPath(o[head], rest.join("."), value) : value } as T;
}

/** Every (key, label, node) the schema knows about, for search and diffs. */
export function allFields(): { key: string; label: string; node: Node; group?: string }[] {
  const out: { key: string; label: string; node: Node; group?: string }[] = [];
  for (const node of NODES) {
    for (const g of node.groups) {
      for (const f of g.fields) {
        const label = "label" in f ? f.label : f.kind === "custom-search" ? "Custom Search" : f.kind === "custom-extraction" ? "Custom Extraction" : f.kind === "custom-javascript" ? "Custom JavaScript" : "";
        if (f.kind === "pair") {
          if (f.crawl) out.push({ key: f.crawl, label: `${f.label}: Crawl`, node, group: g.title });
          if (f.store) out.push({ key: f.store, label: `${f.label}: Store`, node, group: g.title });
        } else if ("key" in f) {
          out.push({ key: f.key, label, node, group: g.title });
        } else if (f.kind === "custom-search") {
          out.push({ key: "custom_search", label, node });
        } else if (f.kind === "custom-extraction") {
          out.push({ key: "custom_extraction", label, node });
        } else if (f.kind === "custom-javascript") {
          out.push({ key: "custom_javascript", label, node });
        }
      }
    }
  }
  return out;
}

/** Breadcrumb for a node, e.g. "Spider > Crawl". */
export function nodePath(node: Node): string {
  const parent = node.parent ? NODES.find((n) => n.id === node.parent) : undefined;
  return parent ? `${parent.label} > ${node.label}` : node.label;
}

/** Settings that differ from the defaults. */
export function changedFields(cfg: CrawlConfig, defaults: CrawlConfig) {
  return allFields().filter((f) => JSON.stringify(getPath(cfg, f.key)) !== JSON.stringify(getPath(defaults, f.key)));
}

export function formatValue(v: unknown): string {
  if (v === null || v === undefined) return "Off";
  if (typeof v === "boolean") return v ? "✓" : "✗";
  if (Array.isArray(v)) {
    if (v.length === 0) return "(empty)";
    if (Array.isArray(v[0])) return (v as [string, string][]).map(([k, val]) => `${k}: ${val}`).join("; ");
    if (typeof v[0] === "object") return `${v.length} rule${v.length === 1 ? "" : "s"}`;
    return v.join(", ");
  }
  return String(v);
}
