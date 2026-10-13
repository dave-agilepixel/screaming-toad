# Screaming Toad: handover brief

**For:** the Claude Code session that picks this project up on the build machine.
**Written:** 12 Oct 2026, at the end of the first build session on another PC.

## The goal
Build **Screaming Toad**, a desktop SEO crawler modelled on the Screaming Frog SEO Spider (v24.3). These two documents are the source of truth:
- `SCREAMING_FROG_SPEC.md`: what the product does (tabs, filters, reports, modes, integrations).
- `CONFIG_REFERENCE.md`: every configuration control, with its default and options. Appendix A lists defaults where the sources conflict.

The `research/` folder holds the raw notes those two were built from.

## Where it stands
The code is written but **has never been compiled**. The previous PC blocks every newly built program (it's managed by Defender for Endpoint), so neither `cargo build` nor its build scripts could run.

What has been verified so far:
- The React UI type-checks (`tsc`) and builds (`vite`).
- The UI was clicked through in a browser using `app/src/mock.ts`.

**Stack:** a Rust workspace (`toad-core` crawl engine, `toad-store` SQLite store and filters, `toad-apis` PSI/Google/AI/link-metric providers, notifications and the shared credential store, `toad-sched` scheduled tasks, `toad-mcp` MCP server, `toad-cli` the `toad` binary) plus a Tauri 2 desktop shell (`app/src-tauri`) with a React and TypeScript UI (`app/src`). Each crawl is one SQLite file (`.toad`).

**What's built:** see the "What's in this first build" section of README.md. In short:
- Spider and List modes
- robots.txt handling, include/exclude rules, limits, speed, URL rewriting
- Extraction of on-page elements, links, hreflang, pagination and canonicals
- XML sitemap crawling and generation
- Near duplicates (MinHash)
- Custom Search and Custom Extraction
- Structured data (JSON-LD and Microdata, with Google rich-result checks)
- 22 tabs of filters, carrying the issue types and priorities from the spec
- Crawl analysis: link counts, Link Score, redirect chains
- 20+ reports and bulk exports
- CLI, desktop app and an end-to-end test

## Your first job: get it compiling
1. **Prerequisites.** Install Rust stable with rustup. On Windows, also install VS 2022 Build Tools with the "Desktop development with C++" workload. Install Node 20 or later.
2. **Build the UI first.** Run `cd app && npm install && npm run build`. Tauri needs `app/dist` to exist.
3. **Compile everything.** Run `cargo check --workspace --all-targets` and fix the errors. Expect a batch: the Rust was written blind. Areas most likely to need fixes:
   - **scraper:** `ElementRef` methods and ancestor/descendant traversal. See `extract.rs`, `structured.rs` and `custom.rs`.
   - **rusqlite:** parameter types in `params!`, `Value`, and `prepare_cached` while a transaction is open.
   - **Tauri 2:** `Emitter` and `Manager` usage, State in async commands, and the capabilities file.
   - **Borrow-checker conflicts** in `toad-store/src/lib.rs` (`insert_batch`) and in the crawler's sitemap discovery.
   - **Version drift:** dependency versions in the root `Cargo.toml` (scraper 0.20, rusqlite 0.32, reqwest 0.12). Bump them if newer ones are required.
4. **Run the tests.** Run `cargo test --workspace --exclude screaming-toad`. The end-to-end test (`crates/toad-store/tests/end_to_end.rs`) serves a small site with known problems and asserts every filter and report. **Fix the code, not the expectations**, unless an expectation is clearly wrong against the spec.
5. **Run the desktop app.** Run `cd app && npm run tauri dev`, then crawl a real site and compare a few counts against expectations.
6. **Commit after each green step.** A GitHub Actions workflow is ready in `.github/workflows/ci.yml` if the repo ends up on GitHub.

## How the code is organised
- **Filters are SQL.** Every tab and filter is a predicate in `crates/toad-store/src/tabs.rs`. The same definition drives the grid, the Overview, the Issues panel and the exports. A new filter is usually one `f(...)` line.
- **Adding a new extracted field:**
  1. Add it to `PageData` (`toad-core/src/model.rs`).
  2. Fill it in `extract.rs`.
  3. Add a column in `toad-store/src/schema.rs`.
  4. Add the value in `url_values()` (`toad-store/src/lib.rs`).
  5. Add a column entry in `tabs.rs` `COLUMNS`.
- **Things computed over the whole crawl** go in `toad-store/src/analysis.rs`.
- **Reports and bulk exports** are named queries in `toad-store/src/reports.rs`.
- **The crawler** sends one `CrawlEvent::Page` per URL and `CrawlEvent::Sitemap` for each sitemap. The CLI and app write these to the store in batches.
- **Defaults** follow CONFIG_REFERENCE.md, with these deliberate deviations:
  - Structured data extraction is on by default.
  - Link Score uses a log scale.
  - Robots matching uses the configured user-agent token, falling back to `*`.
  - The default user-agent is branded "Screaming Toad SEO Spider" (robots token "Screaming Toad").
  - Crawl Analysis > Configure > Auto-analyse at End of Crawl is on by default (Screaming Frog has it off). Link counts and redirect chains are always worked out at the end of a crawl.
  - API data: Search Console and GA4 are fetched before the crawl (so their orphan URLs can be crawled); per-URL APIs (PSI, URL Inspection, AI, embeddings, link metrics) run when the crawl ends, not while it runs. Google sign-in uses the user's own OAuth client. Keys live in the OS credential store, never in profiles.

## What to build next (once it's green)
In this order (spec §9–10):
1. **Compare mode.** Diff two `.toad` files per filter (Added / New / Removed / Missing), with change detection and URL mapping.
2. **Segments.**
3. **JavaScript rendering.** Use headless Chrome or Edge over CDP (chromiumoxide). This unlocks the JavaScript tab, screenshots and accessibility (axe-core).
4. **Remaining extraction:** XPath extraction, spelling and grammar, AMP, RDFa.
5. **Integrations:** GA4, Search Console (including URL Inspection), PageSpeed, Ahrefs, Majestic and Moz, then AI prompts and embeddings.
6. **Crawl management:** scheduling, the MCP server, Google Sheets export, authentication.

## Prompt to start the session
> Read HANDOVER.md, then README.md, SCREAMING_FROG_SPEC.md and CONFIG_REFERENCE.md. Install anything missing from the prerequisites, get `cargo check --workspace --all-targets` passing, then make `cargo test` pass by fixing the code. Commit after each step. Then run the desktop app and crawl a small site. When everything is green, start on Compare mode.

(Note: the screaming-toad.zip in Drive was made before this brief. Save this file into the unzipped folder as HANDOVER.md.)

## Building installers

`cd app && npm run tauri build` (or `-- --bundles nsis` for just the Windows installer). The build first runs `npm run sidecar`, which compiles the `toad` CLI in release mode and copies it to `app/src-tauri/binaries/toad-<target-triple>.exe`. Tauri bundles it next to the app executable as `toad.exe`; File > Scheduling registers tasks that run it. `tauri dev` does the same with a debug build (`npm run sidecar:dev`). Output: `target/release/bundle/`.
