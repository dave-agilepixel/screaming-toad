# Configuration parity: Screaming Toad vs CONFIG_REFERENCE.md

**Audited:** 12 Oct 2026, against commit `f7ada18` ("First successful build"), plus uncommitted working-tree changes at the time.
**Source of truth:** `CONFIG_REFERENCE.md`, which describes Screaming Frog SEO Spider v24.3.
**Scope:** every control row in Parts 1 to 3 of the reference. Part 4 (workflows) is not counted, but the build order below is designed to unlock those workflows.

> **Compare mode was not audited.** Work on it began while this audit ran (`crates/toad-store/src/compare.rs` is untracked, and `docs/COMPARE_MODE.md` exists). §1.34 is scored against HEAD, where Compare does not exist. Re-score it once that branch lands.

## Progress since the audit

The scores below are as audited. Since then:

- **Compare mode (§1.34):** built. It covers Change Detection (14 elements, similarity %, ignore non-indexable), URL Mapping (Regex Replace and Test), the six grid states, overview, issues, site-structure deltas, URL details and link diffs, exports and the CLI. View Source and Rendered Page compare, and the URL Mapping Options and Remove Parameters tabs, were added in WP2 and WP11. Not built yet: Auto Compare (WP13). See `docs/COMPARE_MODE.md`.
- **WP1, Config foundation:** done.
  - The tree dialog exposes every honoured control: all of Spider > Crawl, Limits, Advanced and Preferences, plus Content Area, Duplicates, robots.txt, URL Rewriting, Speed, User-Agent, HTTP Header, and Custom with case sensitivity, move and duplicate. Store boxes that are not yet honoured are shown disabled. That moves those Partials to Done.
  - Also added: search, "Changed from default" (Ctrl+Shift+C), live validation, and the profiles Load, Load Recent, Save As, Save as Default and Clear Default. The app starts with the saved default.
  - §2.2 rows 1–5, 11 and 13–19 are fixed. Row 17 (the meta-refresh bug) has a regression test. Rows 10 and 12 are recorded as deliberate in HANDOVER.md.
  - `scripts/check_config_schema.py` guards against drift between the UI type and the Rust struct, and runs in CI.
  - Still open from WP1: none. Rows 6–9 move to WP3, as planned.
- **WP2, Storage and crawl management:** done.
  - **Store HTML** (§1.2): a deflate-compressed `html` table. It drives View Source, Bulk Export > All Page Source, and Compare's View Source (side-by-side diff) and Content.
  - **File > Crawls** (§2.4): a browser grouped by project, with name, URL, mode, URLs, % completed, modified, size, version and lock. Actions: open, resume, Select To Compare, rename, duplicate, export, lock, delete (refused when locked) and project.
  - **Resume:** stopped or crashed crawls are rebuilt from the recorded links (`tests/resume.rs`).
  - **§2.1:** Storage Directory and Crawl Retention (Never / 7 / 30 / custom; locked, open and running crawls are exempt). Storage Mode is N/A.
  - **§2.2:** List mode in the app (enter or paste, from a file, Download XML Sitemap), the last mode persisted, and Reports > List Mode Export (upload order and duplicates kept).
  - **§3.1:** `toad crawls`, `toad resume` and `--store-html`.
  - Not done: .xlsx upload, Google Sheets input, crawl IDs (Toad addresses crawls by file path), and "Copy DB ID".
- **WP3, Spider Crawl, Extraction, Advanced and Limits:** done, apart from the items listed below.
  - **§1.1:** every Store and Crawl pair is honoured (`CrawlConfig::stores_link`; Crawl implies Store).
    - New link kinds: Media, SWF, AMP and Mobile Alternate.
    - Uncrawlable Links (Store only) feeds Links > Pages With Uncrawlable Internal Outlinks.
    - Crawl Invalid Links reports `hppts://` as No Response "Malformed URL".
    - Internal and External gain a Media filter.
  - **§1.2:** per-element toggles for Page Title, Meta Description, Meta Keywords, H1, H2, Word Count, Readability, Text to Code Ratio, Hash, Page Size and Forms, plus Response Time, Last Modified, Meta Robots and X-Robots-Tag. Indexability stays always on.
    - HTTP Headers adds a lower tab, the HTTP Header Summary report and an All HTTP Response Headers bulk export.
    - Cookies adds a cookies table, an Internal column, a lower tab, Cookie Summary and All Cookies.
  - **§1.5:**
    - Cookie Storage: Session Only, Persistent or Do Not Store.
    - Ignore Paginated URLs for Duplicates.
    - Always Follow Redirects and Always Follow Canonicals, plus the Canonical Chains report.
    - Respect noindex, Canonical and Next/Prev.
    - Respect HSTS Policy (307 "HSTS Policy").
    - Respect Self Referencing Meta Refresh.
    - Extract Images From IMG SRCSET and `<picture>`.
    - Perform HTML Validation.
  - **§1.3:** Limit Crawl Total Per Subdomain, Limit URLs Per Crawl Depth, and Limit by URL Path.
  - Tests: `tests/extraction.rs`, `tests/advanced.rs`, `tests/limits.rs`, and unit tests for extraction, the store rules, HSTS and cookies.
  - **Still open from WP3:**
    - Store-only URLs are recorded as links but not listed as rows in the tabs.
    - The URL-bar scope switcher.
    - Crawl Fragment Identifiers > URL > Broken Bookmark (needs page `id`s stored).
    - Green Hosting Carbon Calculation.
    - The AMP tab and Mobile > Mobile Alternate Link filters (they belong to the AMP and Mobile tab work).
    - Indexability toggle (kept always on).
- **WP4, URL Rewriting, CDNs, Include and Exclude:** done.
  - **§1.12 URL Rewriting:**
    - Remove all parameters.
    - Regex Replace: an ordered table with `$1` back-references, using Rust regex syntax (noted in the UI).
    - Percent-encoding case: Uppercase by default, or Lowercase or None.
    - A live before/after Test pane.
    - The order is fragment, parameters, regex replace, lowercase, then percent case.
  - **§1.13 CDNs:** listed hosts, or host/path prefixes, count as Internal. There is a Test pane with a start URL.
  - **§1.14 / §1.15 Include and Exclude:**
    - Both apply to every List mode URL, never to the Spider start URL.
    - Exclude applies to robots.txt fetches.
    - Exclude can be edited during a crawl (`CrawlControl::set_exclude`; queued URLs are re-checked before they are requested).
    - Test panes show the encoded, rewritten URL each regex runs against.
  - Tests: `tests/include_exclude.rs`, plus urlutil unit tests for rewriting, CDNs and the test pane.
- **WP5, robots.txt, User-Agent and HTTP Header:** done.
  - **§1.11 robots.txt:**
    - Custom Robots per subdomain (host or host:port), with Override Enabled. Its text is used instead of the live file.
    - An editor with Download robots.txt and Clear, and a gutter with line numbers.
    - A Test URL pane: Allowed or Blocked, the deciding line with its line number, and the group used.
    - Matched lines now carry line numbers ("Line 7: Disallow: /x").
    - "Ignore robots.txt but report status" now records the matching line and still crawls the URL; it stays indexable.
    - The Show Blocked options are disabled under Ignore.
  - **§1.17 User-Agent:** presets (Googlebot Smartphone, Desktop, Image and News, Google-InspectionTool, Bingbot, Chrome, Chrome for Android, Firefox, Safari, Edge, GPTBot, ClaudeBot and PerplexityBot) fill both the request user-agent and the robots user-agent. The AI crawler strings are unverified.
  - **§1.18 HTTP Header:** header-name suggestions and a validity mark (the read-only User-Agent row, Add and Reset to Default already existed).
  - Still deliberate: no Googlebot fallback when selecting a robots group (HANDOVER).
- **WP6, Custom Search, Extraction and Link Positions:** done.
  - **XPath 1.0:** the page is converted from HTML into an XML tree (`xpath.rs`) and queried with sxd-xpath. XPath is the default extractor type, as in Screaming Frog.
    - Supports Element, Inner HTML and Text output, Function Value (`count()`, `string-length()`), attribute nodes (`//a/@href`), the optional Attribute field, and `|` unions.
    - XPath 2.0/3.1 functions are a known gap.
  - **§1.19 Custom Search:**
    - Search In gains Page Text No Anchors and XPath (with an expression field).
    - A multi-line query editor and a validity mark for regex queries.
    - Duplicate, Move Up/Down, Bulk Add (shared settings, optional name prefix), Delete All, and at most 100 searches.
  - **§1.20 Custom Extraction:**
    - XPath, CSSPath and Regex, with a validity mark per row (the `check_expression` command).
    - Function Value, Move, Duplicate, Delete All, at most 100 extractors, and 1,000 values per page in total.
    - The CLI gains `--extract Name=xpath:EXPR`.
  - **§1.21 Custom Link Positions:**
    - An Enable toggle: off leaves Link Path and Link Position empty.
    - An ordered table (Add, Move, Delete, Delete All) and a Test box.
  - Not done: Visual Custom Extraction (needs the WP9 browser), and the Custom Search > Filter X Inlinks bulk export.
  - Default change: new and unspecified extractors are XPath, so existing profiles that relied on the old CSS default need `"kind": "css"`. The CLI's `css:` prefix is explicit.
- **WP7, Content:** done, apart from the items listed below.
  - **§1.7 Content Area:** Mode Exclude (the default), Include, or Both (the previous superset behaviour); Check Alt Text.
  - **§1.6 Preferences:**
    - Resource size limit, with Validation > Resource Over X MB. The Validation tab now covers internal 200 URLs, with its HTML checks marked `is_html`.
    - Response Times in one-second buckets: a report and a right-hand panel.
  - **§1.9 Spelling & Grammar:**
    - Spelling uses Hunspell dictionaries via `spellbook`. They are downloaded from wooorm/dictionaries on first use: 31 languages, but no Finnish.
    - Grammar uses a LanguageTool server the user configures. Nothing leaves the machine unless a remote server is set.
    - Language Auto (each page's `lang`, with your chosen dialect for a bare `en`) or Manual.
    - Grammar Rules (a disabled-rules list), Ignore (per profile) and Dictionary (global, `app_config_dir/dictionary.txt`, not in profiles).
    - Content > Spelling Errors and Grammar Errors filters (I/M), plus count columns.
    - A Spelling & Grammar Details lower tab, a top-100 panel with Ignore All, Add to Dictionary and Ignore rule, which update the open crawl without re-crawling, and Reports > Spelling & Grammar Errors Summary and the bulk export.
    - CLI: `--spelling`, `--grammar-server` and `--language`.
  - Crawl files from before this change gain the new `urls` columns when opened (`schema::migrate`).
  - **Not done:**
    - A full Re-run after changing the language or content area (it needs Store HTML to re-extract text).
    - PDF spelling (WP8).
    - The per-language Grammar Rules checklist; it is a rule-id list instead.
- **WP8, Structured data and PDF:** done, apart from the items listed below.
  - **Structured Data:**
    - Separate JSON-LD, Microdata and RDFa toggles (JSON-LD, Microdata and both validations on by default; deliberate deviation 1).
    - A new RDFa parser.
    - Schema.org Validation against the bundled vocabulary (`scripts/gen_schemaorg.py`): unknown types and properties, and properties not valid for the type or its ancestors.
    - Google Rich Result Feature Validation as a separate toggle, with its own filters and columns.
    - Case-Sensitive, the RDFa URLs filter, and Reports > Google Rich Results Features Summary.
  - **PDF:**
    - Store PDF: text, giving word count, readability, Custom Search (the tab includes PDFs) and spelling, plus Bulk Export > All PDF Content.
    - Extract PDF Properties: Subject, Author, Created, Modified and Page Count columns; Title and Keywords fill Title 1 and Meta Keywords.
    - Extract PDF Links: URI annotations, with Link Position "PDF page N"; they are followed for discovery.
    - Parsing runs under `catch_unwind`.
  - **Fixed along the way:** List mode now audits every uploaded URL fully, whatever its host. Before, only URLs on the first URL's host were parsed.
  - **Not done:** PDF link anchor text (`lopdf` gives the link rectangle, not the text under it), and storing PDF files themselves (Bulk Export > All PDF Documents).
- **WP9, JavaScript rendering:** done, apart from the items listed below.
  - **Batch A (done): renderer and dual-HTML crawl.**
    - Spider > Rendering: Text Only / JavaScript, screenshots, JS error reporting, flatten shadow DOM and iframes, AJAX timeout, window-size presets, W/H, scaling, Mobile, Touch, Rotate, Resize to Content and resize time. Also a Browser Executable path, auto-detected when blank.
    - `render.rs` drives an installed Chrome, Edge or Chromium over CDP (`chromiumoxide`), with one browser per crawl, at most `threads` tabs, and a private profile.
    - Internal HTML 200 pages are rendered and extracted from the rendered HTML. The original HTML's title, description, H1, canonical, directives, word count and link count are kept in `orig_*` columns.
    - Links from both HTML versions are kept, with Link Origin (HTML / Rendered HTML / HTML & Rendered HTML) in the inlinks and outlinks tables.
    - Stored: Chrome console messages, screenshots, Store Rendered HTML, and blocked resources (requests that robots.txt disallows).
    - CO2 (mg) and Carbon Rating use everything the browser transferred, or the page's own transfer size in Text Only mode.
    - **Deviation:** subresources are fetched by the browser itself, not routed through Toad's fetcher. Blocked resources are worked out after the page renders.
  - **Batch B (done): JavaScript tab.** It has all 16 filters, placed after Hreflang, with original-vs-rendered columns (word count change, JS word count %, title, description, H1, meta robots, canonical, unique JS outlinks, JS errors and warnings, blocked resources).
    - **Deviation:** fragments are stripped during normalisation, so "Old AJAX Crawling Scheme URLs" only matches `_escaped_fragment_` (and `#!` where it survives).
  - **Batch C (done): lower window and carbon.**
    - New lower tabs: Rendered Page (screenshot), Chrome Console Log and Blocked Resources.
    - View Source switches between Original HTML and Rendered HTML.
    - The Internal tab gets CO2 (mg) and Carbon Rating columns.
    - Content > High Carbon Rating flags rating F. Its tab placement is [unverified] against SF.
    - Green Hosting Carbon Calculation is still open.
  - **Batch D (done): Accessibility.**
    - Spider > Extraction > Accessibility runs the bundled axe-core 4.10.3 (MPL-2.0; see `THIRD_PARTY_NOTICES.md`) on each rendered page, before flattening.
    - The Accessibility tab has All Violations, filters per guideline (Best Practice, WCAG 2.0 A/AA/AAA, 2.1 A/AA, 2.2 AA) and per impact, and per-guideline count columns.
    - New: an Accessibility Details lower tab, Reports > Accessibility Violations Summary, and Bulk Export > All Accessibility Violations.
    - **Not done:** SF's per-rule filters (around 90). The summary report covers them per rule.
  - **Batch E (done): Custom > Custom JavaScript.**
    - Snippets have a name, an Extraction/Action type, code, a timeout and content types. Actions run before extractions.
    - The `seoSpider` shim provides `data`, `error`, `saveText`, `saveUrls` (crawled, with Link Origin "Custom JavaScript") and `loadScript`.
    - The Custom JavaScript tab has up to three value columns per snippet, an error column per snippet, a filter per snippet, and an Errors filter.
  - **Not done in WP9:**
    - Archive Website (WARC).
    - Routing the browser's subresource requests through Toad's fetcher (`Fetch.enable`), so Exclude and custom headers apply to them.
    - SF's snippet library and tester.
    - Per-rule accessibility filters.
    - Visual Custom Extraction.
    - Green Hosting Carbon Calculation.
    - Old AJAX `_escaped_fragment_` crawling.
- **WP10, Authentication, proxy and certificates:** done, apart from the items listed below.
  - **Where things are kept.** Credentials are held for the session in `toad_core::auth::Network`, outside `CrawlConfig`, so they are never written into `.toad` files or profiles.
    - Export/Import writes them to a `.toadauth` file encrypted with AES-256-GCM, using a PBKDF2-SHA256 key at 600k iterations.
    - The CLI reads that file with `--auth-config`, taking the password from `TOAD_AUTH_PASSWORD`.
  - **Standards-based:**
    - Basic and Digest (MD5, MD5-sess, SHA-256; qop=auth; stale nonces retried), with Host or URL prefix rows.
    - Credentials are sent up front once an origin has challenged.
    - An in-crawl Authentication Required dialog appears once per realm; Skip declines that realm for the crawl.
    - Answered credentials are reused for the rest of the session.
  - **Forms-based:**
    - A visible Chrome/Edge window opens at the login page. On Done, all its cookies are captured.
    - Matching cookies (domain, path, secure) are sent with every request and set in the renderer.
  - **File > Settings:**
    - Proxy: HTTP, HTTPS or SOCKS5, with optional credentials and a no-proxy list. It is used by the fetcher, the renderer (`--proxy-server`, with credentials answered over CDP) and the login window. CLI: `--proxy` and `--no-proxy`.
    - Trusted Certificates: PEM/DER files added to the fetcher's roots and validated on save. CLI: `--trusted-cert`.
  - The UI warns users to exclude logout and admin URLs when crawling logged in.
  - Tests: `tests/network.rs` (Basic, Digest, prompt, forms cookies, authenticating proxy, certificate files), plus unit tests against the RFC 2617 and RFC 7616 vectors, header parsing, matching and the encrypted round trip.
  - **Not done:**
    - localStorage and sessionStorage capture for forms logins (cookies only).
    - NTLM and Negotiate.
    - Trusted certificates in the renderer (Chrome uses the OS store).
    - Proxy credentials and site credentials together on one rendered page (site credentials win).
    - The proxy password is stored in plain text in the app settings file.
- **WP11, Crawl Analysis, Segments and Compare:** done, apart from the items listed below.
  - **Batch A (done): Crawl Analysis.**
    - Crawl Analysis > Configure: Auto-analyse at End of Crawl (on by default, deliberate deviation 2) and the Link Score and Content (near duplicates) areas.
    - Link counts and redirect chains are always worked out when a crawl ends. With Auto-analyse off, the status bar shows "Crawl Analysis Required" and Crawl Analysis > Start runs it.
    - Resuming a crawl marks the analysis stale.
    - Sitemaps, pagination, hreflang and canonical filters are computed live in Toad, so they need no analysis step.
    - **Images (needs JavaScript rendering):**
      - Incorrectly Sized Images: real area more than 1.5× the displayed area at the render's scale factor. This threshold is [unverified] against SF.
      - Background Images: CSS `background-image` URLs, crawled as Image links with `rel=background-image` and left out of the alt-text filters.
      - New Real and Display Dimensions columns.
    - Fixed along the way: Unique JS Outlinks counts hyperlinks only.
  - **Batch B (done): Segments.**
    - Configuration > Segments: ordered segments with a name, colour, AND/OR, and rules.
      - A rule tests any grid column (contains, equals, starts/ends with, regex, greater/less than, and their negations).
      - Or it tests Issue: whether the URL is (or is not) in any tab filter.
      - Rules compile to one SQL `CASE`, so the first match wins. Regex uses a registered `regexp()` SQLite function.
    - Segments apply to a finished crawl without re-crawling, and are saved into its configuration.
    - **Where segments show up:**
      - A coloured Segment column after Address in every tab.
      - A segment selector above the grid that narrows the grid, Overview, Issues and counts.
      - The right-hand Segments tab: URLs, indexable, non-indexable, and URLs with issues, warnings and opportunities.
      - Reports > Segments Overview.
      - XML Sitemap > One sitemap per segment (with `sitemap-other.xml` and an index).
    - Validation covers empty or duplicate names, bad regexes and non-numeric comparisons.
    - **Not done:**
      - The segments bar in the Issues tab.
      - Segment-specific Looker Studio exports (not applicable).
      - Crawl visualisations (deferred).
  - **Batch C (done): Compare.**
    - URL Mapping gains Remove Parameters (named query parameters) and Options (Remove all parameters, Lowercase URLs). The Options contents are [inferred] from URL Rewriting's Options tab.
    - Mapping runs on previous-crawl URLs in URL Rewriting's order: parameters, then Regex Replace, then lowercase. The Test tab uses all of them. CLI: `--remove-param`, `--remove-all-params`, `--lowercase-urls`.
    - Rendered Page compare in the lower window: Auto, Current, Previous, or Compare (both screenshots side by side).
    - **Still open:**
      - Auto Compare (WP13).
      - The `--crawl-comparison <id> <id>` spelling. `toad compare` covers it.

## How to read this

| Status | Meaning |
|---|---|
| **Done** | The control is in `CrawlConfig` (or an equivalent), the crawler or store honours it, it is exposed in the desktop UI, and its default matches the reference (or differs only by one of the three deliberate deviations). |
| **Partial** | Part of the control is there. The Notes column says exactly what is missing. Common reasons: honoured but not in the UI; in the struct but never read; always on with no toggle; wrong default. |
| **Missing** | Nothing is implemented. |
| **N/A** | Does not apply to Toad's architecture. A Toad equivalent is proposed where one makes sense. |

**"No UI"** means the value can only be changed by editing a `.toadconfig` JSON file and passing it to `toad crawl --config`. The desktop app has no way to load a profile (see §1.35).

The desktop config dialog (`app/src/components/ConfigDialog.tsx`) has five sections:
- Spider: max URLs, max depth, threads, URL/s, robots mode, user-agent, include, exclude, and seven toggles.
- XML Sitemaps.
- Duplicates.
- Custom Search.
- Custom Extraction.

Everything else in `CrawlConfig` has no UI. `api.ts:74-100` types only the fields the UI edits.

---

## 1. Summary

### 1.1 Per section

| § | Section | Controls | Done | Partial | Missing | N/A |
|---|---|---:|---:|---:|---:|---:|
| 1.1 | Spider > Crawl | 25 | 6 | 13 | 6 | 0 |
| 1.2 | Spider > Extraction | 30 | 0 | 20 | 10 | 0 |
| 1.3 | Spider > Limits | 11 | 2 | 6 | 3 | 0 |
| 1.4 | Spider > Rendering | 15 | 0 | 1 | 14 | 0 |
| 1.5 | Spider > Advanced | 17 | 0 | 6 | 11 | 0 |
| 1.6 | Spider > Preferences | 18 | 0 | 16 | 2 | 0 |
| 1.7 | Content > Content Area | 5 | 0 | 4 | 1 | 0 |
| 1.8 | Content > Duplicates | 3 | 2 | 1 | 0 | 0 |
| 1.9 | Content > Spelling & Grammar | 6 | 0 | 0 | 6 | 0 |
| 1.10 | Content > Embeddings | 11 | 0 | 0 | 11 | 0 |
| 1.11 | robots.txt | 6 | 0 | 3 | 3 | 0 |
| 1.12 | URL Rewriting | 6 | 0 | 2 | 4 | 0 |
| 1.13 | CDNs | 2 | 0 | 0 | 2 | 0 |
| 1.14 | Include | 2 | 0 | 1 | 1 | 0 |
| 1.15 | Exclude | 2 | 0 | 1 | 1 | 0 |
| 1.16 | Speed | 3 | 3 | 0 | 0 | 0 |
| 1.17 | User-Agent | 3 | 1 | 1 | 1 | 0 |
| 1.18 | HTTP Header | 3 | 0 | 1 | 2 | 0 |
| 1.19 | Custom Search | 10 | 3 | 5 | 2 | 0 |
| 1.20 | Custom Extraction | 8 | 2 | 4 | 2 | 0 |
| 1.21 | Custom Link Positions | 4 | 0 | 1 | 3 | 0 |
| 1.22 | Custom JavaScript | 7 | 0 | 0 | 7 | 0 |
| 1.23 | API Access (general) | 5 | 0 | 0 | 4 | 1 |
| 1.24 | GA4 | 13 | 0 | 0 | 13 | 0 |
| 1.25 | Search Console | 14 | 0 | 0 | 14 | 0 |
| 1.26 | PageSpeed Insights | 13 | 0 | 0 | 13 | 0 |
| 1.27 | Majestic | 6 | 0 | 0 | 6 | 0 |
| 1.28 | Ahrefs | 7 | 0 | 0 | 7 | 0 |
| 1.29 | Moz | 6 | 0 | 0 | 6 | 0 |
| 1.30 | AI providers | 23 | 0 | 0 | 23 | 0 |
| 1.31 | Authentication | 6 | 0 | 0 | 6 | 0 |
| 1.32 | Segments | 5 | 0 | 0 | 5 | 0 |
| 1.33 | Crawl Analysis | 13 | 0 | 9 | 4 | 0 |
| 1.34 | Compare (see note above) | 8 | 0 | 0 | 8 | 0 |
| 1.35 | Profiles | 6 | 1 | 3 | 2 | 0 |
| **Part 1** | **Configuration menu** | **322** | **20** | **98** | **203** | **1** |
| 2.1 | File > Settings | 26 | 0 | 2 | 20 | 4 |
| 2.2 | Mode menu | 7 | 1 | 1 | 5 | 0 |
| 2.3 | File > Scheduling | 24 | 0 | 0 | 24 | 0 |
| 2.4 | File > Crawls | 6 | 0 | 2 | 4 | 0 |
| 2.5 | Sitemap generator | 17 | 6 | 4 | 7 | 0 |
| 2.6 | Visualisations | 32 | 0 | 0 | 32 | 0 |
| 2.7 | Multi Export / export dialog | 7 | 0 | 1 | 6 | 0 |
| **Part 2** | **Application settings and menus** | **119** | **7** | **10** | **98** | **4** |
| 3.1 | CLI flags | 26 | 5 | 8 | 12 | 1 |
| 3.2 | `spider.config` and runtime files | 12 | 0 | 0 | 5 | 7 |
| 3.3 | Data Studio Custom Crawl Overview | 1 | 0 | 0 | 1 | 0 |
| **Part 3** | **Automation surfaces** | **39** | **5** | **8** | **18** | **8** |
| **All** | | **480** | **32** | **116** | **319** | **13** |

### 1.2 Overall parity

The percentages exclude N/A rows, which leaves 467 applicable controls.

| Measure | Part 1 (Config menu) | All parts |
|---|---:|---:|
| Done only | 20 / 321 = **6.2%** | 32 / 467 = **6.9%** |
| Weighted (Partial counts as ½) | **21.5%** | **19.3%** |
| Engine coverage (Done + Partial, i.e. something exists) | 36.8% | 31.7% |

**The headline.** The engine is much further along than the "Done" figure suggests. About 60 controls are honoured by the crawler or the store but have no UI, or are hard-wired on with no toggle. Work package 1 below turns most of them into Done for a small amount of effort.

The large Missing blocks are whole features rather than individual toggles:
- JavaScript rendering
- APIs and AI
- Spelling and grammar
- Compare
- Segments
- Scheduling
- Visualisations
- Authentication

---

## 2. Default deviations

### 2.1 Deliberate (listed in HANDOVER.md). Keep these.

| Control | Reference | Toad | Evidence |
|---|---|---|---|
| Structured data extraction (JSON-LD and Microdata) | ✗ | ✓, via a single combined flag `extract_structured_data` | `config.rs:99-102, 179` |
| Link Score scale | 1–100, min-max (scaling unverified) | Log-scaled, then min-max to 1–100 | `analysis.rs:138-142` |
| Robots group selection | Configured UA, then `Googlebot`, then `*` | Configured UA, then `*`. There is no Googlebot step. | `robots.rs:117-131` |

### 2.2 Accidental. Fix these, or decide and document them.

| # | Control | Reference default | Toad behaviour | Evidence | Suggested action |
|---|---|---|---|---|---|
| 1 | Max URL Length to Crawl | 10,000 | **2,000** | `config.rs:152` | Change to 10,000 (Appendix A of the reference picked 10,000) |
| 2 | Pagination: Store | ✓ | **✗** (and Store is never read anyway) | `config.rs:132` | Change to `true` |
| 3 | Custom Extraction: Data | Extract Inner HTML | **Extract Text** | `config.rs:283`, `ConfigDialog.tsx:28` | Change to `InnerHtml` |
| 4 | Soft 404 phrases | "Page Not Found", "404 Page Can't Be Found" (rest unverified) | `page not found`, `404 error`, `not found`. The bare `not found` will over-match. | `config.rs:385` | Use the two documented phrases |
| 5 | Default HTTP headers | Includes `Accept-Encoding: gzip` | No explicit row. reqwest sends `gzip, br, deflate` because of `.gzip/.brotli/.deflate(true)`. | `config.rs:166-173`, `fetch.rs:44-46` | Add the `Accept-Encoding: gzip` row and let it override reqwest's value |
| 6 | Respect HSTS Policy | ✓ (low confidence) | Not implemented, so it behaves as ✗ | none | Implement it (WP3) |
| 7 | Respect Self Referencing Meta Refresh | ✓ (low confidence) | Not implemented, so it behaves as ✗. A self-refresh is silently ignored. | `crawler.rs:460-465` | Implement it (WP3) |
| 8 | Cookie Storage | Session Only | No cookie store, so it behaves as **Do Not Store** | `fetch.rs:38-48` | Enable the reqwest `cookies` feature with a per-crawl jar (WP3) |
| 9 | HTTP Headers (extraction) | ✗ | Always ✓: every response's headers go into `headers_json`. They are hidden in the UI. | `schema.rs:32`, `lib.rs:321`, `DetailPanel.tsx:59` | Harmless. Add the toggle and the lower tab in WP3. |
| 10 | Auto-analyse at End of Crawl | ✗ | Always ✓: analysis runs after every crawl | `app/src-tauri/src/main.rs:140-147`, `toad-cli/src/main.rs:381` | A beneficial deviation. Add it to HANDOVER as deliberate, or add the toggle in WP11. |
| 11 | Sitemap generator: lastmod / priority / changefreq | ✓ / ✓ / ✓ | ✗ / ✗ / ✗ (None) | `sitemap_gen.rs:47-59`, `SitemapDialog.tsx:5-14` | Change to ✓ (server response, depth-based priority, from Last-Modified) |
| 12 | Default User-Agent strings | `Screaming Frog SEO Spider/24.3`; robots UA `Screaming Frog SEO Spider` | `Screaming Toad SEO Spider/<ver>`; robots UA `Screaming Toad` | `config.rs:164-165` | Branding. Keep it, and record it in HANDOVER as a fourth deliberate deviation. |
| 13 | Robots.txt in List mode | ✗ by default, but the user can switch it on | Forced off in List mode. The `robots_mode` setting is ignored there. | `crawler.rs:223` | Apply ignore as a List-mode *default* (`for_list_mode`) and remove the hard check |
| 14 | URL Rewriting scope | Applies to discovered URLs only, not the start URL or List inputs | Applied to the seeds too | `crawler.rs:103` | Skip `normalize` for seeds, apart from fragment stripping |
| 15 | Exact-duplicates scope | Governed by Duplicates > "Only Check Indexable Pages for Duplicates" | Governed by `ignore_non_indexable_for_issues` | `tabs.rs:148-153, 363` | Use `near_duplicates_indexable_only` for both |
| 16 | Max Page Size | "Not processed" when over the limit | The body is truncated, then the truncated HTML is still parsed | `fetch.rs:93-95`, `crawler.rs:457` | Skip extraction when `truncated` is set, and report it |
| 17 | Crawl > Meta Refresh (Crawl ✗) | Meta-refresh targets are not followed | Still followed. `process()` puts the target into `redirect_url`, and `discover()` follows `redirect_url` without checking `crawl_meta_refresh`. | `crawler.rs:329-336, 460-465` | Gate the follow on the redirect type. **This is a bug.** |
| 18 | Crawl Linked Sitemaps in List mode | "Crawl These Sitemaps" still works; only Auto Discover defaults to off | All sitemap crawling is skipped in List mode | `crawler.rs:110` | Skip only the robots auto-discovery |
| 19 | Pages With High Internal Outlinks | "High" = over N | `>=` N | `tabs.rs:496` | Change to `>` (minor) |

---

## 3. Recommended build order

The packages are ordered by user value and dependency. Sizes: **S** ≈ under 1 day, **M** ≈ 1–3 days, **L** ≈ 1–2 weeks, **XL** ≈ over 2 weeks.

### WP1: Config foundation and quick wins (M). Do this first.

**Why first.** It makes about 60 already-honoured controls Done, fixes the accidental defaults, and puts in place the plumbing every later package needs: a config tree UI, persistence and profiles.

**Covers:**
- §1.35 Profiles:
  - Load…, Load Recent, Save As…, Save Current as Default, Clear Default.
  - Persist the current config in `app_config_dir/default.toadconfig`. Today it lives only in `AppState` memory, at `main.rs:18`.
- Rebuild the config dialog as a tree mirroring the reference menu order: Spider ▸ Crawl / Extraction / Limits / Rendering / Advanced / Preferences, Content ▸ …, and so on. Add the "Search…" box, the "Valid config" status, and a diff-against-default view (Ctrl+Shift+C).
- Expose every field that is honoured but has no UI. This makes them Done:
  - Spider > Crawl: crawl canonicals, pagination, hreflang, meta refresh, iframes, internal links; follow external nofollow; check links outside start folder.
  - Limits: folder depth, query strings, max redirects, URL length, links per URL, page size.
  - Advanced: ignore non-indexable for issues, response timeout, 5xx retries, assume HTML.
  - All 16 Preferences.
  - Content Area lists.
  - Only-check-indexable for duplicates.
  - robots: show internal / external blocked.
  - Robots user-agent.
  - Remove Parameters and Lowercase.
  - HTTP header rows, with Add and Reset.
  - Custom Search case sensitivity.
- Fix the accidental defaults in §2.2 rows 1–5, 11, 13–19. Row 17 is a bug fix.
- Type the full `CrawlConfig` in TypeScript. Consider generating it with `ts-rs` or `specta` so it cannot drift.
- Add "Speed" and "User-Agent" as their own tree nodes (they are currently under Spider).

**Files:**
- `crates/toad-core/src/config.rs`
- `crates/toad-core/src/crawler.rs`
- `crates/toad-store/src/tabs.rs`
- `crates/toad-store/src/sitemap_gen.rs`
- `app/src/components/ConfigDialog.tsx` (split into `components/config/*.tsx`)
- `app/src/api.ts`
- `app/src/components/SitemapDialog.tsx`
- `app/src-tauri/src/main.rs`: new commands `load_profile`, `save_profile`, `save_default`, `clear_default` and `recent_profiles`, plus loading the default profile at startup.

**Depends on:** nothing.

### WP2: Storage, crawl management and Store HTML (L)

**Covers:**
- §2.4 File > Crawls:
  - A crawl browser over `app_data_dir/crawls`, listing Name, URL, Mode, URLs, % Completed, Modified, Size and Version.
  - Projects (as folders), Rename, Duplicate, Export, Lock, Delete, Search, and "Select To Compare".
- Resuming a crashed or stopped crawl. Persist the frontier: a `queue` table, plus `seen` derived from `urls`.
- §2.1:
  - Storage Directory.
  - Crawl Retention (Never / 7 / 30 / Custom), with locked crawls exempt.
  - Storage Mode is N/A: Toad is always SQLite-backed.
- §1.2 Store HTML. Add an `html` table holding a zstd-compressed body per URL. This enables:
  - View Source.
  - Bulk Export > All Page Source / All Page Text.
  - Later: AI Page Text, N-grams and Compare "Content".
  - Store Rendered HTML uses the same table with a `rendered` flag, populated by WP9.
- §2.2:
  - List mode in the UI: Enter Manually, Paste, From a File (.txt/.csv/.xlsx), and Download XML Sitemap (one or several).
  - Persist the last mode used.
  - List-mode Export, keeping upload order and duplicates (needs an `original_url` / `list_order` column).
- §3.1: `--list-crawls`, and `--load-crawl` by DB id.

**Files:**
- `crates/toad-store/src/lib.rs`, `schema.rs`, `reports.rs`
- `crates/toad-core/src/crawler.rs`: frontier persistence, sending the HTML body in `PageRecord`
- `crates/toad-core/src/model.rs`
- `app/src-tauri/src/main.rs`
- New `app/src/components/CrawlsDialog.tsx`, `ListModeDialog.tsx` and a View Source pane in `DetailPanel.tsx`
- `crates/toad-cli/src/main.rs`

**Depends on:** WP1 (UI shell).

**Unblocks:** Compare (WP11), AI and embeddings (WP12), Spelling with PDF (WP7).

### WP3: Spider Crawl / Extraction toggles, Advanced behaviours and Limits (L)

**§1.1 Crawl:**
- Honour every **Store** flag. Today none of `store_*` is read anywhere: links of every kind are always written to `links`. The gate goes in `insert_batch` or in `discover`.
- New link kinds:
  - Media (`<video>`, `<audio>`, `<source>`).
  - SWF (`<embed>`, `<object>`).
  - AMP (`rel=amphtml`).
  - Mobile Alternate (`rel=alternate media=…`).
- Uncrawlable Links (Store only). Detect `<span href>`, `<div href>`, `<a onclick>` and `javascript:`, and add the Links filter and the Link Crawlability column.
- Crawl Invalid Links (`hppts://` and similar → No Response "malformed").
- URL-bar scope switcher (Subdomain / All Subdomains / Subfolder).

**§1.2 Extraction:**
- Individual toggles for the 12 page-detail fields, the URL details, and the directives. All of these are currently always on.
- Cookies: needs the cookie jar. Add a Cookies lower tab and the Internal Cookies column.
- HTTP Headers toggle, plus a lower tab. The data is already stored.

**§1.5 Advanced:**
- Cookie Storage (Session / Persistent / Do Not Store). Session means one jar per worker; Persistent means a shared jar.
- Ignore Paginated URLs for Duplicate Filters. Add `AND u.rel_prev IS NULL` to `dup()` in `tabs.rs:149`.
- Always Follow Redirects and Always Follow Canonicals. Bypass `max_depth` for 3xx and canonical targets, capped by `max_redirects`. Add a Canonical Chains report.
- Respect noindex / Canonical / Next-Prev. Add a `hidden` flag, set at analysis time and excluded from tab bases.
- Respect HSTS Policy. Cache the `Strict-Transport-Security` max-age per host, and synthesise a 307 "HSTS Policy" record.
- Respect Self-Referencing Meta Refresh, giving the indexability reason "Self Referencing Meta Refresh".
- Extract Images From IMG SRCSET (`srcset`, `<picture><source srcset>`).
- Crawl Fragment Identifiers: the URL > Broken Bookmark filter needs the page's `id` attributes stored.
- Perform HTML Validation toggle.
- Green Hosting Carbon Calculation (CO2.js SWD model in Rust; the rating needs WP9).

**§1.3 Limits:**
- Limit Crawl Total Per Subdomain.
- Limit URLs Per Crawl Depth.
- Limit by URL Path (regex → max count table).
- Max Page Size semantics (§2.2 row 16).

**Files:**
- `crates/toad-core/src/config.rs`, `crawler.rs`, `extract.rs`, `fetch.rs`, `model.rs`
- `crates/toad-store/src/lib.rs`, `schema.rs`, `tabs.rs`, `analysis.rs`, `reports.rs`
- Config UI pages from WP1

**Depends on:** WP1.

### WP4: URL Rewriting, CDNs, Include/Exclude (M)

**Covers:**
- §1.12:
  - Remove all parameters.
  - Regex Replace (an ordered table; Rust `regex` with `$1` back-references; note the Java-to-Rust syntax differences in the UI).
  - Percent-encoding case (Uppercase / Lowercase / None).
  - A live before/after test pane.
- §1.13 CDNs list. Treat listed hosts as Internal: extend `Scope::is_internal` with host and host/path prefixes. Add a Test URL → Internal/External pane.
- §1.14/§1.15:
  - Apply Include/Exclude to List-mode seeds, and Exclude to robots.txt fetches. Today seeds bypass `allowed()`, and `robots_for` never checks Exclude.
  - Allow Exclude to be edited mid-crawl (a `RwLock<Vec<Regex>>` in `CrawlControl`).
  - Test tabs showing the encoded URL the regex runs against.

**Files:** `urlutil.rs` (`normalize`, `Scope`), `crawler.rs` (seed path at line 103, `robots_for` at 201, `allowed` at 394), `config.rs`, config UI.

**Depends on:** WP1.

### WP5: robots.txt, User-Agent and HTTP Header (M)

**Covers:**
- §1.11:
  - Custom robots per subdomain, with Override Enabled. Use them before fetching the live file.
  - robots.txt editor with Download / Clear.
  - URL tester with matched-line highlight. `Robots::check` already returns the line.
  - "Ignore but report" should record `blocked_by_robots_line` while still crawling. Today it returns before checking (`crawler.rs:228`).
- §1.17:
  - User-agent presets: SF, Googlebot Desktop/Smartphone/Image/News, Google-InspectionTool, Bingbot, Chrome, Chrome Android, Firefox, Safari, Edge, plus AI crawlers (GPTBot, ClaudeBot, PerplexityBot; mark these unverified). Each preset fills both the request UA and the robots UA.
  - Robots UA field.
- §1.18: header table UI with name suggestions, a validity icon, a read-only User-Agent row, Add, and Reset to Default.

**Files:** `robots.rs`, `crawler.rs`, `fetch.rs`, `config.rs` (a `custom_robots: Vec<{host, txt, enabled}>` field and a UA preset enum or table), config UI.

**Depends on:** WP1.

### WP6: Custom Search, Extraction and Link Positions completion (M)

**Covers:**
- §1.19:
  - Search In: add Page Text No Anchors and XPath.
  - Multi-line query editor, case-sensitive checkbox, Duplicate row, Move Up/Down, Bulk Add dialog, Delete All.
  - Max 100 searches.
  - Default names "Search N".
- §1.20:
  - XPath extraction. Choose between `sxd-xpath` (XPath 1.0 only) and `xee`/`xot` (XPath 3.1). XPath 1.0 covers most real use. Mark 2.0/3.1 as a known gap if you pick 1.0.
  - Function Value data mode.
  - `|`-combined expressions.
  - Validity tick/cross per row.
  - Move, Duplicate, Delete All. Max 100 extractors and 1,000 values in total (today the cap is 100 per extractor, at `custom.rs:8`).
  - Visual Custom Extraction is a separate, later item. It needs an embedded webview showing Source and Rendered panes; reuse the WP9 browser.
- §1.21 Custom Link Positions:
  - Move `LINK_POSITIONS` (`extract.rs:15-22`) into config as an ordered table.
  - Enable toggle. When off, skip `link_path`, which saves memory.
  - Move/Add/Delete buttons.
  - Test tab.

**Files:** `config.rs`, `custom.rs`, `extract.rs`, `ConfigDialog` custom pages, `toad-cli` (`--extract xpath:`).

**Depends on:** WP1.

### WP7: Content: Area, Duplicates, Preferences extras, Spelling & Grammar (L)

**Covers:**
- §1.7:
  - Include/Exclude **Mode** radio. Today both lists apply at once, which is a superset; keep that as an advanced option.
  - Check Alt Text (add `img@alt` to the content text).
  - Tag, class and ID list editors (WP1 adds plain textareas).
- §1.8: Duplicates UI completion (Only Check Indexable).
- §1.6:
  - Resource size limit, plus the Validation > Resource Over X MB filter (needs resource sizes, which `CheckOnly` fetches already record).
  - Server response-time buckets, for the Response Times panel and reports.
- §1.9 Spelling & Grammar:
  - Spell check with Hunspell dictionaries via the `spellbook` crate (pure Rust).
  - Grammar via an optional LanguageTool server (local Java or HTTP), or `nlprule` (English and German only).
  - Language Auto (from `lang`) or Manual; Grammar Rules list; Ignore (per crawl); Dictionary (global, stored in the app config dir, not in the profile).
  - Re-run without re-crawling. Needs WP2 Store HTML, or keeping `content_text`, which is already in `PageData`. Check whether it is persisted.
  - Spelling/Grammar filters, the Details lower tab, and the bulk export.

**Files:** `config.rs`, `extract.rs` (`collect_text`), new `crates/toad-core/src/spelling.rs`, `toad-store/analysis.rs` and `tabs.rs`, UI.

**Depends on:** WP1. Re-run depends on WP2.

### WP8: Structured data and PDF completion (M–L)

**Covers:**
- §1.2:
  - Split `extract_structured_data` into JSON-LD / Microdata / RDFa toggles. Keep JSON-LD and Microdata defaulting to ✓, as deliberate deviation 1.
  - RDFa parser (`vocab`/`typeof`/`property`).
  - Schema.org vocabulary validation. Bundle the schema.org JSON-LD vocabulary and check types and properties exist; today the only check is that an `@context` is present (`structured.rs:85`).
  - Separate the Google Rich Result Feature Validation toggle, splitting Rich Result Validation Errors/Warnings from Schema.org Validation Errors/Warnings.
  - Case-Sensitive toggle.
  - Store PDF: text extraction via `pdf-extract` or `lopdf`, giving word count, readability, and Custom Search on PDFs.
  - Extract PDF Properties (Subject, Author, Created/Modified, Page Count).
  - Extract Link Text from PDF links.
- Reports > Google Rich Results Features (Summary).

**Files:** `structured.rs`, `extract.rs`, `crawler.rs` (a PDF branch in `process`), `config.rs`, `tabs.rs`, `reports.rs`.

**Depends on:** WP1. PDF storage benefits from WP2.

### WP9: JavaScript rendering (XL). The big one.

**Covers:**
- §1.4, all 15 controls:
  - Rendering mode: Text Only / JavaScript. Old AJAX Crawling Scheme is deprecated; support `_escaped_fragment_` only if cheap.
  - Screenshots.
  - JS error reporting (Chrome console capture → JavaScript tab).
  - Flatten Shadow DOM and iframes.
  - Archive Website (hierarchical / WARC).
  - AJAX timeout.
  - Window-size presets, W/H, scaling, Mobile, Touch, Resize to Content, resize time, Rotate.
- §1.2 Store Rendered HTML and Accessibility (inject axe-core; Accessibility tab).
- §1.22 Custom JavaScript, all 7 controls:
  - Snippet table with Extraction/Action types, timeout, content types.
  - A `seoSpider` API shim: `data`, `error`, `saveText`, `saveUrls`, `loadScript`.
  - System and User library, and a tester.
- Dual-HTML crawl: discover links from both raw and rendered HTML, plus the Link Origin column.
- Carbon Rating.
- §2.1 Embedded Browser. Toad equivalent: a "Chrome/Edge executable path" setting with auto-detect and a "Disable JS rendering" switch.

**Implementation:**
- Use `chromiumoxide` over CDP, with a pool of N tabs equal to threads.
- Make rendering a `Renderer` trait so Text Only remains the zero-dependency path.
- A rendered page needs CSS and JS fetched: route the browser's subresource requests through the Toad fetcher (CDP `Fetch.enable`) so robots, exclude and user-agent apply.

**Files:**
- New `crates/toad-core/src/render/` (`mod.rs`, `chrome.rs`, `custom_js.rs`, `axe.rs`)
- `crawler.rs` (`process`), `model.rs`, `config.rs`
- Store schema: rendered HTML, screenshots blob, console log
- `tabs.rs`: JavaScript, Accessibility and Custom JavaScript tabs
- UI: Rendered Page and Chrome Console lower tabs

**Depends on:** WP1 and WP2 (blob storage for rendered HTML and screenshots).

**Unblocks:** forms-based auth (WP10), Visual Custom Extraction (WP6), Images analysis (WP11).

### WP10: Authentication, proxy and network trust (M; forms-based adds L after WP9)

**Covers:**
- §1.31:
  - Standards-based. Answer Basic/Digest challenges with pre-entered rows (URL/host, username, password). reqwest does Basic; use `diqwest` or a hand-rolled Digest. A prompt-during-crawl dialog goes in the desktop app.
  - Forms-based: an embedded Chrome window. Capture cookies, localStorage and sessionStorage, then replay them through the jar and CDP. Needs WP9.
  - Auth profile export/import, AES-256-GCM encrypted. This is the `--auth-config` equivalent.
- §2.1:
  - Proxy: address, port and credentials via `reqwest::Proxy`. Note that reqwest's default features already honour `HTTP(S)_PROXY` environment variables (unverified; check the enabled features).
  - Trusted Certificates: add PEM/DER roots via `ClientBuilder::add_root_certificate`. With the default native-tls on Windows, the OS store is already trusted (unverified).
- §3.1 `--auth-config`.

**Files:** `fetch.rs`, `config.rs`, a new settings struct (app-level, not per crawl), `app/src-tauri/src/main.rs`, UI.

**Depends on:** WP1. Forms-based also depends on WP9.

**Security note.** Apply the reference's warning in the UI: suggest Exclude rules for logout and admin URLs when auth is configured.

### WP11: Crawl Analysis, Segments and Compare (L–XL)

**Covers:**
- §1.33 Crawl Analysis:
  - Configure dialog with per-area checkboxes.
  - Start (manual), plus the UI progress already wired to `crawl-analysing`.
  - Auto-analyse toggle (see §2.2 row 10).
  - Missing areas:
    - Images: Background Images and Incorrectly Sized Images. These need rendering (WP9).
    - AMP: needs the WP3 AMP link kind.
    - Analytics and Search Console orphans: need WP12.
  - "Resuming a paused crawl clears analysis": add an `analysed` meta reset.
- §1.32 Segments:
  - Ordered segments with name, colour, and AND/OR rules over any column or Issue.
  - Compile them to SQL predicates. This fits `tabs.rs` naturally: an issue filter is already a predicate.
  - Segments column in every tab, a right-hand Segments panel, a global segment filter, Reports > Segments Overview, and sitemaps by segment (§2.5).
- §1.34 Compare. Finish the in-progress `compare.rs` against `docs/COMPARE_MODE.md`:
  - Change Detection element checkboxes.
  - Similarity Change % (needs WP2 Store HTML or a stored MinHash; Toad already computes MinHash when near duplicates are on).
  - Ignore non-indexable.
  - URL Mapping regex.
  - Current/Previous swap.
- §3.1 `--crawl-comparison`.

**Files:** `toad-store/src/analysis.rs`, `tabs.rs`, `compare.rs`, new `segments.rs`, `config.rs`, UI.

**Depends on:** WP2 (crawl browser and Store HTML for Compare Content). Some areas depend on WP9 and WP12.

### WP12: API Access and AI/Embeddings (XL)

**Covers:**
- §1.23–1.30 (87 controls) and §1.10 Embeddings (11). Suggested order:
  1. PSI (API key only, simplest). Local Lighthouse later, via the WP9 Chrome.
  2. GSC Search Analytics and URL Inspection (Google OAuth loopback flow, quota counters).
  3. GA4 (same OAuth).
  4. AI providers. OpenAI-compatible first, since that one client covers Azure, DeepSeek, Grok, OpenRouter and LM Studio. Then Gemini, Anthropic and Ollama.
  5. Embeddings, Semantic Similarity and Low Relevance.
  6. Ahrefs, Majestic and Moz.
- Shared infrastructure: a per-API async request queue with RPM/quota throttling, "Request API Data" for a finished crawl, right-click "request for this URL", Auto Connect on Start, and secrets in the OS keychain (`keyring` crate) rather than the profile, which is safer than SF.
- Mode > APIs (§2.2).
- CLI: `--use-*` flags.

**Files:** new crate `crates/toad-apis` (one module per provider), `toad-store` (an `api_values` table, plus Analytics, Search Console, PageSpeed, Link Metrics, AI and Content-semantic tabs), UI.

**Depends on:** WP1. AI Page Text and HTML need WP2. Prompt segment matching needs WP11.

### WP13: Automation and output: Scheduling, notifications, exports, CLI parity, MCP (L)

**Covers:**
- §2.3 Scheduling:
  - Toad equivalent: tasks stored as JSON, run by the OS scheduler (Windows Task Scheduler via `schtasks`, cron or launchd), which invokes `toad crawl` headless.
  - Export Tabs, Bulk Exports and Reports selections; output mode; Auto Compare; History.
- §2.1 Notifications: SMTP via `lettre`. Outlook and Gmail OAuth are optional.
- §2.7:
  - Multi Export with presets shared with the CLI.
  - XLSX via `rust_xlsxwriter`.
  - Consolidate Spreadsheets and Skip Empty Reports.
- §3.1 flags:
  - `--bulk-export` and `--save-report` inside `crawl`.
  - `--export-format`, `--overwrite`, `--timestamped-output`, `--project-name`, `--task-name`.
  - `--create-sitemap` and `--create-images-sitemap` inside `crawl`.
  - `--email-on-complete`.
  - `--help export-tabs` and similar.
  - `--crawl-google-sheet` and the Drive account flag are optional.
- §2.1 MCP Server: Streamable HTTP on port 11435, with tools over the store. The STDIO variant via `toad mcp` is easy, since the store is already a library.
- §3.3 Data Studio summary (deferred; needs Google Sheets).

**Files:** `crates/toad-cli/src/main.rs`, new `crates/toad-sched`, `toad-store/reports.rs`, `app/src-tauri/src/main.rs`, UI.

**Depends on:** WP1. Auto Compare needs WP11. APIs in schedules need WP12.

### Deferred: Visualisations and settings polish

- §2.6 (32 controls). Crawl/directory tree and force-directed graphs: d3-force in the React UI, fed by a store query. 3D and the Content Cluster Diagram come after WP12 embeddings.
- §2.5:
  - Images sitemap (include images, noindex images, relevant images with ≤ N inlinks, CDN regex).
  - Hreflang alternates in sitemaps.
  - By segment (after WP11).
  - Response-code inclusion checkboxes.
  - Per-depth priority/changefreq dropdowns.
- §2.1:
  - Theme: Light/Dark. Today the app follows the OS via `prefers-color-scheme` (`styles.css:20`).
  - Accent colour.
  - UI language.

---

## 4. Section detail

**Column key.** "Ref default" is from CONFIG_REFERENCE.md. Status values: D = Done, P = Partial, M = Missing, N/A. File references are relative to the repo root, and paths under `crates/toad-core/src` are abbreviated to the file name.

### §1.1 Spider > Crawl

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Images (Crawl / Store) | ✓ / ✓ | P | Crawl is honoured (`crawler.rs:382`) and in the UI ("Crawl images"). Store is declared (`config.rs:29`) but **never read**. Only `<img src>` is extracted (`extract.rs:148-155`); there is no srcset (see §1.5). |
| Media | ✗ / ✗ | M | There is no `LinkKind` for `<video>`/`<audio>`/`<source>`. |
| CSS | ✓ / ✓ | P | Crawl is honoured with UI. Store is never read. Only `<link rel=stylesheet>` is extracted. |
| JavaScript | ✓ / ✓ | P | Crawl is honoured with UI. Store is never read. |
| SWF | ✓ / ✓ | M | `<embed>`/`<object>` are not extracted. |
| Internal Hyperlinks | ✓ / ✓ | P | `crawl_internal_links` is honoured (`crawler.rs:377`). No UI. There is no store flag. |
| External Links | ✓ / ✓ | P | Crawl is honoured for every external kind (`crawler.rs:371`), with UI labelled "Check external links". Store is never read. |
| Canonicals | ✓ / ✓ | P | Crawl is honoured (`crawler.rs:385`). No UI. Store is never read. |
| Pagination (Rel Next/Prev) | ✗ / ✓ | P | Crawl is honoured (`crawler.rs:386`). No UI. **The Store default is ✗** (`config.rs:132`), and Store is never read. |
| Hreflang | ✗ / ✓ | P | Crawl is honoured (`crawler.rs:387`). No UI. Store is never read. Sitemap hreflang (`xhtml:link`) is not extracted. |
| AMP | ✗ / ✗ | M | `rel=amphtml` is not extracted, and there is no AMP tab. |
| Meta Refresh | ✓ / ✓ | P | The toggle is checked for the link (`crawler.rs:389`), but the target is followed anyway through `redirect_url` (`crawler.rs:329-336`, `460-465`). **Bug.** No UI. |
| iframes | ✓ / ✓ | P | Crawl is honoured (`crawler.rs:388`). No UI. Store is never read. |
| Mobile Alternate | ✗ / ✗ | M | Not extracted. |
| Uncrawlable Links (Store) | ✗ | M | `resolve()` silently drops `javascript:` (`urlutil.rs:8-22`). `span`/`div` href and onclick are not detected. |
| Check Links Outside of Start Folder | ✓ | P | Honoured (`crawler.rs:355-360`). No UI. |
| Crawl Outside of Start Folder | ✗ | D | `crawler.rs:355`. In the UI. |
| Crawl All Subdomains | ✗ | D | `urlutil.rs:106`. In the UI. A bare root domain auto-includes subdomains, as documented. |
| Follow Internal "nofollow" | ✗ | D | Covers rel nofollow, sponsored and ugc (`extract.rs:271`), and meta and X-Robots nofollow (`crawler.rs:339`). In the UI. |
| Follow External "nofollow" | ✗ | P | Honoured (`crawler.rs:379`). No UI. |
| Crawl Invalid Links | ✗ | M | Malformed hrefs are dropped by `resolve()`. |
| Crawl Linked XML Sitemaps | ✗ | D | `crawler.rs:110, 255-317`. In the UI. Spider mode only; see §2.2 row 18. |
| Auto Discover XML Sitemaps via robots.txt | ✓ (child) | D | `crawler.rs:264`. The UI greys it out until the parent is ticked. |
| Crawl These Sitemaps | ✗ / empty | D | `sitemap_urls`. In the UI. CLI `--sitemap-url`. |
| URL-bar scope switcher | Subdomain | P | Subfolder scope comes implicitly from the start URL path, and All Subdomains from the toggle. There is no switcher in the URL bar (`App.tsx`). |

The link-discovery rules are matched:
- Only `<a href>`/`<area href>` are followed as hyperlinks.
- Breadth-first order.
- HTTPS is the default when the scheme is omitted (`main.rs:60`, `toad-cli main.rs:399`).

Not matched:
- The `_escaped_fragment_` snapshot (deprecated; low priority).

### §1.2 Spider > Extraction

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Page Title | ✓ | P | Always extracted (`extract.rs:50-56`). No toggle. |
| Meta Description | ✓ | P | Always extracted. No toggle. |
| Meta Keywords | ✓ | P | Always extracted. No toggle. The large-site workflow wants it off. |
| H1 | ✓ | P | Always extracted; first 2 plus a count. No toggle. |
| H2 | ✓ | P | Always extracted. No toggle. |
| Indexability | ✓ | P | Always computed (`model.rs:143-165`). No toggle. |
| Word Count | ✓ | P | Always computed over the content area. No toggle. |
| Readability | ✓ | P | Flesch is always computed (`extract.rs:217-223`). English-only heuristic. No toggle. |
| Text to Code Ratio | ✓ | P | Always computed. No toggle. |
| Hash Value | ✓ | P | MD5 always computed on 200 responses (`crawler.rs:453-455`). No toggle. |
| Page Size | ✓ | P | Always recorded. No toggle. |
| Forms | ✓ | P | Always extracted, and feeds Security > Form filters (`tabs.rs:243-244`). No toggle. |
| Accessibility | ✗ | M | Needs JS rendering and axe-core (WP9). |
| Response Time | ✓ | P | Always recorded. No toggle. |
| Last Modified | ✓ | P | Always recorded (`crawler.rs:435`). No toggle. |
| HTTP Headers | ✗ | P | Always stored (`headers_json`) but hidden in the UI (`DetailPanel.tsx:59`). There are no per-header columns, no lower tab, no bulk export and no HTTP Header Summary report. The default effectively differs. |
| Cookies | ✗ | M | No cookie capture. |
| Meta Robots | ✓ | P | Always extracted, including the UA-specific meta name (`extract.rs:67`). No toggle. |
| X-Robots-Tag | ✓ | P | Always extracted (`crawler.rs:436-441`). No toggle. |
| JSON-LD | ✗ | P | Extracted under the combined `extract_structured_data` flag, which defaults to ✓ (a deliberate deviation). No UI. No per-format toggle. |
| Microdata | ✗ | P | As for JSON-LD. |
| RDFa | ✗ | M | Not parsed. |
| Schema.org Validation | ✗ | M | Only checks that an `@context` contains schema.org (`structured.rs:85, 138-143`). There is no vocabulary validation. |
| Google Rich Result Feature Validation | ✗ | P | About 20 Google rich-result types are checked whenever structured data is extracted (`structured.rs:50+`). There is no separate toggle. Rich-result and schema.org errors are not split. There is no Rich Results Features report. |
| Case-Sensitive | ✗ | M | – |
| Store HTML | ✗ | M | Not stored. There is no View Source. (WP2) |
| Store Rendered HTML | ✗ | M | Needs WP9. |
| Store PDF | ✗ | M | PDFs are fetched (if linked) but not parsed. |
| Extract PDF Properties | ✗ | M | – |
| Extract Link Text (PDF) | ✗ (unverified) | M | – |

### §1.3 Spider > Limits

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Limit Crawl Total | ✓ 5,000,000 | D | `max_urls` (`crawler.rs:129`). In the UI. The CLI has `--max-urls`. There is no checkbox: a large number means unlimited. |
| Limit Crawl Total Per Subdomain | ✗ | M | – |
| Limit Crawl Depth | ✗ (List: ✓ 0) | D | `max_depth` (`crawler.rs:324`). In the UI. List mode sets 0 (`config.rs:192-197`). A redirect counts as one level (`crawler.rs:328`). |
| Limit URLs Per Crawl Depth | ✗ (1,000) | M | – |
| Limit Max Folder Depth | ✗ (5) | P | Honoured (`crawler.rs:407`) using `folder_depth`, which matches the reference. No UI. |
| Limit Number of Query Strings | ✗ (5) | P | Honoured (`crawler.rs:410`). No UI. |
| Max Redirects to Follow | 10 | P | Honoured (`crawler.rs:330`). Default 10. No UI. |
| Max URL Length to Crawl | 10,000 | P | Honoured (`crawler.rs:397`). **The default is 2,000** (`config.rs:152`). No UI. Seeds bypass it. |
| Max Links per URL to Crawl | 10,000 | P | Honoured (`extract.rs:183`). It counts every link kind, which is acceptable. No UI. There is no 50,000 cap. |
| Max Page Size (KB) | 50,000 | P | Truncates the download (`fetch.rs:93`), but the truncated HTML is still parsed. The reference says it is "not processed". No UI. |
| Limit by URL Path | empty | M | – |

The internal limits are only partly matched:
- The sitemap split at 49,999 is implemented (`sitemap_gen.rs:57`).
- The 500-hreflang-per-URL cap is not enforced.

### §1.4 Spider > Rendering

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Rendering | Text Only | P | Text Only is the only mode, and it is implicit. There is no dropdown. |
| Enable Rendered Page Screenshots | ✓ | M | WP9 |
| Enable JavaScript Error Reporting | ✗ | M | WP9 |
| Flatten Shadow DOM | ✓ | M | WP9 |
| Flatten iframes | ✓ | M | WP9 |
| Archive Website | Do Not Archive | M | WP9 |
| AJAX Timeout | 5 s | M | WP9 |
| Window Size | Googlebot Mobile 411×731 | M | WP9 |
| Width / Height | 411 / 731 | M | WP9 |
| Scaling Factor | 1.0 | M | WP9 |
| Mobile | follows preset | M | WP9 |
| Touch Enabled | follows preset | M | WP9 |
| Resize to Content | follows preset | M | WP9 |
| Window Resize Time | 1 s | M | WP9 |
| Rotate | ✗ | M | WP9 |

### §1.5 Spider > Advanced

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Cookie Storage | Session Only | M | No cookie jar, so it behaves as Do Not Store (`fetch.rs:38-48`). |
| Ignore Non-Indexable URLs for Issues | ✓ | P | Honoured across on-page, Content, Structured Data Missing and High Crawl Depth filters (`tabs.rs:144-148`). No UI. Its scope does not yet include Sitemaps > Orphan, Analytics or GSC (those filters do not exist). |
| Ignore Paginated URLs for Duplicate Filters | ✓ | M | `dup()` (`tabs.rs:149-153`) has no `rel_prev` exclusion. |
| Always Follow Redirects | ✗ | M | In List mode (depth 0), `discover` returns before following redirects (`crawler.rs:324-336`). |
| Always Follow Canonicals | ✗ | M | – |
| Respect noindex | ✗ | M | – |
| Respect Canonical | ✗ | M | – |
| Respect Next/Prev | ✗ | M | – |
| Respect HSTS Policy | ✓ | M | Effectively ✗. There is a Security > Missing HSTS Header filter, but no 307 synthesis. |
| Respect Self Referencing Meta Refresh | ✓ | M | Self-refresh is ignored (`crawler.rs:461`). There is no Non-Indexable reason. |
| Extract Images From IMG SRCSET | ✗ | M | – |
| Crawl Fragment Identifiers | ✗ | P | Fragments are kept when on (`urlutil.rs:26-28`). There is no `id` check and no URL > Broken Bookmark filter. No UI. |
| Perform HTML Validation | ✓ | P | The Validation tab is always on (`tabs.rs:504-516`). No toggle. It has the head/body checks, but not every "Outside `<head>`" mapping is gated. |
| Green Hosting Carbon Calculation | ✗ (unverified) | M | – |
| Assume Pages are HTML | ✗ | P | Honoured (`crawler.rs:452`). No UI. |
| Response Timeout (secs) | 20 | P | Honoured, as both the connect timeout and the total timeout (`fetch.rs:40-41`). No UI. |
| 5XX Response Retries | 0 | P | Honoured, with a linear back-off (`fetch.rs:56-60`). No UI. |

### §1.6 Spider > Preferences

All values live in `Preferences` (`config.rs:331-391`) and are honoured by `tabs.rs`. **None of them is in the UI**, so every one is Partial until WP1. Filter names embed the configured value, as the reference describes.

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Page Title chars min / max | 30 / 60 | P | `tabs.rs:289-290` |
| Page Title pixels min / max | 200 / 561 | P | `tabs.rs:291-292`. Pixel widths come from `pixel.rs`. |
| Meta Description chars min / max | 70 / 155 | P | `tabs.rs:307-308` |
| Meta Description pixels min / max | 400 / 985 | P | `tabs.rs:309-310` |
| URL max characters | 115 | P | `tabs.rs:277` |
| H1 max characters | 70 | P | `tabs.rs:336` |
| H2 max characters | 70 | P | `tabs.rs:351` |
| Image alt text max | 100 | P | `tabs.rs:388` |
| Max Image Size (KB) | 100 | P | `tabs.rs:385` |
| Low Content (word count) | 200 | P | `tabs.rs:365`. The filter name is "Low Content Pages (under N words)", which differs from SF's name. |
| High Crawl Depth | 3 | P | `tabs.rs:487` (`depth > N`, which matches) |
| High Internal Outlinks | 1,000 | P | `tabs.rs:496` uses `>=` |
| High External Outlinks | 10 | P | `tabs.rs:495` |
| Non-Descriptive Anchor Text | 13 phrases | P | The list matches the reference. Used at `tabs.rs:491` and `reports.rs:137`. |
| Soft 404 error text | "Page Not Found", "404 Page Can't Be Found", … | P | **The default list differs** (`config.rs:385`; see §2.2 row 4). |
| HTML Document size limit | 2 MB | P | `tabs.rs:514` |
| Resource size limit | 2 MB | M | There is no Validation > Resource Over X MB filter. |
| Server response time categories | buckets | M | There is no Response Times panel or bucket report. |

The fixed thresholds are matched:
- Readability bands (`tabs.rs:373-374`).

Not matched:
- Incorrectly Sized Images (needs WP9 and WP11).

### §1.7 Content > Content Area

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Check Alt Text | ✗ | M | – |
| Mode (Include / Exclude) | Exclude | P | There is no mode switch. Include lists take effect when non-empty, and exclude lists always apply (`extract.rs:199-202, 320-347`). That is a superset of SF's behaviour. No UI. |
| Exclude/Include Tags | `nav`, `footer` | P | The default matches (`config.rs:322`). Honoured. No UI. |
| Exclude/Include Classes | empty | P | Honoured. No UI. |
| Exclude/Include IDs | empty | P | Honoured. No UI. |

The content area feeds word count, readability, near duplicates and the Custom Search "Content Area" scope. Spelling, N-grams, AI and embeddings do not exist yet.

### §1.8 Content > Duplicates

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Only Check Indexable Pages for Duplicates | ✓ | P | Honoured for near duplicates (`analysis.rs:38`). Exact duplicates use `ignore_non_indexable_for_issues` instead (`tabs.rs:363`). No UI. |
| Enable Near Duplicates | ✗ | D | `analysis.rs:37`. In the UI. CLI `--near-duplicates`. |
| Near Duplicate Similarity Threshold | 90 | D | In the UI, with a minimum of 50 (`ConfigDialog.tsx:172`). The reference allows ≤100; consider allowing a lower minimum. |

The algorithms match:
- MinHash on content-area text.
- Exact duplicates via the MD5 Hash.

Not yet confirmed:
- That exact duplicates are excluded from the near-duplicate set.
- That scores of 99.5% or more display as 100%.

### §1.9 Content > Spelling & Grammar

All 6 rows are **Missing**: Enable Spell Check, Enable Grammar Check, Language, Grammar Rules, Ignore, Dictionary. See WP7.

### §1.10 Content > Embeddings

All 11 rows are **Missing**: Enable, Embedding Prompt, Semantic Similarity with its threshold and its two options, Embedding Filter Rules, Low Relevance with its threshold and its two options. See WP12.

### §1.11 robots.txt

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Mode | Respect (List: Ignore) | P | All three modes are in the UI. Ignore skips the download. Problems: **List mode forces Ignore** whatever the setting (`crawler.rs:223`). "Ignore but report" fetches the file but never records the matched line (`crawler.rs:228-230`). |
| Show Internal URLs Blocked | ✓ | P | Honoured (`crawler.rs:134-141`). No UI. It is not disabled under Ignore. |
| Show External URLs Blocked | ✗ | P | Honoured. No UI. |
| Custom Robots (per subdomain) | empty | M | – |
| robots.txt editor | – | M | – |
| Test URL + result | – | M | The engine support exists (`Robots::check` returns the matched line). |

Matching-algorithm notes:
- Longest match wins, Allow wins ties, and `*`/`$` work (`robots.rs:90-113`).
- A specific group replaces `*`.
- There is no Googlebot fallback (deliberate).
- A blank robots UA obeys only `*`: correct, because `group_for` skips empty agents.
- **Exclude is not applied to robots.txt fetches** (`crawler.rs:201-219`).
- Non-200 robots responses allow everything. SF treats 5xx differently (unverified). Consider following Google's rule: 5xx means disallow all.
- The "Matched Robots.txt Line" column exists (`tabs.rs:125`), but stores only the line text, not the line number.

### §1.12 URL Rewriting

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Remove all parameters | ✗ | M | – |
| Remove Parameters list | empty | P | Honoured (`urlutil.rs:29-40`). No UI. Applied to seeds too (`crawler.rs:103`). |
| Regex Replace | empty | M | – |
| Lowercase discovered URLs | ✗ | P | Honoured by lowercasing the whole URL (`urlutil.rs:41-45`). No UI. Applied to seeds too. |
| Percent-encoding case | Uppercase | M | The `url` crate preserves input case. |
| Before/after test fields | – | M | – |

### §1.13 CDNs

| Control | Ref default | Status | Notes |
|---|---|---|---|
| CDN list | empty | M | `Scope::is_internal` (`urlutil.rs:115-125`) has no CDN hook. |
| Test URL → Internal/External | – | M | – |

### §1.14 Include

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Include regex list | empty | P | Partial regex on the encoded URL, case-sensitive, applied to internal URLs only and not to the start URL (`crawler.rs:403-405`). In the UI. CLI `--include`. **Not applied to List-mode inputs**, which the reference says it should be. |
| Test tab | – | M | – |

### §1.15 Exclude

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Exclude regex list | empty | P | Honoured for discovered and sitemap URLs (`crawler.rs:400`). In the UI. CLI `--exclude`. **Not applied** to List-mode inputs or robots.txt fetches. It **cannot be edited mid-crawl** (regexes are compiled once in `Crawler::new`). |
| Test tab | – | M | – |

### §1.16 Speed

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Max Threads | 5 | D | `crawler.rs:129`. In the UI. CLI `--threads`. |
| Limit URL/s | ✗ | D | Modelled as `max_urls_per_sec: Option<f64>`; empty means off (`crawler.rs:118-122`). In the UI. CLI `--rate`. |
| Max URL/s | 1.0 when enabled | D | As above. The UI shows the "No limit" placeholder rather than a checkbox plus 1.0, which is acceptable. |

### §1.17 User-Agent

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Preset User Agents | SF | M | – |
| HTTP Request User-Agent | `Screaming Frog SEO Spider/24.3` | D | `fetch.rs:42`. In the UI. CLI `--user-agent`. Toad-branded default (§2.2 row 12). |
| Robots User-Agent | `Screaming Frog SEO Spider` | P | Honoured for robots groups (`crawler.rs:235`) and UA-specific meta robots (`extract.rs:43, 67`). No UI. Toad-branded default. |

### §1.18 HTTP Header

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Header rows | UA (read-only), Accept, Accept-Encoding gzip, Cache-Control no-cache, Pragma no-cache | P | `custom_headers` is honoured (`fetch.rs:32-37`) and skips any User-Agent row. Missing the explicit `Accept-Encoding: gzip` row. No UI. No Accept-Language is sent by default, which is correct. |
| Reset to Default | – | M | – |
| Add | – | M | – |

### §1.19 Custom > Custom Search

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Name | "Search N" | D | In the UI. The UI requires a name but does not auto-number. |
| Match (Contains / Does Not Contain) | Contains | D | `custom.rs:84-99`. In the UI. |
| Mode (Text / Regex) | Text | D | Text is case-insensitive unless Case Sensitive is on (`custom.rs:40-44`). In the UI. |
| Query | empty | P | Single-line input only (`ConfigDialog.tsx:196`). |
| Case Sensitive | ✗ | P | Honoured (`custom.rs:42`). Not in the UI. |
| Search In | HTML | P | Has HTML, Head, Body, Page Text and Content Area. **Missing Page Text No Anchors and XPath.** PDFs are not searchable. |
| Row buttons (duplicate / delete) | – | P | Delete only. |
| Move Up / Down | – | M | – |
| Bulk Add | – | M | – |
| Add / Delete All (max 100) | – | P | Add only. There is no Delete All and no 100 cap. |

### §1.20 Custom > Custom Extraction

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Name | "Extractor N" | D | In the UI. |
| Type | XPath | P | CSSPath and Regex only. **No XPath**, which is the reference default (`config.rs:263-294`). |
| Expression | empty | P | There is no validity tick/cross: errors surface only when the crawl starts (`custom.rs:52-60`). `\|` combinations are not supported. |
| Attribute (Optional) | empty | D | CSS only (`custom.rs:108-109`). In the UI. |
| Data | Extract Inner HTML | P | Element, Inner HTML and Text are supported. There is no Function Value. **The default is Text** (`config.rs:283`). |
| Visual Custom Extraction | – | M | – |
| Move Up / Down | – | M | – |
| Duplicate / delete / Add / Delete All (max 100, 1,000 values) | – | P | Add and delete only. There is a 100-values-per-extractor cap (`custom.rs:8`) instead of 1,000 in total, and no cap on the number of extractors. The UI shows the first 3 values as columns (`tabs.rs:139`). |

Extraction runs only on internal 200 HTML pages (`crawler.rs:457`), as the reference says.

### §1.21 Custom > Custom Link Positions

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Enable | ✓ | M | Always on. There is no toggle to save memory. |
| Position rows | Head, Navigation, Header, Aside, Footer, Content | P | The defaults are hard-coded and match the reference (`extract.rs:15-22`). Rows cannot be configured. |
| Move / Add / Delete buttons | – | M | – |
| Test tab | – | M | – |

### §1.22 Custom > Custom JavaScript

All 7 rows are **Missing**: Snippet list, Type, Timeout, Content Types, Move, Add/Library, Editor/Tester. This needs WP9.

### §1.23–1.30 API Access

Every row is **Missing**, except Google Universal Analytics, which is **N/A** because Google sunset it. Do not build it; accept `--use-google-analytics` as a no-op with a warning, if at all.

| § | Area | Rows | Notes |
|---|---|---:|---|
| 1.23 | General: Auto Connect on Start, Request API Data, right-click request, re-request prompt on GA4/GSC config change, legacy UA | 5 | 4 M, 1 N/A |
| 1.24 | GA4: account, property, stream, quota, date range, metrics, dimensions, filters ×2, match slash, match case, limit max results, crawl new URLs | 13 | M |
| 1.25 | GSC: account, date range, device, country, type, query filter, match slash, match case, limit, crawl new URLs, URL Inspection enable / ignore non-indexable / multiple properties / counters | 14 | M |
| 1.26 | PSI: source, key, max outstanding, auto connect, connect, device, 7 metric groups | 13 | M |
| 1.27 | Majestic | 6 | M |
| 1.28 | Ahrefs | 7 | M |
| 1.29 | Moz | 6 | M |
| 1.30 | AI: 4 account rows, 10 prompt rows, 9 advanced rows | 23 | M |

### §1.31 Authentication

All 6 rows are **Missing**: allow standards-based, standards-based data rows, forms-based Add, forms-based list, profile import/export, set/clear default.

The only workaround today is adding a `Cookie:` row to `custom_headers` in a JSON profile. That is a possibility, not a feature.

### §1.32 Segments

All 5 rows are **Missing**: segment list, name, colour, rule rows, AND/OR/Reset.

### §1.33 Crawl Analysis

Toad runs every analysis area that exists, automatically, after every crawl:
- Link counts, redirect chains, Link Score and near duplicates are computed in `analysis.rs:7-42`.
- Pagination, hreflang, canonicals, links and sitemaps filters are live SQL in `tabs.rs`, so they never need a separate pass.

There is no Configure dialog, no per-area toggle, and no manual Start button in the UI. The CLI has a `toad analyse` subcommand.

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Link Score | ✓ | P | Always on. Log-scaled (deliberate). |
| Response Codes (redirect chain / loop) | ✓ | P | Always on (`analysis.rs:46+`). |
| Content (near dupes, semantic, low relevance) | ✓ | P | Near duplicates only, and only when enabled. Semantic and low relevance are missing. |
| Images (background, incorrectly sized) | ✓ | M | Needs WP9. |
| Canonicals (Unlinked) | ✓ | P | Live SQL (`tabs.rs:407`). No toggle. |
| Pagination (Unlinked, Loop) | ✓ | P | Live SQL (`tabs.rs:426-427`). |
| Hreflang (Unlinked, Missing) | ✓ | P | Live SQL (`tabs.rs:470`, `466`). |
| AMP | ✓ (unverified) | M | There is no AMP support. |
| Links (high depth, follow/nofollow, nofollow only, non-indexable only, non-descriptive) | ✓ | P | All 5 filters exist (`tabs.rs:487-500`). |
| Sitemaps (in, not in, orphan, non-indexable, multiple) | ✓ | P | All 5 exist (`tabs.rs:524-528`). |
| Analytics (orphans) | ✓ | M | Needs GA4. |
| Search Console (orphans) | ✓ | M | Needs GSC. |
| Auto-analyse at End of Crawl | ✗ | P | Always on (`main.rs:140-147`). There is no toggle. The default effectively differs, beneficially. |

### §1.34 Compare

All 8 rows are **Missing** at HEAD: Select All, Change Detection elements, Similarity Change %, Content Area button, Ignore Non-Indexable, URL Mapping regex, URL Mapping Options/Parameters/Test, Current/Previous swap.

**Work in progress** is in `crates/toad-store/src/compare.rs` and `docs/COMPARE_MODE.md`, and was not audited. Compare "Content" needs Store HTML (WP2).

### §1.35 Profiles

| Control | Status | Notes |
|---|---|---|
| Load… | P | Available only through CLI `--config <file.toadconfig>` (`toad-cli main.rs:309-312`). Not available in the app. |
| Load Recent | M | – |
| Save As… | P | `toad config <out>` writes **defaults only** (`toad-cli main.rs:212-216`). The app cannot save the current config. JSON format (`config.rs:199-207`). |
| Save Current Configuration as Default | M | The app's config lives in memory and resets on restart (`app/src-tauri/src/main.rs:18`). |
| Clear Default Configuration | P | The app's "Reset to defaults" button (`ConfigDialog.tsx:257`) resets the in-memory config. There is no persisted default to clear. |
| Crawl file stores its configuration | D | Stored in `meta.config` (`toad-store lib.rs:51`) and restored on Open (`main.rs:307`). The tab layout is not stored. |

---

### §2.1 File > Settings

| Panel > Control | Ref default | Status | Notes |
|---|---|---|---|
| Licence | – | N/A | Toad is unlicensed, with no 500-URL cap. |
| UI > Theme | Light | P | Follows the OS through `prefers-color-scheme` (`styles.css:20`). There is no manual switch. |
| UI > Accent Colour | Green | M | Low priority. |
| Language | OS | M | English only. Low priority. |
| Proxy > Use Proxy Server | ✗ | M | reqwest's default features probably honour `HTTP(S)_PROXY` env vars (unverified). There is no setting. |
| Proxy > Address / Port | empty | M | – |
| Proxy > Credentials | ✗ | M | – |
| Storage Mode | Database | N/A | Each crawl is its own SQLite `.toad` file, so DB mode is inherent. Memory mode is not needed. |
| Storage > Directory | `~/.ScreamingFrogSEOSpider` | P | Fixed at `app_data_dir/crawls` (`main.rs:76`). It cannot be configured. |
| Memory Allocation | 2 GB | N/A | No JVM. Optionally expose a SQLite `cache_size` / `mmap_size` setting instead. |
| Crawl Retention | Never | M | WP2 |
| Crawl Retention > Custom days | 1 | M | WP2 |
| Embedded Browser > Enable | ✓ | N/A | There is no embedded browser yet. **Toad equivalent (WP9):** a Chrome/Edge path with auto-detect and a "JS rendering available" status. |
| Trusted Certificates list | empty | M | The default native-tls on Windows probably already trusts the OS store (unverified). WP10. |
| Trusted Certificates > Discover | – | M | WP10 |
| Notifications > email accounts | – | M | WP13 |
| Notifications > custom SMTP | – | M | WP13 |
| Notifications > Send Test Email | – | M | WP13 |
| Notifications > Email on Crawl Complete | ✗ | M | WP13 |
| MCP > Start | stopped | M | WP13 |
| MCP > Port | 11435 | M | WP13 |
| MCP > Max Response Size | 100000 | M | WP13 |
| MCP > Directory | – | M | WP13 |
| MCP > Node.js runtime | ✗ | M | WP13. Consider skipping it: it is a high-risk surface. |
| MCP > View Tools | – | M | WP13 |
| MCP > Auto-start | ✗ (unverified) | M | WP13 |

### §2.2 Mode menu

| Mode / behaviour | Status | Notes |
|---|---|---|
| Spider | D | – |
| List | P | Available in the CLI (`--list file.txt`, http(s) lines only). The engine supports it (`for_list_mode`). **The desktop UI has no way to start a List crawl:** `api.startList` exists but is never called. There is no CSV/XLSX/XML sitemap/Sheets input. Behaviours that match: depth 0, robots ignored (forced; see §2.2 row 13), URLs without a protocol dropped, de-duplication. |
| SERP | M | `pixel.rs` already computes widths, so this is cheap. |
| Compare | M | WIP (see §1.34). |
| APIs | M | WP12 |
| List-mode Export (original order + duplicates) | M | WP2 |
| Last mode persists across restarts | M | WP2 |

### §2.3 File > Scheduling

All 24 rows are **Missing**:
- The list and its buttons.
- General: name, project, Auto Compare, description, date/interval.
- Spider: headless, crawler mode, seed, crawl config, auth config.
- APIs.
- Exports: output folder, Drive, output mode, format, consolidate, skip empty, spreadsheets, local files, Data Studio, presets.
- Notifications: recipients, attachments.

**Building blocks that already exist in the CLI:**
- Headless crawls (`toad crawl`).
- A crawl config (`--config`).
- An output folder.
- `--export-tabs`.

An OS-scheduler wrapper (WP13) is the Toad equivalent.

### §2.4 File > Crawls

| Control | Status | Notes |
|---|---|---|
| Expand/collapse, Search, Show | M | – |
| Table (Name, URL, Mode, URLs, % Completed, Modified, Size, Version, Locked) | M | Crawls are auto-saved to `app_data_dir/crawls/<host>-<ts>.toad` (`main.rs:76-80`), but there is no browser. |
| Right-click menu | P | Only Open (file dialog) and Export (Save As → `save_copy`) exist. There is no rename, duplicate, project, lock, delete or "copy DB id". |
| Bottom buttons | P | Open only. |
| Footer (counts, used/free) | M | – |
| Resume crashed or stopped crawl | M | The frontier is not persisted. |

### §2.5 Sitemaps > XML / Images Sitemap generator

| Control | Ref default | Status | Notes |
|---|---|---|---|
| Pages > Noindex | ✗ | D | `SitemapDialog.tsx`; CLI `--include-noindex` |
| Pages > Canonicalised | ✗ | D | – |
| Pages > Paginated | ✗ | D | – |
| Pages > PDFs | ✗ | D | – |
| Pages > Response codes (2xx locked, plus others) | 2xx only | M | Hard-wired to 2xx. |
| Last Modified > Include | ✓ | P | Supported. **The default is ✗.** |
| Last Modified > source (server / custom date) | server | D | `LastMod::{ServerResponse, Custom}` |
| Priority > Include | ✓ | P | Supported. **The default is ✗.** |
| Priority > by depth 0–5+ | 1.0…0.5 | P | Values are fixed and match the reference defaults (`sitemap_gen.rs:41`). They cannot be edited. |
| Change Frequency > Include | ✓ | P | Supported. **The default is None.** |
| Change Frequency > mode (Last-Modified / depth) | from Last-Modified | D | Both modes exist. Per-depth values are not editable. |
| Images > Include Images | ✓ | M | There is no image sitemap. |
| Images > Include Noindex Images | ✗ | M | – |
| Images > relevant with ≤ N inlinks | ✓ 10 | M | – |
| Images > CDN regex | empty | M | – |
| Hreflang > Include | ✓ | M | – |
| Segments > Create By Segment | ✓ | M | Needs segments. |

The fixed rules match: the split at 49,999 URLs with an index.

### §2.6 Visualisations

All 32 rows are **Missing**: 11 rows in the 2D tree/force-directed cog panel, 12 in the 3D panel, and 9 in the Content Cluster panel. The visualisations themselves do not exist. See Deferred.

### §2.7 Multi Export and the export dialog

| Control | Status | Notes |
|---|---|---|
| Multi Export | M | – |
| Presets | M | – |
| Output Mode / Overwrite | M | – |
| Skip Empty Reports | M | – |
| Export type (CSV / XLS / XLSX / Sheets) | P | CSV only. |
| Consolidate spreadsheets | M | – |
| Save & Open | M | – |

---

### §3.1 CLI flags

The Toad CLI uses subcommands (`toad crawl|issues|overview|tabs|export|links|analyse|config|sitemap|report`) rather than SF's single binary with flags. That is fine; parity is judged on capability.

| SF flag | Status | Toad equivalent and gap |
|---|---|---|
| `--crawl URL` | D | `toad crawl URL` |
| `--crawl-list` | P | `--list file`. Plain text only; no CSV/XLSX/sitemap. |
| `--crawl-google-sheet` | M | – |
| `--load-crawl` | P | Every subcommand takes a `.toad` path. There are no DB ids (no crawl registry). |
| `--list-crawls` | M | WP2 |
| `--crawl-comparison` | M | WP11 |
| `--project-crawl-comparison` | M | WP13 |
| `--config` | D | `--config file.toadconfig` (JSON) |
| `--auth-config` | M | WP10 |
| `--headless` | D | Inherent: the CLI is always headless. |
| `--save-crawl` | D | Inherent: the crawl is always written to `--out` (default `crawl.toad`). |
| `--output-folder` / `--overwrite` / `--timestamped-output` | P | `--output-folder` exists. It always overwrites. There is no timestamped mode. |
| `--project-name` / `--task-name` | M | – |
| `--export-tabs` | D | `--export-tabs "Tab:Filter,…"` |
| `--bulk-export` | P | Available after the crawl as `toad report <crawl> "<name>"`, not inside `crawl`. |
| `--save-report` | P | As above. |
| `--export-custom-summary` | M | – |
| `--export-format` | P | CSV only. |
| `--google-drive-account` | M | – |
| `--create-sitemap` / `--create-images-sitemap` | P | `toad sitemap <crawl>` after the crawl. There is no images sitemap. |
| `--use-google-analytics-4` | M | WP12 |
| `--use-google-analytics` | N/A | Sunset. |
| `--use-google-search-console` | M | WP12 |
| `--use-pagespeed/-majestic/-mozscape/-ahrefs` | M | WP12 |
| `--email-on-complete` | M | WP13 |
| `--help <export-tabs\|bulk-export\|save-report\|…>` | P | `toad tabs <crawl>` and `toad report <crawl> --list` list the names, but only against an existing crawl. |

Toad-only extras, which are good to keep:
- `--include`, `--exclude`, `--max-urls`, `--max-depth`, `--threads`, `--rate`, `--user-agent`, `--ignore-robots`, `--all-subdomains`, `--sitemaps`, `--sitemap-url`, `--near-duplicates`, `--search`, `--search-regex`, `--extract`.

The reference's rule that "anything without a flag must come from `--config`" is met: the JSON profile carries every field.

### §3.2 `spider.config` and runtime files

| Key | Status | Notes |
|---|---|---|
| `eula.accepted` | N/A | – |
| `storage.mode` | N/A | Always SQLite. |
| `storage.db_dir` | M | Becomes the app settings JSON `crawls_dir` (WP2). |
| `embeddedBrowser.enable` | N/A | Becomes a `chrome_path` / `rendering_enabled` setting (WP9). |
| `updatechecker.enabled` | N/A | There is no updater. Revisit if Tauri's updater is added. |
| `PSI.secretkey` | M | WP12. Store it in the keychain. |
| `ahrefs.authkey` | M | WP12 |
| `majestic.authkey` | M | WP12 |
| `moz.secretkey` | M | WP12 |
| `licence.txt` | N/A | – |
| `-Xmx` | N/A | No JVM. |
| `DISPLAY` / Xvfb | N/A | Headless Chrome via CDP does not need X. |

### §3.3 Data Studio Custom Crawl Overview

**Missing** (1 row: the 352-field summary export). Deferred: it needs Google Sheets output. A local CSV "crawl summary" row-append is a cheap first step, because `overview()` already yields every Tab:Filter count.

---

## 5. Uncertainty and method notes

- **Counting.** Each table row in CONFIG_REFERENCE.md counts as one control. A control that appears in two places (for example Store HTML in §1.2 and in the Compare Content dependency) is counted once, in its home section. §2.6 rows come from the three cog-panel tables. §1.23 is counted as 5 general behaviours.
- **"Always on" is classed as Partial, not Done.** The behaviour matches the default, but the user cannot turn it off. That matters for the large-site workflow (Part 4) and for memory.
- **Store flags.** I grepped for every `store_*` field across `crates/` and `app/`. None is read outside `config.rs`. This is high confidence.
- **The meta-refresh bug (§2.2 row 17)** comes from reading the code: `process()` sets `redirect_url` for meta refresh, and `discover()` follows `redirect_url` before it consults `should_crawl`. It has not been tested at runtime.
- **reqwest proxy and native-tls behaviour** (§2.1) is from the documented defaults of reqwest 0.12 with default features. I have not checked it against the lockfile's enabled features. Treat it as unverified.
- **Reference values marked [unverified]** in CONFIG_REFERENCE.md are carried through as-is. Where Toad matches an unverified default (title px max 561, description px 400/985, alt 100), the match is only as good as the reference.
- **Compare (§1.34)** is scored at HEAD only. The concurrent WIP was not reviewed.
