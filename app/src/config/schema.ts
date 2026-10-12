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
  search_in: "html" | "html_head" | "html_body" | "page_text" | "content_area";
}

export interface CustomExtractor {
  name: string;
  kind: "css" | "regex";
  expression: string;
  attribute: string | null;
  extract: "element" | "inner_html" | "text";
}

export interface ContentArea {
  include_elements: string[];
  exclude_elements: string[];
  include_classes: string[];
  exclude_classes: string[];
  include_ids: string[];
  exclude_ids: string[];
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
  crawl_internal_links: boolean;
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
  ignore_non_indexable_for_issues: boolean;
  response_timeout_secs: number;
  retries_5xx: number;
  assume_pages_are_html: boolean;
  threads: number;
  max_urls_per_sec: number | null;
  robots_mode: RobotsMode;
  show_internal_blocked_by_robots: boolean;
  show_external_blocked_by_robots: boolean;
  user_agent: string;
  robots_user_agent: string;
  custom_headers: [string, string][];
  include: string[];
  exclude: string[];
  remove_parameters: string[];
  lowercase_urls: boolean;
  content_area: ContentArea;
  extract_structured_data: boolean;
  near_duplicates: boolean;
  near_duplicate_threshold: number;
  near_duplicates_indexable_only: boolean;
  custom_search: CustomSearch[];
  custom_extraction: CustomExtractor[];
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
  | { kind: "pair"; label: string; store?: string; crawl: string; help?: string; storePending?: string }
  | { kind: "headers"; key: string; label: string; help?: string }
  | { kind: "custom-search" }
  | { kind: "custom-extraction" }
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

const STORE_PENDING = "Store is not honoured yet: every link is recorded. It arrives with the Spider work package.";

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
          { kind: "pair", label: "Images", store: "store_images", crawl: "crawl_images", storePending: STORE_PENDING },
          { kind: "pair", label: "CSS", store: "store_css", crawl: "crawl_css", storePending: STORE_PENDING },
          { kind: "pair", label: "JavaScript", store: "store_js", crawl: "crawl_js", storePending: STORE_PENDING },
        ],
      },
      {
        title: "Page Links",
        fields: [
          { kind: "pair", label: "Internal Hyperlinks", crawl: "crawl_internal_links" },
          { kind: "pair", label: "External Links", store: "store_external_links", crawl: "crawl_external_links", storePending: STORE_PENDING },
          { kind: "pair", label: "Canonicals", store: "store_canonicals", crawl: "crawl_canonicals", storePending: STORE_PENDING },
          { kind: "pair", label: "Pagination (Rel Next/Prev)", store: "store_pagination", crawl: "crawl_pagination", storePending: STORE_PENDING },
          { kind: "pair", label: "Hreflang", store: "store_hreflang", crawl: "crawl_hreflang", storePending: STORE_PENDING },
          { kind: "pair", label: "Meta Refresh", crawl: "crawl_meta_refresh" },
          { kind: "pair", label: "iframes", crawl: "crawl_iframes" },
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
        title: "Structured Data",
        fields: [
          {
            kind: "bool",
            key: "extract_structured_data",
            label: "JSON-LD and Microdata, with Google Rich Result Feature Validation",
            help: "On by default in Toad (Screaming Frog has it off).",
          },
        ],
      },
      {
        fields: [
          {
            kind: "note",
            text: "Page details (title, meta description, H1/H2, indexability, word count, readability, hash, size, response time, meta robots, X-Robots-Tag) are always extracted. Per-element toggles, Store HTML and PDF extraction are planned.",
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
          { kind: "limit", key: "max_depth", label: "Limit Crawl Depth", whenOn: 3, min: 0, help: "Clicks from the start URL. A redirect counts as one level." },
          { kind: "limit", key: "max_folder_depth", label: "Limit Max Folder Depth", whenOn: 5, min: 0 },
          { kind: "limit", key: "max_query_params", label: "Limit Number of Query Strings", whenOn: 5, min: 0 },
          { kind: "number", key: "max_redirects", label: "Max Redirects to Follow", min: 0, max: 20 },
          { kind: "number", key: "max_url_length", label: "Max URL Length to Crawl", min: 1, unit: "characters" },
          { kind: "number", key: "max_links_per_url", label: "Max Links per URL to Crawl", min: 1 },
          { kind: "number", key: "max_page_size_kb", label: "Max Page Size", min: 1, unit: "KB", help: "Larger pages are reported as not processed." },
        ],
      },
    ],
  },
  {
    id: "rendering",
    label: "Rendering",
    parent: "spider",
    groups: [],
    planned: "Toad crawls in Text Only mode. JavaScript rendering (headless Chrome), screenshots and the window-size presets are planned.",
  },
  {
    id: "advanced",
    label: "Advanced",
    parent: "spider",
    groups: [
      {
        fields: [
          { kind: "bool", key: "ignore_non_indexable_for_issues", label: "Ignore Non-Indexable URLs for Issues" },
          { kind: "bool", key: "crawl_fragment_identifiers", label: "Crawl Fragment Identifiers" },
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
            text: "The content area is the text used for word count, readability, near duplicates and Custom Search > Content Area. Include lists take effect when they are not empty; exclude lists always apply.",
          },
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
          { kind: "bool", key: "show_internal_blocked_by_robots", label: "Show Internal URLs Blocked by robots.txt" },
          { kind: "bool", key: "show_external_blocked_by_robots", label: "Show External URLs Blocked by robots.txt" },
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
          { kind: "lines", key: "remove_parameters", label: "Remove Parameters", placeholder: "sessionid", help: "Query parameter names to strip from discovered URLs, one per line." },
          { kind: "bool", key: "lowercase_urls", label: "Lowercase discovered URLs" },
          { kind: "note", text: "Rewriting applies to discovered URLs, not to the start URL or List mode URLs." },
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
            help: "Only internal URLs matching one of these regexes are crawled (partial match, case-sensitive). The start URL is always crawled.",
          },
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
            help: "URLs matching one of these regexes are never requested (partial match, case-sensitive).",
          },
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
        const label = "label" in f ? f.label : f.kind === "custom-search" ? "Custom Search" : f.kind === "custom-extraction" ? "Custom Extraction" : "";
        if (f.kind === "pair") {
          out.push({ key: f.crawl, label: `${f.label}: Crawl`, node, group: g.title });
          if (f.store) out.push({ key: f.store, label: `${f.label}: Store`, node, group: g.title });
        } else if ("key" in f) {
          out.push({ key: f.key, label, node, group: g.title });
        } else if (f.kind === "custom-search") {
          out.push({ key: "custom_search", label, node });
        } else if (f.kind === "custom-extraction") {
          out.push({ key: "custom_extraction", label, node });
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
