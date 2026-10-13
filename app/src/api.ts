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

export type { CrawlConfig, CustomExtractor, CustomSearch } from "./config/schema";
import type { CrawlConfig } from "./config/schema";

export type LastMod = "none" | "server_response" | { custom: string };

export interface ImageSitemapOptions {
  max_inlinks: number | null;
  cdn_regex: string;
  include_non_200: boolean;
  include_noindex: boolean;
  max_urls_per_file: number;
}

export interface SitemapOptions {
  include_noindex: boolean;
  include_canonicalised: boolean;
  include_paginated: boolean;
  include_pdfs: boolean;
  include_3xx: boolean;
  include_4xx: boolean;
  include_5xx: boolean;
  last_mod: LastMod;
  priority: boolean;
  priority_by_depth: number[];
  change_freq: "none" | "from_last_modified" | "by_depth";
  change_freq_by_depth: string[];
  max_urls_per_file: number;
  by_segment: boolean;
  include_images: boolean;
  images: ImageSitemapOptions;
  include_hreflang: boolean;
}

export const api = {
  runCrawlAnalysis: () => invoke<void>("run_crawl_analysis"),
  segmentsOverview: () => invoke<Table>("get_segments_overview"),
  setSegmentFilter: (segment: string | null) => invoke<void>("set_segment_filter", { segment }),
  getSegmentFilter: () => invoke<string | null>("get_segment_filter"),
  segmentFields: () => invoke<string[]>("segment_fields"),
  analysisStatus: () => invoke<boolean>("analysis_status"),
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
  report: (name: string) => invoke<Table>("get_report", { name }),
  structuredDetails: (url: string) => invoke<Table>("get_structured_details", { url }),
  nearDuplicates: (url: string) => invoke<Table>("get_near_duplicates", { url }),
  generateSitemap: (dir: string, options: SitemapOptions) => invoke<string[]>("generate_sitemap", { dir, options }),
  urlDetails: (url: string) => invoke<[string, string][]>("get_url_details", { url }),
  pageSource: (url: string) => invoke<string | null>("get_page_source", { url }),
  renderedSource: (url: string) => invoke<string | null>("get_rendered_source", { url }),
  screenshot: (url: string) => invoke<string | null>("get_screenshot", { url }),
  consoleLog: (url: string) => invoke<Table>("get_console_log", { url }),
  blockedResources: (url: string) => invoke<Table>("get_blocked_resources", { url }),
  accessibilityDetails: (url: string) => invoke<Table>("get_accessibility_details", { url }),
  httpHeaders: (url: string) => invoke<Table>("get_http_headers", { url }),
  languageDetails: (url: string) => invoke<Table>("get_language_details", { url }),
  topLanguageErrors: () => invoke<Table>("get_top_language_errors"),
  dismissLanguageError: (kind: string, text: string, action: "ignore" | "dictionary" | "ignore_rule") =>
    invoke<number>("dismiss_language_error", { kind, text, action }),
  spellingLanguages: () => invoke<[string, string][]>("spelling_languages"),
  getDictionaryWords: () => invoke<string[]>("get_dictionary_words"),
  setDictionaryWords: (words: string[]) => invoke<void>("set_dictionary_words", { words }),
  cookies: (url: string) => invoke<Table>("get_cookies", { url }),
  exportPageSource: (dir: string) => invoke<number>("export_page_source", { dir }),
  exportPdfText: (dir: string) => invoke<number>("export_pdf_text", { dir }),
  exportCsv: (tab: string, filter: string, search: string | undefined, path: string) =>
    invoke<number>("export_csv", { req: { tab, filter, search, path } }),
  openCrawl: (path: string) => invoke<void>("open_crawl", { path }),
  saveCrawlAs: (path: string) => invoke<void>("save_crawl_as", { path }),
  getConfig: () => invoke<CrawlConfig>("get_config"),
  setConfig: (config: CrawlConfig) => invoke<void>("set_config", { config }),
  resetConfig: () => invoke<CrawlConfig>("reset_config"),
  builtinConfig: () => invoke<CrawlConfig>("builtin_config"),
  testUrl: (config: CrawlConfig, start: string | null, url: string) => invoke<UrlTest>("test_url", { config, start, url }),
  updateExclude: (patterns: string[]) => invoke<boolean>("update_exclude", { patterns }),
  testRobots: (txt: string, userAgent: string, url: string) => invoke<RobotsTest>("test_robots", { txt, userAgent, url }),
  checkExpression: (kind: string, expression: string) => invoke<string | null>("check_expression", { kind, expression }),
  downloadRobots: (host: string) => invoke<string>("download_robots", { host }),
  validateConfig: (config: CrawlConfig) => invoke<string[]>("validate_config", { config }),
  recentProfiles: () => invoke<ProfileInfo[]>("recent_profiles"),
  loadProfile: (path: string) => invoke<CrawlConfig>("load_profile", { path }),
  saveProfile: (path: string, config: CrawlConfig) => invoke<void>("save_profile", { path, config }),
  saveDefaultProfile: (config: CrawlConfig) => invoke<void>("save_default_profile", { config }),
  clearDefaultProfile: () => invoke<CrawlConfig>("clear_default_profile"),
  hasDefaultProfile: () => invoke<boolean>("has_default_profile"),
};

export interface RobotsTest {
  allowed: boolean;
  line: string | null;
  group: string | null;
}

export interface UrlTest {
  rewritten: string;
  encoded: string;
  internal: boolean | null;
  include_match: string | null;
  included: boolean;
  exclude_match: string | null;
  error: string | null;
}

export interface ProfileInfo {
  path: string;
  name: string;
  exists: boolean;
}

export const events = {
  onProgress: (cb: (p: Progress) => void): Promise<UnlistenFn> =>
    listen<Progress>("crawl-progress", (e) => cb(e.payload)),
  onAnalysing: (cb: () => void): Promise<UnlistenFn> => listen("crawl-analysing", () => cb()),
  onFinished: (cb: () => void): Promise<UnlistenFn> => listen("crawl-finished", () => cb()),
  onAuthRequired: (cb: (r: AuthRequired) => void): Promise<UnlistenFn> => listen<AuthRequired>("auth-required", (e) => cb(e.payload)),
  onApiProgress: (cb: (p: ApiProgress) => void): Promise<UnlistenFn> => listen<ApiProgress>("api-progress", (e) => cb(e.payload)),
  onApiMessage: (cb: (m: string) => void): Promise<UnlistenFn> => listen<string>("api-message", (e) => cb(e.payload)),
  onModelDownload: (cb: (p: { id: string; done_mb: number; total_mb: number }) => void): Promise<UnlistenFn> =>
    listen<{ id: string; done_mb: number; total_mb: number }>("model-download", (e) => cb(e.payload)),
};

// ---------------------------------------------------------------- Authentication and network

export interface Credential {
  url: string;
  username: string;
  password: string;
}

export interface AuthCookie {
  name: string;
  value: string;
  domain: string;
  path: string;
  secure: boolean;
  http_only: boolean;
  host_only: boolean;
}

export interface FormLogin {
  name: string;
  login_url: string;
  cookies: AuthCookie[];
}

export interface AuthConfig {
  prompt: boolean;
  standards: Credential[];
  forms: FormLogin[];
}

export interface ProxyConfig {
  enabled: boolean;
  kind: "http" | "https" | "socks5";
  host: string;
  port: number;
  username: string;
  password: string;
  no_proxy: string[];
}

export interface NetworkSettings {
  proxy: ProxyConfig;
  trusted_certificates: string[];
}

/** A Basic/Digest challenge during a crawl, waiting for the user. */
export interface AuthRequired {
  id: number;
  url: string;
  realm: string;
  scheme: string;
}

export interface GoogleProperties {
  gsc: string[];
  ga4: [string, string][];
  errors: string[];
}

export interface ApiProgress {
  source: string;
  done: number;
  total: number;
}

export interface MultiExport {
  items: string[];
  format: "csv" | "xlsx";
  consolidate: boolean;
  skip_empty: boolean;
}

export interface ExportPreset {
  name: string;
  export: MultiExport;
}

export interface NotificationSettings {
  smtp_host: string;
  smtp_port: number;
  security: "start_tls" | "tls" | "none";
  username: string;
  from: string;
  default_recipients: string[];
}

export const notifyApi = {
  get: () => invoke<NotificationSettings>("get_notification_settings"),
  set: (settings: NotificationSettings) => invoke<void>("set_notification_settings", { settings }),
  test: (to: string[]) => invoke<void>("send_test_email", { to }),
};

export interface ScheduledTask {
  id: string;
  name: string;
  project: string;
  enabled: boolean;
  url: string;
  list_file: string;
  config_profile: string;
  frequency: "once" | "daily" | "weekly" | "monthly";
  start: string;
  weekday: number | null;
  output_folder: string;
  exports: MultiExport;
  create_sitemap: boolean;
  create_images_sitemap: boolean;
  use_apis: boolean;
  email: boolean;
  email_to: string[];
  auto_compare: boolean;
  overwrite: boolean;
}

export interface RunRecord {
  task_id: string;
  task_name: string;
  started: number;
  finished: number;
  ok: boolean;
  message: string;
  crawl: string;
}

export const scheduleApi = {
  tasks: () => invoke<ScheduledTask[]>("get_tasks"),
  history: (limit = 50) => invoke<RunRecord[]>("get_task_history", { limit }),
  save: (task: ScheduledTask) => invoke<ScheduledTask>("save_task", { task }),
  remove: (id: string) => invoke<void>("delete_task", { id }),
  runNow: (id: string) => invoke<void>("run_task_now", { id }),
};

export interface VizNode {
  id: number;
  parent: number | null;
  url: string;
  label: string;
  depth: number;
  status: number;
  indexable: boolean;
  link_score: number | null;
  unique_inlinks: number;
  segment: string | null;
  virtual_node: boolean;
  cluster: number | null;
}

export interface VizGraph {
  nodes: VizNode[];
  edges: [number, number, number][];
  truncated: boolean;
  cluster_labels: string[];
}

export const vizApi = {
  crawlTree: (limit: number) => invoke<VizGraph>("viz_crawl_tree", { limit }),
  directoryTree: (limit: number) => invoke<VizGraph>("viz_directory_tree", { limit }),
  words: (url: string, body: boolean, limit = 120) => invoke<[string, number][]>("viz_words", { url, body, limit }),
  contentClusters: (limit: number, neighbours = 4) => invoke<VizGraph>("viz_content_clusters", { limit, neighbours }),
};

export const exportApi = {
  multiExport: (spec: MultiExport, dir: string) => invoke<string[]>("multi_export", { spec, dir }),
  presets: () => invoke<ExportPreset[]>("get_export_presets"),
  savePresets: (presets: ExportPreset[]) => invoke<void>("save_export_presets", { presets }),
};

export interface LocalModel {
  id: string;
  label: string;
  size_mb: number;
  downloaded: boolean;
}

export const apiAccess = {
  localModels: () => invoke<LocalModel[]>("local_models"),
  downloadLocalModel: (id: string) => invoke<void>("download_local_model", { id }),
  secretStatus: () => invoke<Record<string, boolean>>("api_secret_status"),
  setSecret: (key: string, value: string) => invoke<void>("set_api_secret", { key, value }),
  googleConnect: () => invoke<void>("google_connect"),
  googleDisconnect: () => invoke<void>("google_disconnect"),
  googleProperties: () => invoke<GoogleProperties>("google_properties"),
  testAiPrompt: (prompt: import("./config/schema").AiPrompt, text: string) => invoke<string>("test_ai_prompt", { prompt, text }),
  requestApiData: () => invoke<string[]>("request_api_data"),
};

export const authApi = {
  get: () => invoke<AuthConfig>("get_auth_config"),
  set: (auth: AuthConfig) => invoke<void>("set_auth_config", { auth }),
  answer: (id: number, url: string, credentials: [string, string] | null) => invoke<void>("answer_auth", { id, url, credentials }),
  exportTo: (path: string, password: string) => invoke<void>("export_auth", { path, password }),
  importFrom: (path: string, password: string) => invoke<AuthConfig>("import_auth", { path, password }),
  startFormLogin: (url: string) => invoke<void>("start_form_login", { url }),
  finishFormLogin: (name: string, loginUrl: string) => invoke<FormLogin>("finish_form_login", { name, loginUrl }),
  cancelFormLogin: () => invoke<void>("cancel_form_login"),
};

// ---------------------------------------------------------------- Compare mode

export type CompareState = "Current" | "Previous" | "Added" | "New" | "Removed" | "Missing";
export type ChangeState = "Changed" | "Not Changed";
export type LinkState = "Current" | "Previous" | "Added" | "Removed";

export const COMPARE_STATES: CompareState[] = ["Current", "Previous", "Added", "New", "Removed", "Missing"];

/** Status-bar text for each grid state, as Screaming Frog words it. */
export const COMPARE_STATE_TEXT: Record<CompareState, string> = {
  Current: "URLs in current crawl",
  Previous: "URLs in previous crawl",
  Added: "URLs in previous crawl that moved to filter of current crawl",
  New: "New URLs found in current crawl and filter",
  Removed: "URLs in filter for previous crawl, but not in filter for current crawl",
  Missing: "URLs not found in the current crawl, that previously were in filter",
};

export const CHANGE_ELEMENTS = [
  "Indexability",
  "PageTitles",
  "MetaDescription",
  "H1",
  "WordCount",
  "CrawlDepth",
  "Inlinks",
  "UniqueInlinks",
  "InternalOutlinks",
  "UniqueInternalOutlinks",
  "ExternalOutlinks",
  "UniqueExternalOutlinks",
  "StructuredDataUniqueTypes",
  "Content",
] as const;
export type ChangeElement = (typeof CHANGE_ELEMENTS)[number];

/** Display names for Change Detection elements (match the tab's filter names). */
export const CHANGE_ELEMENT_NAMES: Record<ChangeElement, string> = {
  Indexability: "Indexability",
  PageTitles: "Page Titles",
  MetaDescription: "Meta Description",
  H1: "H1",
  WordCount: "Word Count",
  CrawlDepth: "Crawl Depth",
  Inlinks: "Inlinks",
  UniqueInlinks: "Unique Inlinks",
  InternalOutlinks: "Internal Outlinks",
  UniqueInternalOutlinks: "Unique Internal Outlinks",
  ExternalOutlinks: "External Outlinks",
  UniqueExternalOutlinks: "Unique External Outlinks",
  StructuredDataUniqueTypes: "Structured Data Unique Types",
  Content: "Content",
};

export interface UrlMapping {
  regex: string;
  replace: string;
}

export interface CompareConfig {
  elements: ChangeElement[];
  similarity_change_pct: number;
  ignore_non_indexable: boolean;
  url_mapping: UrlMapping[];
  remove_parameters: string[];
  remove_all_parameters: boolean;
  lowercase: boolean;
}

export interface CompareSummary {
  current_path: string;
  previous_path: string;
  current_start_url: string | null;
  previous_start_url: string | null;
  current_urls: number;
  previous_urls: number;
  in_both: number;
  content_available: boolean;
  config: CompareConfig;
}

export interface CompareOverviewRow {
  tab: string;
  filter: string;
  current: number;
  previous: number;
  change: number;
  change_pct: number | null;
  added: number | null;
  new: number | null;
  removed: number | null;
  missing: number | null;
}

export interface CompareIssueRow {
  tab: string;
  filter: string;
  issue_type: IssueType;
  priority: Priority;
  current: number;
  previous: number;
  change: number;
  change_pct: number | null;
  added: number;
  new: number;
  removed: number;
  missing: number;
  description: string;
}

export interface CrawlInfo {
  path: string;
  name: string;
  start_url: string | null;
  urls: number;
  modified: number;
}

export interface CompareRowsRequest {
  tab: string;
  filter: string;
  state: CompareState | ChangeState;
  search?: string;
  sortColumn?: string;
  sortDesc?: boolean;
  offset: number;
  limit?: number;
}

export const compareApi = {
  listCrawls: () => invoke<CrawlInfo[]>("list_crawls"),
  crawlInfo: (path: string) => invoke<CrawlInfo>("crawl_info_for", { path }),
  getConfig: () => invoke<CompareConfig>("get_compare_config"),
  setConfig: (config: CompareConfig) => invoke<void>("set_compare_config", { config }),
  testMapping: (config: CompareConfig, url: string) => invoke<string>("test_url_mapping", { config, url }),
  screenshots: (url: string) => invoke<[string | null, string | null]>("compare_screenshots", { url }),
  run: (current: string, previous: string) => invoke<CompareSummary>("run_compare", { current, previous }),
  open: (path: string) => invoke<CompareSummary>("open_comparison", { path }),
  close: () => invoke<void>("close_comparison"),
  summary: () => invoke<CompareSummary | null>("compare_summary"),
  tabs: () => invoke<TabDef[]>("compare_tabs"),
  overview: () => invoke<CompareOverviewRow[]>("compare_overview"),
  issues: () => invoke<CompareIssueRow[]>("compare_issues"),
  rows: (req: CompareRowsRequest) => invoke<Table>("compare_rows", { req }),
  exportCsv: (req: CompareRowsRequest, path: string) => invoke<number>("compare_export", { req, path }),
  exportOverview: (path: string) => invoke<number>("compare_export_overview", { path }),
  urlDetails: (url: string) => invoke<Table>("compare_url_details", { url }),
  links: (url: string, outlinks: boolean, linkState: LinkState) => invoke<Table>("compare_links", { url, outlinks, linkState }),
  siteStructure: (depth: number) => invoke<Table>("compare_site_structure", { depth }),
  source: (url: string) => invoke<SourceCompare>("compare_source", { url }),
};

export interface DiffLine {
  tag: "equal" | "delete" | "insert";
  old: number | null;
  new: number | null;
  text: string;
}

export interface SourceCompare {
  current: string | null;
  previous: string | null;
  lines: DiffLine[];
  changed: number;
}

// ---------------------------------------------------------------- File > Crawls, Settings, List mode

export type CrawlStatus = "Crawling" | "Stopped" | "Complete";

export interface CrawlEntry {
  path: string;
  name: string;
  project: string;
  start_url: string | null;
  mode: string;
  urls: number;
  queued: number;
  pct_complete: number;
  status: CrawlStatus;
  created: number;
  modified: number;
  size_bytes: number;
  version: string;
  locked: boolean;
}

export interface AppSettings {
  storage_dir: string | null;
  retention_days: number | null;
  last_mode: string;
  network: NetworkSettings;
  mcp_server: boolean;
  mcp_port: number;
  theme: "dark" | "light" | "system";
  accent: string;
}

export const crawlsApi = {
  list: () => invoke<CrawlEntry[]>("crawls_list"),
  current: () => invoke<string | null>("current_crawl_path"),
  rename: (path: string, name: string) => invoke<void>("crawl_rename", { path, name }),
  setProject: (path: string, project: string) => invoke<void>("crawl_set_project", { path, project }),
  setLocked: (path: string, locked: boolean) => invoke<void>("crawl_set_locked", { path, locked }),
  duplicate: (path: string) => invoke<string>("crawl_duplicate", { path }),
  remove: (path: string) => invoke<void>("crawl_delete", { path }),
  exportTo: (path: string, dest: string) => invoke<void>("crawl_export", { path, dest }),
  continueCrawl: (path: string) => invoke<void>("continue_crawl", { path }),
  getSettings: () => invoke<AppSettings>("get_app_settings"),
  setSettings: (settings: AppSettings) => invoke<void>("set_app_settings", { settings }),
  fetchSitemapUrls: (urls: string[]) => invoke<string[]>("fetch_sitemap_urls", { urls }),
  readUrlFile: (path: string) => invoke<string[]>("read_url_file", { path }),
};

/** Yes/No confirmation: the native dialog in the app, window.confirm in a browser. */
export async function confirmAction(message: string, title = "Screaming Toad"): Promise<boolean> {
  if (!inTauri) return window.confirm(message);
  const { ask } = await import("@tauri-apps/plugin-dialog");
  return ask(message, { title, kind: "warning" });
}
