# Screaming Toad

A desktop SEO crawler modelled on the Screaming Frog SEO Spider. The engine is Rust and the app is a Tauri 2 shell with a React UI.

- [SCREAMING_FROG_SPEC.md](SCREAMING_FROG_SPEC.md): what the reference product does.
- [CONFIG_REFERENCE.md](CONFIG_REFERENCE.md): every configuration control, its default and its options.
- `research/`: the source notes those two documents were built from.

> **Status:** first build, written without a compiler. The machine it was written on blocks compiled programs from running. The TypeScript front end type-checks and builds; the Rust code has **not yet been compiled**. Expect a round of small compile fixes on first build.

## Layout

| Path | What it is |
|---|---|
| `crates/toad-core` | Crawl engine: config, URL handling and scope, robots.txt, fetching, HTML extraction, pixel widths, the crawl loop |
| `crates/toad-store` | SQLite crawl file (`.toad`), tab/filter definitions, crawl analysis (link counts, Link Score, redirect chains), queries, CSV export |
| `crates/toad-cli` | `toad` command-line tool |
| `app/src-tauri` | Desktop app backend: Tauri commands over the engine and store |
| `app/src` | React UI: tabs, filters, virtualised grid, Overview/Issues panel, URL details/inlinks/outlinks, configuration |

### How it fits together
- **Crawling.** The crawler sends one record per URL over a channel, and the store writes them to SQLite in batches.
- **Tabs and filters.** Every tab and filter is a SQL predicate in `crates/toad-store/src/tabs.rs`. The same definition drives the grid, the Overview counts, the Issues panel and the exports. To add a filter, add one line there.
- **Crawl analysis.** After the crawl, analysis fills in inlink and outlink counts, % of total, redirect chains and loops, and Link Score (PageRank, damping 0.85, 10 iterations, scaled 1–100).

## Setting up a machine

**Windows**
1. Install Visual Studio 2022 Build Tools with the "Desktop development with C++" workload.
2. Install Rust with `rustup` from https://rustup.rs.
3. Install Node.js 20 or later.
4. WebView2 is already on Windows 10 and 11.

**macOS:** run `xcode-select --install`, then install rustup and Node.

**Linux:** install rustup and Node, plus the Tauri system packages listed at https://tauri.app/start/prerequisites/.

## Commands

Run the unit tests and the end-to-end test. The end-to-end test starts a local test site, crawls it, and checks that the filters find its known problems.

```bash
cargo test
```

Crawl from the command line:

```bash
cargo run --release -p toad-cli -- crawl https://example.com --max-urls 500 --export-tabs "Internal:All,Response Codes:Internal Client Error (4XX)"
```

Crawl with sitemaps, near duplicates, a custom search and a custom extractor:

```bash
cargo run --release -p toad-cli -- crawl https://example.com --sitemaps --near-duplicates --search "GA4=gtag(" --extract "Price=css:span.price" --extract "SKU=regex:\"sku\":\"([^\"]+)\""
```

Generate an XML sitemap from a crawl:

```bash
cargo run --release -p toad-cli -- sitemap crawl.toad --out-dir sitemaps --lastmod server --priority
```

Keep page source (View Source, Bulk Export > All Page Source, Compare > Content), and continue a crawl that stopped early:

```bash
cargo run --release -p toad-cli -- crawl https://example.com --store-html --max-urls 1000 -o example.toad
```

```bash
cargo run --release -p toad-cli -- resume example.toad --max-urls 5000
```

List the crawls in a folder, with status and % completed:

```bash
cargo run --release -p toad-cli -- crawls .
```

Compare two crawls (the newer first), mapping staging URLs onto live ones:

```bash
cargo run --release -p toad-cli -- compare new.toad old.toad --map "staging\.example\.com=>www.example.com"
```

If Windows Smart App Control blocks freshly built test binaries ("An Application Control policy has blocked this file"), run the tests in WSL with `scripts/test-wsl.ps1`.

Inspect a saved crawl:

```bash
cargo run --release -p toad-cli -- issues crawl.toad
```

```bash
cargo run --release -p toad-cli -- overview crawl.toad
```

```bash
cargo run --release -p toad-cli -- export crawl.toad --tab "Page Titles" --filter Duplicate --out titles.csv
```

```bash
cargo run --release -p toad-cli -- report crawl.toad "Redirect Chains" --out redirect_chains.csv
```

Preview the UI in a normal browser with mock data (no Rust needed):

```bash
cd app && npm install && npm run dev
```

Run the desktop app in development mode:

```bash
cd app && npm install && npm run tauri dev
```

Build the desktop app installer:

```bash
cd app && npm run tauri build
```

The placeholder icons in `app/src-tauri/icons` can be regenerated from a 1024×1024 PNG with `npx tauri icon path/to/icon.png`.

## What's in this first build

**Crawling and configuration**
- Spider mode and List mode.
- robots.txt handling: respect, ignore, or ignore but report. Blocked URLs are reported with the robots.txt line that matched.
- Include and exclude rules (partial regex match).
- Limits: total URLs, depth, folder depth, query parameters, redirects, URL length, links per page, page size.
- Threads and a URLs-per-second limit; user-agent and custom HTTP headers.
- Parameter removal, subdomain and start-folder scope, and nofollow handling.

**Extraction and data**
- Extracted from each page: titles and meta descriptions with character and pixel widths, meta keywords, H1/H2, meta robots and X-Robots-Tag, canonicals (HTML and HTTP header), rel next/prev, hreflang, and meta refresh.
- Links with anchor text, alt text, rel, target, path type, link path and position (Head, Navigation, Header, Aside, Footer, Content).
- Images, CSS, JS and iframes.
- Content-area word count, Flesch readability, text ratio, MD5 hash, security headers, and forms.
- **XML sitemaps:** crawled from robots.txt or a list, including sitemap index files and gzipped sitemaps.
- **Near duplicates:** MinHash on content-area text, using locality-sensitive hashing so pages aren't all compared with each other. The threshold is configurable (90% by default).
- **Custom Search:** text or regex, searching HTML, head, body, page text or the content area. Each search is Contains or Does Not Contain.
- **Custom Extraction:** CSS selectors (returning text, inner HTML, the whole element, or an attribute) or regex.
- **Structured data:** JSON-LD (including @graph and nested items) and Microdata. Parse errors are reported, and Google rich-result required and recommended properties are checked for about 20 types (Product, Offer, Article, BreadcrumbList, FAQPage, LocalBusiness, Recipe, Event, VideoObject, JobPosting and more).

**Tabs and filters**
- 22 tabs: Internal, External, Security, Response Codes, URL, Page Titles, Meta Description, Meta Keywords, H1, H2, Content, Images, Canonicals, Pagination, Directives, Hreflang, Links, Validation, Sitemaps, Structured Data, Custom Search, and Custom Extraction.
- Hreflang checks include missing, inconsistent, non-canonical and noindex return links, unlinked alternates, duplicate codes, and annotations on canonicalised pages.
- Pagination checks cover unlinked URLs, loops and sequence errors. Canonical checks include chains.
- Filter thresholds come from `Preferences`. Filters carry their Issue/Warning/Opportunity type and High/Medium/Low priority.

**Interface and output**
- Issues and Overview panels; inlinks, outlinks and URL details for any URL.
- CSV export; saving and opening crawls; JSON configuration profiles (`toad config default.toadconfig`).
- XML sitemap generation (`toad sitemap`, or the XML Sitemap button). It follows Screaming Frog's rules: indexable 200 pages only by default, optional lastmod, priority and changefreq, and splitting into an index above 49,999 URLs.
- Duplicate Details and Structured Data Details panels.
- **Reports menu:**
  - Redirect Chains, with every hop listed
  - All Redirects and Redirects to Error
  - Canonical Chains and Non-Indexable Canonicals
  - Insecure Content and Orphan Pages
  - SERP Summary
  - Hreflang: All URLs and Missing Return Links
  - Structured Data: issues and a summary
- **Bulk exports:**
  - All Inlinks and All Anchor Text
  - 3XX, 4XX, 5XX, No Response and All Error inlinks
  - Images Missing Alt Text
  - Internal Nofollow and Non-Descriptive Anchor Text links

  These are available from `toad report <crawl> --list` and the app's Reports menu.
- Live crawl progress, with pause, resume and stop.

## Not built yet (next phases)
Taken from the spec's suggested build order:
1. **JavaScript rendering** using headless Chrome or Edge through CDP (chromiumoxide). This also unlocks the JavaScript tab, screenshots and accessibility checks.
2. **Content analysis:** spelling and grammar, XPath extraction, custom JavaScript, and image sitemaps.
3. **AMP and RDFa:** AMP validation and RDFa structured data.
4. **Integrations:** GA4, Search Console, PageSpeed Insights, Ahrefs, Majestic, Moz, and AI providers with embeddings.
5. **Crawl management:** compare mode, scheduling, segments, visualisations, the MCP server, Google Sheets export, and authentication.
