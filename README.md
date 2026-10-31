# Screaming Toad

**A free, open-source desktop SEO crawler for Windows and macOS.**

Screaming Toad crawls a website the way a search engine does, pulls the SEO data out of every page, and flags the problems: broken links, redirect chains, duplicate content, missing titles, canonical and hreflang mistakes, structured data errors, accessibility violations and a lot more. It can also enrich a crawl with PageSpeed, Search Console, GA4, backlink metrics and AI.

It's built in Rust, with a Tauri 2 desktop app and a React interface. Everything runs on your own computer. There's no account, no licence key and no crawl limit apart from the ones you set.

> Screaming Toad is an independent project. It is **not affiliated with, endorsed by or connected to Screaming Frog Ltd** or the Screaming Frog SEO Spider. It was inspired by that tool and follows many of its conventions so SEOs feel at home, but it shares no code with it.

## Why open source?

- **Free to use, for anything.** Dual-licensed MIT or Apache-2.0: use it personally, at an agency or inside a commercial product.
- **Your data stays yours.** Crawls are plain SQLite files on your disk. API keys go into your operating system's credential store and are never written into crawl files or profiles. Nothing is sent anywhere except the sites you crawl and the APIs you switch on.
- **Inspectable.** Every filter is a readable SQL query (`crates/toad-store/src/tabs.rs`), so you can see exactly why a URL was flagged and change the rule if you disagree.
- **Scriptable.** The same engine powers a full command-line tool and an MCP server for AI assistants.

## Download

**Latest release: [Screaming Toad 0.1.0](https://github.com/wrenda-admin/screaming-toad/releases/latest)**

| Platform | Download |
|---|---|
| **Windows** (64-bit) | [ScreamingToad-windows-x64-setup.exe](https://github.com/wrenda-admin/screaming-toad/releases/download/v0.1.0/ScreamingToad-windows-x64-setup.exe) (recommended) or [ScreamingToad-windows-x64.msi](https://github.com/wrenda-admin/screaming-toad/releases/download/v0.1.0/ScreamingToad-windows-x64.msi) (for IT deployment) |
| **macOS** (Apple Silicon and Intel) | [ScreamingToad-macos-universal.dmg](https://github.com/wrenda-admin/screaming-toad/releases/download/v0.1.0/ScreamingToad-macos-universal.dmg) |

Checksums are in [SHA256SUMS.txt](https://github.com/wrenda-admin/screaming-toad/releases/download/v0.1.0/SHA256SUMS.txt). Each installer includes the `toad` command-line tool. Builds come from [GitHub Actions](.github/workflows/ci.yml), which also produces fresh installers for every push.

The builds aren't code-signed yet:
- **Windows:** SmartScreen may warn on first run. Choose *More info > Run anyway*.
- **macOS:** right-click the app and choose *Open* the first time.

To build it yourself, see [Building from source](#building-from-source).

## Features

### Crawling

- **Spider mode** crawls from a start URL, following links. **List mode** crawls exactly the URLs you give it: pasted, from a `.txt`/`.csv` file, or from an XML sitemap (it follows sitemap indexes).
- **Pause, resume and stop.** Stopped or crashed crawls can be continued later, with higher limits if you like.
- **JavaScript rendering** in headless Chrome, Edge or Chromium. It captures rendered and original HTML side by side, screenshots, console errors, resources blocked by robots.txt, and transferred bytes for a CO2 estimate. Device presets include Googlebot Mobile/Desktop, iPhone, Galaxy and iPad.
- **robots.txt:** respect it, ignore it, or ignore it but report what would have been blocked. Custom per-host robots.txt overrides, with a test pane that uses Google's matching rules.
- **Scope and limits:**
  - include/exclude regex rules, subdomains, staying inside the start folder, CDNs treated as internal;
  - limits on total URLs, depth, folder depth, query parameters, redirects, URL length, links per page and page size;
  - per-subdomain, per-depth and per-path caps.
- **URL rewriting:** strip all or named parameters, ordered regex replacements and lowercasing, with a test pane.
- **Speed:** thread count and a URLs-per-second cap.
- **Identity:** user-agent presets (Googlebot variants, Bingbot, GPTBot, ClaudeBot, PerplexityBot, major browsers), a separate robots.txt token, and custom HTTP headers.
- **Authentication:**
  - Basic and Digest HTTP authentication;
  - forms-based login: you sign in once in a browser window and the cookies are reused for the crawl;
  - credentials are held for the session only, with an optional encrypted export.
- **Network:** HTTP, HTTPS and SOCKS5 proxies, and trusted certificates for staging sites.
- **Configuration profiles** saved as `.toadconfig` files. There's a "changed from default" view and search across every setting.

### What's extracted from every page

- **Titles, meta descriptions and meta keywords:** character length, pixel width, duplicates, and tags placed outside `<head>`.
- **H1 and H2:** values, lengths and counts, plus non-sequential heading order.
- **Directives:** meta robots, X-Robots-Tag and meta refresh.
- **Canonicals** (HTML and HTTP header), **rel next/prev** pagination and **hreflang**.
- **Content:** word count, Flesch readability, text ratio, an exact-duplicate hash, and near-duplicate fingerprints. The "content area" can exclude navigation and footers.
- **Links:** 17 link types, including hyperlinks, images, CSS, JS, iframes, canonicals, hreflang and redirects. Each link records:
  - anchor text and alt text;
  - follow/nofollow, rel and target;
  - the link's path in the page, and its position (head, navigation, header, aside, footer or content).
- **Images**, including srcset, CSS background images, and real vs displayed sizes.
- **Structured data:**
  - JSON-LD, Microdata and RDFa, validated against the Schema.org vocabulary;
  - Google rich-result checks for 17 types (Article, Product, Offer, Review, BreadcrumbList, FAQPage, LocalBusiness, Recipe, Event, VideoObject, JobPosting, Course and more).
- **Accessibility:** WCAG checks with the bundled axe-core engine, run on rendered pages.
- **Spelling and grammar:**
  - Hunspell spelling in 31 languages; grammar through your own LanguageTool server;
  - Ignore All, Add to Dictionary and Ignore Rule without re-crawling.
- **PDFs:** text, properties (title, author, dates, page count) and links.
- **Response data:** status codes, redirect type, HTTP version, response time, page size, every response header, and cookies with their security flags.
- **Custom Search:** text or regex, Contains / Does Not Contain, across HTML, head, body, page text, the content area or an XPath scope.
- **Custom Extraction:** XPath 1.0, CSS selectors or regex. It can return text, inner HTML, whole elements, attributes or function values.
- **Custom JavaScript:** your own snippets run on rendered pages. They can extract data, report errors, save text, or queue new URLs for crawling.

### 32 tabs, 450+ filters

Each filter is labelled as an Issue, Warning or Opportunity, with High/Medium/Low priority. Each tab has an Overview count and appears in the Issues panel.

| Group | Tabs |
|---|---|
| **Site** | Internal, External, Security, Response Codes, URL |
| **On-page** | Page Titles, Meta Description, Meta Keywords, H1, H2, Content, Images |
| **Indexing** | Canonicals, Pagination, Directives, Hreflang, Sitemaps |
| **Technical** | JavaScript, Accessibility, Mobile, Links, Validation, Structured Data |
| **Custom** | Custom Search, Custom Extraction, Custom JavaScript |
| **Data sources** | PageSpeed, Search Console, Analytics, Link Metrics, AI |

Some highlights:
- **Security:** mixed content, insecure forms, missing security headers.
- **Response Codes:** redirect chains and loops, blocked by robots.txt.
- **Canonicals:** canonical chains, non-indexable canonicals.
- **Hreflang:** missing return links, inconsistent language codes, non-canonical alternates.
- **Content:** exact and near duplicates, low content, lorem ipsum, semantically similar pages.
- **JavaScript:** content that only appears after rendering.
- **Sitemaps:** orphan URLs and oversized sitemaps.

### Crawl analysis

- **Link Score:** internal PageRank, scaled 1–100.
- **Inlink and outlink counts**, and redirect chains followed to the final destination, with loop detection.
- **Near duplicates** using MinHash with locality-sensitive hashing, so large sites aren't compared page by page.
- **Semantic similarity:** each page's closest match, and low-relevance pages measured against the site's overall topic. This uses embeddings.
- **Orphan pages** found from XML sitemaps, Search Console and GA4.

### Built-in AI embeddings (no API key needed)

Screaming Toad ships with a local embedding engine (`toad-embed`, written in pure Rust with candle). It runs on your CPU, so semantic similarity, low-relevance detection and content clusters work offline and free. Choose from:
- `bge-small-en-v1.5` (the default);
- `all-MiniLM-L6-v2`;
- `multilingual-e5-small`.

Models download once from Hugging Face. You can also use OpenAI, Gemini, Ollama or any OpenAI-compatible service instead.

### Integrations (optional, bring your own keys)

| Service | What you get |
|---|---|
| **PageSpeed Insights** | Core Web Vitals (CrUX field data), Lighthouse scores and metrics, and 27 opportunity audits with estimated savings. Works without a key at a low shared quota. |
| **Google Search Console** | Clicks, impressions, CTR and position per URL (filter by device, country, search type and query). URL Inspection adds index status, Google's chosen canonical and rich results. |
| **Google Analytics 4** | Sessions and other metrics per page, filterable by channel group. |
| **Ahrefs, Majestic, Moz** | Domain Rating, Trust/Citation Flow, Domain/Page Authority, backlinks and referring domains, by URL, subdomain or root domain. |
| **AI prompts** | Run your own prompts over every page with OpenAI, Anthropic Claude, Google Gemini, Ollama or any OpenAI-compatible endpoint (Azure, OpenRouter, LM Studio and others). For example: classify intent, write meta descriptions, summarise content. |
| **Email** | A crawl summary and Auto Compare changes sent over SMTP when a scheduled crawl finishes. |

Google sign-in uses your own OAuth client with read-only scopes. Keys live in Windows Credential Manager, the macOS Keychain or the Linux keyring, and can also be passed as `TOAD_*` environment variables.

### Compare crawls

Compare two crawls of the same site, such as last month vs today or staging vs live.

- **Per filter:** each one shows what's Added, New, Removed or Missing, with change and % change.
- **Change Detection** across 14 elements:
  - titles, meta descriptions, H1, word count, crawl depth;
  - indexability, inlinks and outlinks, structured data types;
  - page content similarity.
- **URL mapping** with regex, for comparing staging URLs against live ones.
- **Side-by-side views:** screenshots, a View Source diff, and site structure.
- **Auto Compare:** scheduled crawls can compare themselves against the previous run automatically.

### Segments

Group URLs into colour-coded segments with rules, for example "Blog" = URL contains `/blog/`. A rule can test any column, or whether the URL has a given issue. Segments apply to finished crawls without re-crawling, and they filter every tab, the Overview and the Issues panel.

### Reports and exports

- **Reports:**
  - Crawl Overview, Redirect Chains, All Redirects, Redirects to Error;
  - Canonical Chains, Non-Indexable Canonicals, Insecure Content, Orphan Pages;
  - Hreflang, Structured Data, Accessibility, Spelling & Grammar and SERP summaries;
  - Response Times, HTTP Headers, Cookies, Segments Overview.
- **Bulk exports:**
  - All Inlinks, All Anchor Text;
  - 3XX, 4XX, 5XX and no-response inlinks;
  - images missing alt text, nofollow and non-descriptive anchor text links;
  - all headers, cookies, accessibility violations and spelling errors;
  - all page source, and all PDF text.
- **Multi Export:** any mix of tabs, reports and bulk exports in one go, as CSV or Excel (one workbook or one sheet each). Save the mix as a preset to reuse in the app, the CLI and schedules.

### XML sitemaps

- **Default contents:** indexable 200 pages, with toggles to add noindex, canonicalised, paginated or PDF pages and redirect or error URLs.
- **Extras:** `lastmod` from the server or a fixed date, priority and change frequency by crawl depth, image entries and hreflang entries.
- **Splitting:** a sitemap per segment, and automatic splitting into a sitemap index for large sites.
- **Images sitemap:** a separate one is available, with CDN support.

### Visualisations

- Crawl tree and directory tree graphs.
- Force-directed crawl and directory diagrams.
- A content cluster diagram, which groups pages by meaning using embeddings.
- Word clouds for inlink anchor text and body text.

Nodes can be coloured by indexability, response code, depth, Link Score, inlinks, segment or cluster.

### Scheduling

Schedule crawls once, daily, weekly or monthly using your operating system's own scheduler: Windows Task Scheduler, macOS launchd or Linux cron. Each scheduled crawl can:
- use a saved profile;
- fetch API data;
- write exports or a Multi Export preset;
- email a summary;
- compare itself with the previous run.

### MCP server for AI assistants

Screaming Toad includes a [Model Context Protocol](https://modelcontextprotocol.io) server. Claude, Cursor and other AI assistants can use it to run crawls and to browse and question your saved crawls. It runs over STDIO (`toad mcp`) or local HTTP (`127.0.0.1:11435`) and offers 14 tools, including:
- starting a crawl (with a configuration profile) and following its progress;
- listing crawls, tabs and issues;
- querying URLs by tab, filter or segment;
- URL details, inlinks and outlinks;
- running reports, exporting tabs and reports to CSV or XLSX, and comparing crawls.

Crawls run in a separate `toad crawl` process: the MCP server's own binary under `toad mcp`, the `toad` next to the desktop app, or the path in `TOAD_BIN`.

### The desktop app

- A fast virtualised data grid that handles large crawls.
- An Overview and Issues side panel.
- A per-URL detail panel with 13 tabs: URL details, inlinks, outlinks, duplicates, structured data, spelling, HTTP headers, cookies, view source (original and rendered), screenshot, console log, accessibility and blocked resources.
- A crawl manager: rename, group into projects, lock, duplicate, continue, compare, auto-delete old crawls.
- Dark, light or system theme, with a custom accent colour.

### Command-line tool

The `toad` CLI ships inside the installers and covers everything the app does, headless:

```bash
toad crawl https://example.com --max-urls 5000 --sitemaps --near-duplicates -o example.toad
toad crawl --list urls.txt --export-tabs "Response Codes:Internal Client Error (4XX)" --export-format xlsx
toad issues example.toad
toad overview example.toad
toad export example.toad --tab "Page Titles" --filter Duplicate --out titles.csv
toad report example.toad "Redirect Chains" --out redirect_chains.csv
toad report example.toad "Issues Overview" --out issues_overview.csv
toad compare new.toad old.toad --map "staging\.example\.com=>www.example.com"
toad sitemap example.toad --out-dir sitemaps --lastmod server
toad resume example.toad --max-urls 50000
toad api example.toad --use-pagespeed
toad mcp
```

The full list of commands is `crawl`, `resume`, `issues`, `overview`, `crawls`, `tabs`, `export`, `links`, `report`, `analyse`, `api`, `compare`, `compare-show`, `compare-export`, `sitemap`, `config`, `mcp`, `tasks`, `run-task` and `google-auth`. Run `toad <command> --help` for each one's options.

## How it works

Each crawl is a single, self-contained **SQLite file** (`.toad`) holding every URL, link, header, structured data item and API value, plus the configuration used. Crawl files are portable, so you can open them in any SQLite tool, and older files are upgraded automatically.

| Path | What it is |
|---|---|
| `crates/toad-core` | Crawl engine: configuration, URL handling, robots.txt, fetching, JavaScript rendering, authentication, HTML/PDF extraction, structured data, spelling |
| `crates/toad-store` | The SQLite crawl file; tabs and filters as SQL; crawl analysis; reports; exports; compare; segments; sitemaps; visualisation data |
| `crates/toad-apis` | PageSpeed, Search Console, GA4, link metrics, AI providers, embeddings, email, the credential store |
| `crates/toad-embed` | Built-in CPU text embeddings (candle) |
| `crates/toad-sched` | Scheduled crawls via the OS scheduler |
| `crates/toad-mcp` | MCP server |
| `crates/toad-cli` | The `toad` command-line tool |
| `app/src-tauri` | Desktop app backend (Tauri 2) |
| `app/src` | Desktop app interface (React and TypeScript) |

**Adding a filter** is usually one line in `crates/toad-store/src/tabs.rs`. The same definition drives the grid, Overview counts, Issues panel and exports.

## Building from source

**Prerequisites**
- Rust stable ([rustup](https://rustup.rs)) and Node.js 20 or later.
- **Windows:** Visual Studio 2022 Build Tools with "Desktop development with C++". WebView2 is already part of Windows 10 and 11.
- **macOS:** `xcode-select --install`.
- **Linux:** the [Tauri system packages](https://tauri.app/start/prerequisites/).

**Commands**

Run the tests. They include an end-to-end crawl of a local test site with known problems:

```bash
cargo test --workspace --exclude screaming-toad
```

Run the desktop app in development mode:

```bash
cd app && npm install && npm run tauri dev
```

Build an installer, which goes into `target/release/bundle/`:

```bash
cd app && npm run tauri build
```

Preview the interface in a normal browser with mock data (no Rust needed):

```bash
cd app && npm install && npm run dev
```

## Contributing

Issues and pull requests are welcome. CI runs the full test suite on Windows, macOS and Linux for every pull request. When fixing a filter, change the code rather than the test's expected counts, unless the expectation itself is wrong.

## Licence

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. Unless you say otherwise, any contribution you submit is dual-licensed the same way.

Bundled third-party components (axe-core under MPL-2.0, Schema.org data under CC BY-SA 3.0, fonts and embedding models) keep their own licences; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).

"Screaming Frog" and "SEO Spider" are trademarks of Screaming Frog Ltd, used here only to describe what this project is inspired by.
