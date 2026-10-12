// Fake backend used when the UI runs in a plain browser (`npm run dev`
// without Tauri), so layout and interactions can be checked without a crawl.

import type { CompareOverviewRow, CompareSummary, IssueRow, OverviewRow, TabDef, Table } from "./api";

const TABS: TabDef[] = [
  { name: "Internal", columns: [], filters: ["All", "HTML", "JavaScript", "CSS", "Images", "PDF", "Other", "Unknown"].map((name) => ({ name, issue: null, description: "" })) },
  { name: "Response Codes", columns: [], filters: [
    { name: "All", issue: null, description: "" },
    { name: "Internal Client Error (4XX)", issue: ["Issue", "High"], description: "Client errors such as 404 and 410." },
    { name: "Internal Redirection (3XX)", issue: ["Warning", "Low"], description: "Server-side redirects." },
  ] },
  { name: "Page Titles", columns: [], filters: [
    { name: "All", issue: null, description: "" },
    { name: "Missing", issue: ["Issue", "High"], description: "No title, or an empty one." },
    { name: "Duplicate", issue: ["Opportunity", "Medium"], description: "The same title appears on more than one page." },
    { name: "Over 60 Characters", issue: ["Opportunity", "Medium"], description: "Title may be truncated in search results." },
  ] },
  ...["Meta Description", "H1", "H2", "Content", "Images", "Canonicals", "Directives", "Links", "Sitemaps", "Custom Search"].map((name) => ({
    name,
    columns: [],
    filters: [{ name: "All", issue: null, description: "" }],
  })),
];

const COLUMNS = ["Address", "Content Type", "Status Code", "Status", "Indexability", "Indexability Status", "Title 1", "Title 1 Length", "Title 1 Pixel Width", "Word Count", "Crawl Depth", "Link Score", "Inlinks", "Response Time"];

function row(i: number): string[] {
  const status = i % 17 === 0 ? 404 : i % 11 === 0 ? 301 : 200;
  const title = status === 200 ? `Example page ${i} – toads, frogs and other amphibians` : "";
  return [
    `https://www.example.com/section-${i % 7}/page-${i}`,
    "text/html; charset=utf-8",
    String(status),
    status === 200 ? "OK" : status === 301 ? "Moved Permanently" : "Not Found",
    status === 200 ? "Indexable" : "Non-Indexable",
    status === 200 ? "" : status === 301 ? "Redirected" : "Client Error",
    title,
    String(title.length),
    String(Math.round(title.length * 8.6)),
    String(150 + ((i * 37) % 900)),
    String(1 + (i % 5)),
    String(Math.max(1, 100 - i)),
    String((i * 13) % 60),
    (0.1 + (i % 9) / 10).toFixed(2),
  ];
}

const TOTAL = 1250;

const ISSUES: IssueRow[] = [
  { tab: "Response Codes", filter: "Internal Client Error (4XX)", issue_type: "Issue", priority: "High", urls: 74, pct_of_total: 5.9, description: "Client errors such as 404 and 410." },
  { tab: "Page Titles", filter: "Missing", issue_type: "Issue", priority: "High", urls: 3, pct_of_total: 0.3, description: "No title, or an empty one." },
  { tab: "Response Codes", filter: "Internal Redirection (3XX)", issue_type: "Warning", priority: "Low", urls: 107, pct_of_total: 8.6, description: "Server-side redirects." },
  { tab: "Page Titles", filter: "Duplicate", issue_type: "Opportunity", priority: "Medium", urls: 212, pct_of_total: 19.4, description: "The same title appears on more than one page." },
  { tab: "Page Titles", filter: "Over 60 Characters", issue_type: "Opportunity", priority: "Medium", urls: 88, pct_of_total: 8.1, description: "Title may be truncated in search results." },
];

const OVERVIEW: OverviewRow[] = TABS.flatMap((t) =>
  t.filters.map((f, i) => ({ tab: t.name, filter: f.name, urls: i === 0 ? TOTAL : (f.name.length * 7) % 120 })),
);

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 30));
  switch (cmd) {
    case "get_tabs":
      return TABS as T;
    case "get_rows": {
      const req = (args?.req ?? {}) as { offset: number; limit: number; search?: string };
      let rows = Array.from({ length: TOTAL }, (_, i) => row(i + 1));
      if (req.search) rows = rows.filter((r) => r[0].includes(req.search!));
      const table: Table = { columns: COLUMNS, rows: rows.slice(req.offset, req.offset + req.limit), total: rows.length };
      return table as T;
    }
    case "get_issues":
      return ISSUES as T;
    case "get_overview":
      return OVERVIEW as T;
    case "get_inlinks":
    case "get_outlinks":
      return {
        columns: ["Type", "From", "To", "Anchor Text", "Alt Text", "Follow", "Target", "Rel", "Status Code", "Status", "Path Type", "Link Path", "Link Position"],
        rows: [["Hyperlink", "https://www.example.com/", String(args?.url), "Read more", "", "true", "", "", "200", "OK", "Root-Relative", "/html/body/main/a", "Content"]],
        total: 1,
      } as T;
    case "get_reports":
      return [
        { name: "Redirect Chains", kind: "Report", description: "Redirects with two or more hops." },
        { name: "Orphan Pages", kind: "Report", description: "In a sitemap, not linked." },
        { name: "All Inlinks", kind: "BulkExport", description: "Every link." },
      ] as T;
    case "get_structured_details":
      return {
        columns: ["Kind", "Format", "Type", "Property", "Detail"],
        rows: [["Item", "JSON-LD", "Product", "", "name, offers"], ["Error", "", "Offer", "priceCurrency", "Missing required property 'priceCurrency'."]],
        total: 2,
      } as T;
    case "get_near_duplicates":
      return { columns: ["Address", "Similarity"], rows: [["https://www.example.com/section-1/page-8", "94.5%"]], total: 1 } as T;
    case "get_url_details":
      return [["address", String(args?.url)], ["status_code", "200"], ["title_1", "Example page"], ["word_count", "512"]] as T;
    case "get_config":
    case "reset_config":
    case "builtin_config":
    case "clear_default_profile":
      return structuredClone(DEFAULT_CONFIG) as T;
    case "validate_config": {
      const c = args?.config as typeof DEFAULT_CONFIG;
      const problems: string[] = [];
      for (const p of [...c.include, ...c.exclude]) {
        try {
          new RegExp(p);
        } catch {
          problems.push(`Include/Exclude: '${p}' is not a valid regex`);
        }
      }
      if (c.threads < 1) problems.push("Speed: Max Threads must be at least 1");
      return problems as T;
    }
    case "test_url": {
      const url = String(args?.url ?? "");
      return { rewritten: url.replace(/#.*$/, ""), encoded: url, internal: args?.start ? url.includes("example.com") : null, include_match: null, included: true, exclude_match: url.includes("?sort") ? "\\?sort" : null, error: null } as T;
    }
    case "test_robots": {
      const blocked = String(args?.url).includes("/private");
      return { allowed: !blocked, line: blocked ? "Line 2: Disallow: /private/" : null, group: "*" } as T;
    }
    case "download_robots":
      return "User-agent: *\nDisallow: /private/\nSitemap: https://www.example.com/sitemap.xml\n" as T;
    case "recent_profiles":
      return [{ path: "C:/profiles/ecommerce.toadconfig", name: "ecommerce", exists: true }, { path: "C:/old/gone.toadconfig", name: "gone", exists: false }] as T;
    case "has_default_profile":
      return false as T;
    case "get_page_source":
      return MOCK_HTML_CURRENT as T;
    case "compare_source": {
      const a = MOCK_HTML_PREVIOUS.split(/\r?\n/);
      const b = MOCK_HTML_CURRENT.split(/\r?\n/);
      const lines = [
        ...a.slice(0, 3).map((t, i) => ({ tag: "equal", old: i + 1, new: i + 1, text: t })),
        { tag: "delete", old: 4, new: null, text: a[3] },
        { tag: "insert", old: null, new: 4, text: b[3] },
        { tag: "insert", old: null, new: 5, text: b[4] },
        ...a.slice(4).map((t, i) => ({ tag: "equal", old: i + 5, new: i + 6, text: t })),
      ];
      return { current: MOCK_HTML_CURRENT, previous: MOCK_HTML_PREVIOUS, lines, changed: 3 } as T;
    }
    case "crawls_list":
      return MOCK_CRAWLS as T;
    case "current_crawl_path":
      return MOCK_CRAWLS[0].path as T;
    case "get_app_settings":
      return { storage_dir: null, retention_days: null, last_mode: "spider" } as T;
    case "read_url_file":
    case "fetch_sitemap_urls":
      return ["https://www.example.com/a", "https://www.example.com/b", "https://www.example.com/a"] as T;
    case "list_crawls":
      return [
        { path: "C:/crawls/example-2.toad", name: "example-2", start_url: "https://www.example.com/", urls: 1250, modified: 1791800000 },
        { path: "C:/crawls/example-1.toad", name: "example-1", start_url: "https://www.example.com/", urls: 1180, modified: 1791200000 },
      ] as T;
    case "get_compare_config":
      return { elements: ["Indexability", "PageTitles", "MetaDescription", "H1", "WordCount", "CrawlDepth", "Inlinks", "UniqueInlinks", "InternalOutlinks", "UniqueInternalOutlinks", "ExternalOutlinks", "UniqueExternalOutlinks", "StructuredDataUniqueTypes", "Content"], similarity_change_pct: 10, ignore_non_indexable: false, url_mapping: [{ regex: "staging\.example\.com", replace: "www.example.com" }] } as T;
    case "test_url_mapping":
      return String(args?.url).replace("staging.example.com", "www.example.com") as T;
    case "run_compare":
    case "open_comparison":
    case "compare_summary":
      return COMPARE_SUMMARY as T;
    case "compare_tabs":
      return [
        ...TABS,
        { name: "Change Detection", columns: [], filters: ["All", "Indexability", "Page Titles", "Meta Description", "H1", "Word Count", "Crawl Depth", "Content"].map((name) => ({ name, issue: null, description: `Pages that have changed ${name.toLowerCase()}.` })) },
      ] as T;
    case "compare_overview":
      return COMPARE_OVERVIEW as T;
    case "compare_issues":
      return ISSUES.map((i, n) => ({ tab: i.tab, filter: i.filter, issue_type: i.issue_type, priority: i.priority, current: i.urls, previous: i.urls + (n % 2 ? 12 : -9), change: n % 2 ? -12 : 9, change_pct: n % 2 ? -10.5 : 13.6, added: n % 2 ? 0 : 11, new: 3, removed: n % 2 ? 14 : 2, missing: 1, description: i.description })) as T;
    case "compare_rows": {
      const req = (args?.req ?? {}) as { tab: string; filter: string; state: string; offset: number; limit: number };
      if (req.tab === "Change Detection") {
        const rows = Array.from({ length: 42 }, (_, i) => [`https://www.example.com/section-${i % 7}/page-${i}`, `Example page ${i} – toads and frogs`, `Example page ${i} – toads`, ]);
        return { columns: ["Address", "Current Page Title", "Previous Page Title"], rows: rows.slice(req.offset, req.offset + req.limit), total: rows.length } as T;
      }
      const n = req.state === "Current" ? TOTAL : req.state === "Previous" ? 1180 : 37;
      const rows = Array.from({ length: n }, (_, i) => row(i + 1));
      return { columns: COLUMNS, rows: rows.slice(req.offset, req.offset + req.limit), total: n } as T;
    }
    case "compare_url_details":
      return { columns: ["Name", "Current Value", "Previous Value"], rows: [["address", String(args?.url), String(args?.url)], ["status_code", "200", "200"], ["title_1", "Example page – toads and frogs", "Example page – toads"], ["word_count", "512", "498"], ["depth", "2", "2"]], total: 5 } as T;
    case "compare_links":
      return {
        columns: ["Type", "From", "To", "Anchor Text", "Alt Text", "Follow", "Target", "Rel", "Status Code", "Status", "Path Type", "Link Path", "Link Position"],
        rows: [["Hyperlink", "https://www.example.com/", String(args?.url), `${args?.linkState} link`, "", "true", "", "", "200", "OK", "Root-Relative", "/html/body/main/a", "Content"]],
        total: 1,
      } as T;
    case "compare_site_structure":
      return { columns: ["Path", "Current URLs", "Previous URLs", "Change", "Change %"], rows: [["/", "1250", "1180", "70", "5.93"], ["/section-1/", "180", "190", "-10", "-5.26"], ["/section-2/", "175", "160", "15", "9.38"], ["/blog/", "40", "0", "40", ""]], total: 4 } as T;
    default:
      return undefined as T;
  }
}

const COMPARE_SUMMARY: CompareSummary = {
  current_path: "C:/crawls/example-2.toad",
  previous_path: "C:/crawls/example-1.toad",
  current_start_url: "https://www.example.com/",
  previous_start_url: "https://www.example.com/",
  current_urls: 1250,
  previous_urls: 1180,
  in_both: 1143,
  content_available: true,
  config: { elements: ["PageTitles", "Content"], similarity_change_pct: 10, ignore_non_indexable: false, url_mapping: [] },
};

const COMPARE_OVERVIEW: CompareOverviewRow[] = [
  ...OVERVIEW.map((o, i) => {
    const previous = Math.max(0, o.urls - ((i * 7) % 23) + 9);
    const change = o.urls - previous;
    const removed = (i * 3) % 5;
    const missing = (i * 5) % 4;
    const added = Math.max(0, change + removed + missing - 2);
    const neu = change + removed + missing - added;
    return { tab: o.tab, filter: o.filter, current: o.urls, previous, change, change_pct: previous ? Math.round((change * 10000) / previous) / 100 : null, added, new: neu, removed, missing };
  }),
  ...["All", "Indexability", "Page Titles", "Meta Description", "H1", "Word Count", "Crawl Depth", "Content"].map((f, i) => {
    const n = f === "All" ? 1143 : (i * 37) % 210;
    return { tab: "Change Detection", filter: f, current: n, previous: n, change: f === "All" ? 0 : n, change_pct: f === "All" ? null : Math.round((n * 10000) / 1143) / 100, added: null, new: null, removed: null, missing: null };
  }),
];

const DEFAULT_CONFIG = {
  mode: "spider" as const,
  store_images: true, crawl_images: true, store_css: true, crawl_css: true, store_js: true, crawl_js: true,
  store_internal_links: true, crawl_internal_links: true, store_external_links: true,
  store_media: false, crawl_media: false, store_swf: true, crawl_swf: true, store_amp: false, crawl_amp: false,
  store_meta_refresh: true, store_iframes: true, store_mobile_alternate: false, crawl_mobile_alternate: false,
  store_uncrawlable_links: false, crawl_invalid_links: false, crawl_external_links: true, store_canonicals: true, crawl_canonicals: true,
  store_pagination: true, crawl_pagination: false, store_hreflang: true, crawl_hreflang: false, crawl_meta_refresh: true, crawl_iframes: true,
  check_links_outside_start_folder: true, crawl_outside_start_folder: false, crawl_all_subdomains: false,
  follow_internal_nofollow: false, follow_external_nofollow: false, crawl_fragment_identifiers: false,
  crawl_linked_sitemaps: false, auto_discover_sitemaps_via_robots: true, sitemap_urls: [] as string[],
  max_urls: 5000000, max_depth: null as number | null, max_folder_depth: null as number | null, max_query_params: null as number | null,
  max_redirects: 10, max_url_length: 10000, max_links_per_url: 10000, max_page_size_kb: 50000,
  max_urls_per_subdomain: null as number | null, max_urls_per_depth: null as number | null, path_limits: [] as { pattern: string; max: number }[],
  cookie_storage: "session_only" as const, ignore_paginated_for_duplicates: true, always_follow_redirects: false, always_follow_canonicals: false,
  respect_noindex: false, respect_canonical: false, respect_next_prev: false, respect_hsts: true, respect_self_referencing_meta_refresh: true,
  extract_images_from_srcset: false, perform_html_validation: true,
  ignore_non_indexable_for_issues: true, response_timeout_secs: 20, retries_5xx: 0, assume_pages_are_html: false,
  threads: 5, max_urls_per_sec: null as number | null, robots_mode: "respect" as const,
  show_internal_blocked_by_robots: true, show_external_blocked_by_robots: false, custom_robots: [] as { host: string; enabled: boolean; txt: string }[],
  user_agent: "Screaming Toad SEO Spider/0.1.0", robots_user_agent: "Screaming Toad",
  custom_headers: [["Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"], ["Accept-Encoding", "gzip"], ["Cache-Control", "no-cache"], ["Pragma", "no-cache"]] as [string, string][],
  include: [] as string[], exclude: [] as string[], remove_all_parameters: false, remove_parameters: [] as string[],
  url_rewrite_rules: [] as { regex: string; replace: string }[], lowercase_urls: false, percent_encoding_case: "uppercase" as const, cdns: [] as string[],
  content_area: { include_elements: [], exclude_elements: ["nav", "footer"], include_classes: [], exclude_classes: [], include_ids: [], exclude_ids: [] },
  extraction: { page_title: true, meta_description: true, meta_keywords: true, h1: true, h2: true, word_count: true, readability: true, text_to_code_ratio: true, hash: true, page_size: true, forms: true, response_time: true, last_modified: true, http_headers: false, cookies: false, meta_robots: true, x_robots_tag: true },
  extract_structured_data: true, near_duplicates: false, near_duplicate_threshold: 0.9, near_duplicates_indexable_only: true,
  custom_search: [], custom_extraction: [],
  preferences: {
    title_min_chars: 30, title_max_chars: 60, title_min_px: 200, title_max_px: 561,
    description_min_chars: 70, description_max_chars: 155, description_min_px: 400, description_max_px: 985,
    url_max_chars: 115, h1_max_chars: 70, h2_max_chars: 70, alt_text_max_chars: 100, image_max_kb: 100,
    low_content_words: 200, high_crawl_depth: 3, high_internal_outlinks: 1000, high_external_outlinks: 10, html_max_mb: 2,
    non_descriptive_anchor_text: ["click here", "click this", "find out more", "go", "here", "learn more", "more", "over here", "read here", "read more", "right here", "start", "this"],
    soft_404_phrases: ["Page Not Found", "404 Page Can't Be Found"],
  },
};

const MOCK_HTML_PREVIOUS = `<!doctype html>
<html lang="en">
<head>
<title>Example page – toads</title>
<meta name="description" content="All about toads">
</head>
<body><h1>Toads</h1></body>
</html>`;

const MOCK_HTML_CURRENT = `<!doctype html>
<html lang="en">
<head>
<title>Example page – toads and frogs</title>
<link rel="canonical" href="https://www.example.com/page">
<meta name="description" content="All about toads">
</head>
<body><h1>Toads</h1></body>
</html>`;

const MOCK_CRAWLS = [
  { path: "C:/crawls/www_example_com-1791800000.toad", name: "www.example.com", project: "Example Ltd", start_url: "https://www.example.com/", mode: "spider", urls: 1250, queued: 0, pct_complete: 100, status: "Complete", created: 1791800000, modified: 1791800400, size_bytes: 48_000_000, version: "0.1.0", locked: true },
  { path: "C:/crawls/staging-1791700000.toad", name: "Staging before launch", project: "Example Ltd", start_url: "https://staging.example.com/", mode: "spider", urls: 812, queued: 431, pct_complete: 65.3, status: "Stopped", created: 1791700000, modified: 1791700300, size_bytes: 31_000_000, version: "0.1.0", locked: false },
  { path: "C:/crawls/books-1791600000.toad", name: "books.toscrape.com", project: "", start_url: "https://books.toscrape.com/", mode: "spider", urls: 150, queued: 0, pct_complete: 100, status: "Complete", created: 1791600000, modified: 1791600100, size_bytes: 4_200_000, version: "0.1.0", locked: false },
  { path: "C:/crawls/list-1791500000.toad", name: "Redirect check", project: "", start_url: "https://www.example.com/old", mode: "list", urls: 38, queued: 0, pct_complete: 100, status: "Crawling", created: 1791500000, modified: 1791500050, size_bytes: 900_000, version: "0.1.0", locked: false },
];
