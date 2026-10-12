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
      return {
        max_urls: 5000000, max_depth: null, threads: 5, max_urls_per_sec: null, robots_mode: "respect",
        user_agent: "Screaming Toad SEO Spider/0.1.0", include: [], exclude: [], crawl_all_subdomains: false,
        crawl_external_links: true, crawl_images: true, crawl_css: true, crawl_js: true, follow_internal_nofollow: false,
        crawl_outside_start_folder: false, crawl_linked_sitemaps: false, auto_discover_sitemaps_via_robots: true,
        sitemap_urls: [], near_duplicates: false, near_duplicate_threshold: 0.9, custom_search: [], custom_extraction: [],
      } as T;
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
