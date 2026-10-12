import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen, type EventCallback, type UnlistenFn } from "@tauri-apps/api/event";
import { mockInvoke } from "./mock";

/** True inside the Tauri app; false when the UI is opened in a plain browser. */
const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

const invoke: typeof tauriInvoke = inTauri ? tauriInvoke : (cmd, args) => mockInvoke(cmd, args as Record<string, unknown>);

function listen<T>(event: string, cb: EventCallback<T>): Promise<UnlistenFn> {
  return inTauri ? tauriListen<T>(event, cb) : Promise.resolve(() => {});
}

export type IssueType = "Issue" | "Warning" | "Opportunity";
export type Priority = "High" | "Medium" | "Low";

export interface FilterDef {
  name: string;
  issue: [IssueType, Priority] | null;
  description: string;
}

export interface TabDef {
  name: string;
  columns: string[];
  filters: FilterDef[];
}

export interface Table {
  columns: string[];
  rows: string[][];
  total: number;
}

export interface IssueRow {
  tab: string;
  filter: string;
  issue_type: IssueType;
  priority: Priority;
  urls: number;
  pct_of_total: number;
  description: string;
}

export interface ReportDef {
  name: string;
  kind: "Report" | "BulkExport";
  description: string;
}

export interface OverviewRow {
  tab: string;
  filter: string;
  urls: number;
}

export interface Progress {
  crawled: number;
  queued: number;
  discovered: number;
  elapsed_ms: number;
}

export interface RowsRequest {
  tab: string;
  filter: string;
  search?: string;
  sortColumn?: string;
  sortDesc?: boolean;
  offset: number;
  limit: number;
}

/** Mirrors toad_core::config::CrawlConfig; only the fields the UI edits are typed. */
export interface CrawlConfig {
  max_urls: number;
  max_depth: number | null;
  threads: number;
  max_urls_per_sec: number | null;
  robots_mode: "respect" | "ignore" | "ignore_but_report";
  user_agent: string;
  include: string[];
  exclude: string[];
  crawl_all_subdomains: boolean;
  crawl_external_links: boolean;
  crawl_images: boolean;
  crawl_css: boolean;
  crawl_js: boolean;
  follow_internal_nofollow: boolean;
  crawl_outside_start_folder: boolean;
  crawl_linked_sitemaps: boolean;
  auto_discover_sitemaps_via_robots: boolean;
  sitemap_urls: string[];
  near_duplicates: boolean;
  near_duplicate_threshold: number;
  custom_search: CustomSearch[];
  custom_extraction: CustomExtractor[];
  [key: string]: unknown;
}

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

export type LastMod = "none" | "server_response" | { custom: string };

export interface SitemapOptions {
  include_noindex: boolean;
  include_canonicalised: boolean;
  include_paginated: boolean;
  include_pdfs: boolean;
  last_mod: LastMod;
  priority: boolean;
  change_freq: "none" | "from_last_modified" | "by_depth";
  max_urls_per_file: number;
}

export const api = {
  startCrawl: (url: string) => invoke<string>("start_crawl", { req: { url } }),
  startList: (list: string[]) => invoke<string>("start_crawl", { req: { list } }),
  pause: () => invoke<void>("pause_crawl"),
  resume: () => invoke<void>("resume_crawl"),
  stop: () => invoke<void>("stop_crawl"),
  tabs: () => invoke<TabDef[]>("get_tabs"),
  rows: (req: RowsRequest) => invoke<Table>("get_rows", { req }),
  overview: () => invoke<OverviewRow[]>("get_overview"),
  issues: () => invoke<IssueRow[]>("get_issues"),
  inlinks: (url: string) => invoke<Table>("get_inlinks", { url }),
  outlinks: (url: string) => invoke<Table>("get_outlinks", { url }),
  reports: () => invoke<ReportDef[]>("get_reports"),
  exportReport: (name: string, path: string) => invoke<number>("export_report", { name, path }),
  structuredDetails: (url: string) => invoke<Table>("get_structured_details", { url }),
  nearDuplicates: (url: string) => invoke<Table>("get_near_duplicates", { url }),
  generateSitemap: (dir: string, options: SitemapOptions) => invoke<string[]>("generate_sitemap", { dir, options }),
  urlDetails: (url: string) => invoke<[string, string][]>("get_url_details", { url }),
  exportCsv: (tab: string, filter: string, search: string | undefined, path: string) =>
    invoke<number>("export_csv", { req: { tab, filter, search, path } }),
  openCrawl: (path: string) => invoke<void>("open_crawl", { path }),
  saveCrawlAs: (path: string) => invoke<void>("save_crawl_as", { path }),
  getConfig: () => invoke<CrawlConfig>("get_config"),
  setConfig: (config: CrawlConfig) => invoke<void>("set_config", { config }),
  resetConfig: () => invoke<CrawlConfig>("reset_config"),
};

export const events = {
  onProgress: (cb: (p: Progress) => void): Promise<UnlistenFn> =>
    listen<Progress>("crawl-progress", (e) => cb(e.payload)),
  onAnalysing: (cb: () => void): Promise<UnlistenFn> => listen("crawl-analysing", () => cb()),
  onFinished: (cb: () => void): Promise<UnlistenFn> => listen("crawl-finished", () => cb()),
};
