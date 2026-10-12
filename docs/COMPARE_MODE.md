# Compare mode: behaviour reference

Research notes on Screaming Frog's Compare mode (v15–v24), gathered from the user guide, the
"How To Compare Crawls" tutorial and its screenshots, and the release notes. This is the spec
Toad's Compare mode is built against. Items marked [inferred] are not stated by the source.

Sources:
- [UG-cfg] user-guide/configuration (Mode > Compare)
- [UG-tabs] user-guide/tabs (Change Detection)
- [UG-gen] user-guide/general (CLI)
- [TUT] tutorials/how-to-compare-crawls
- [v15]–[v24] the blog release posts (blog/seo-spider-NN)

## 1. Workflow
- **Toolbar:** `Current: [Select Crawl]` · `Previous: [Select Crawl]` · **Compare** · **Clear** · swap ⇄ (enabled once both are chosen) · cog (Compare config).
  - A chosen crawl's button shows its name and date.
- **Status bar:** `Compare Mode: Idle`.
- **File > Crawls:** "Select To Compare" is enabled only when exactly two crawls are selected.
- **Default order:** the later crawl is Current and the earlier is Previous. The CLI decides by crawl date.
- **Saved comparison:** each comparison is auto-saved as a small "Comparison" entry with mode "Compare". It can be renamed.
  - Opening it does not re-run the analysis.
  - Deleting either source crawl breaks it.
- The compare config is persisted (v23 fixed "comparison forgetting config").

## 2. Overview panel
- **Columns:** `Current URLs | Previous URLs | Change | Change % | Added | New | Removed | Missing`.
- **Summary rows** (Total URLs etc.) show only Current, Previous, Change and Change %.
- **Change** = Current − Previous, shown with ▲/▼. **Change %** = Change / Previous.
- **States:**
  - **Added:** "URLs in previous crawl that moved to filter of current crawl." The URL is in both crawls.
  - **New:** "New URLs not in the previous crawl, that are in current crawl and filter."
  - **Removed:** "URLs in filter for previous crawl, but not in filter for current crawl." The URL is in both crawls.
  - **Missing:** "URLs not found in the current crawl, that previously were in filter."
- **Invariant:** Current = Previous + Added + New − Removed − Missing.
- Clicking a count opens that filter in the grid with that state selected.
- **Colours depend on the metric:**
  - Overview: an increase is green.
  - Issues: a decrease is green, an increase red.
  - Crawl depth: +1 is red.
  - Indexability changes are red.
- **Lower chart:** grouped bars of Current vs Previous.
- **Issues panel:**
  - Columns: `Issue Name | Issue Type | Issue Priority | Current | Previous | Change | Change % | Added | New | Removed | Missing`.
  - Header chips like `Warnings: 22 (-1 ▼)`.

## 3. Main grid
- **Filter bar:** `[Filter ▾] [State ▾]`, where State is one of **Current, Previous, Added, New, Removed, Missing**.
- **Status text by state:**
  - Current: "Showing: URLs in current crawl"
  - Previous: "Showing: URLs in previous crawl"
  - Added: "Showing: URLs in previous crawl that moved to filter of current crawl."
  - New: "Showing: New URLs found in current crawl and filter"
  - Removed: "Showing: URLs in filter for previous crawl, but not in filter for current crawl"
  - Missing: [inferred] "Showing: URLs not found in the current crawl, that previously were in filter"
- **Columns:** normal tabs keep their single-crawl columns. Added and New rows come from the current crawl. Previous, Removed and Missing rows come from the previous crawl, with their original unmapped URLs [inferred].

## 4. Change Detection tab
- **14 filters:** Indexability, Page Titles, Meta Description, H1, Word Count, Crawl Depth, Inlinks, Unique Inlinks, Internal Outlinks, Unique Internal Outlinks, External Outlinks, Unique External Outlinks, Structured Data Unique Types, Content.
  - Each means "Pages that have changed X".
  - Content means pages whose content changed by more than the Similarity Change % (default 10).
- **Overview:**
  - "All" comes first: URLs in both crawls that were analysed.
  - Each element's count is the number of changed URLs.
  - Change % = changed / All.
- **Grid bar:** `[Element ▾] [Changed | Not Changed]`.
- **Columns:**
  - Text elements: `Address | Current X | Previous X`. Indexability adds `Current/Previous Indexability Status`.
  - Numeric elements: `Address | Current X | Previous X | Change | Change %`, where Change % is relative to Previous.
  - Content: `Address | Similarity Match % | Change % | Current Word Count | Previous Word Count`.
- **Thresholds:** only Content has one. Numeric elements flag any non-zero change.
- **Content** needs Store HTML (MinHash over the content area) in both crawls.
- **Ignore Non-Indexable URLs for Change Detection:** default off [inferred: excludes URLs non-indexable in either crawl].
- URLs in only one crawl are excluded.

## 5. URL Mapping
- **Dialog:** "Crawl Comparison Configuration", with pages Change Detection and URL Mapping. URL Mapping has tabs `Regex Replace | Test | Options | Remove Parameters`.
- "Enter one or more regular expressions to map the previous crawl URL into the current crawl URL." Rules apply to **previous-crawl URLs only**, in order, with no limit on their number.
- **Examples:**
  - `screamingfrogcouk.kinsta.cloud` → `www.screamingfrog.co.uk`
  - `example.com/mobile/` → `example.com/`
  - `/$` → (empty)

## 6. Lower window
- **Inlinks/Outlinks:** `Auto | Current | Previous | Added | Removed`.
  - Removed: "Links from the previous crawl that don't exist in the current crawl; URLs have been remapped".
- **View Source:** `Auto | Current | Previous | Compare`, plus Show Differences (side-by-side diff).
- **Rendered Page:** the same options; Compare shows two screenshots side by side.
- **URL Details:** `Name | Current Value | Previous Value`.

## 7. Other surfaces
- **Site Structure:**
  - Tree columns `Path | Current URLs | Previous URLs | Change | Change %`.
  - Crawl Depth chart with grouped bars.
- **Exports:** Grid, Bulk and Reports. Reports > Crawl Overview exports the compare data.
- **Auto Compare:** compares the last two crawls in a project, and the email adds an issues delta table.
- **CLI:** `--crawl-comparison <id> <id>` and `--project-crawl-comparison true`.

## Toad implementation notes
- A comparison is a `.toadcmp` SQLite file. It records both source paths, the compare config and
  the materialised results: per-filter membership for each side, counts, and Change Detection rows.
- Filters are evaluated inside each crawl's own database, because filter SQL uses unqualified
  tables. Their results are joined on the URL key: the current address, or the mapped previous address.
- Change Detection compares internal HTML pages that returned 200 in both crawls.
