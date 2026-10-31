# Screaming Frog filter gap

Screaming Frog SEO Spider filters that had no exact-name match in Screaming Toad, from the 31 Oct 2026 comparison of the two tools' tab and filter lists (Screaming Frog: 34 tabs, 495 filters; Toad: 30 tabs, 311 filters). Status is as of the parity work: phase 1 fixed known defects, phase 2 added accessibility, overview and naming parity, phase 3 adds missing checks.

Filter names are Screaming Frog's. Toad matches filter names loosely in the CLI, MCP server and exports (case, spaces, punctuation and a final "s" are ignored), so most cosmetic differences already resolve; the ones listed as renamed now use Screaming Frog's exact name.


## AMP

| Filter | Status |
|---|---|
| Contains Disallowed HTML | Skipped (AMP is out of scope) |
| Indexable | Skipped (AMP is out of scope) |
| Missing &lt;body&gt; Tag | Skipped (AMP is out of scope) |
| Missing &lt;head&gt; Tag | Skipped (AMP is out of scope) |
| Missing &lt;html amp&gt; Tag | Skipped (AMP is out of scope) |
| Missing Canonical | Skipped (AMP is out of scope) |
| Missing Canonical to Non-AMP | Skipped (AMP is out of scope) |
| Missing Non-AMP Return Link | Skipped (AMP is out of scope) |
| Missing/Invalid &lt;!doctype html&gt; Tag | Skipped (AMP is out of scope) |
| Missing/Invalid &lt;meta charset&gt; Tag | Skipped (AMP is out of scope) |
| Missing/Invalid &lt;meta viewport&gt; Tag | Skipped (AMP is out of scope) |
| Missing/Invalid AMP Boilerplate | Skipped (AMP is out of scope) |
| Missing/Invalid AMP Script | Skipped (AMP is out of scope) |
| Non-200 Response | Skipped (AMP is out of scope) |
| Non-Indexable | Skipped (AMP is out of scope) |
| Non-Indexable Canonical | Skipped (AMP is out of scope) |
| Other Validation Errors | Skipped (AMP is out of scope) |

## Accessibility

| Filter | Status |
|---|---|
| &lt;blink&gt; Elements Deprecated & Must Not Be Used | Phase 2: added |
| &lt;dl&gt; Must Only Have Ordered &lt;dt&gt; & &lt;dd&gt; Groups | Phase 2: added |
| &lt;dt&gt; & &lt;dd&gt; Elements Must Be Contained by &lt;dl&gt; | Phase 2: added |
| &lt;object&gt; Elements Require Alternate Text | Phase 2: added |
| &lt;th&gt; Element Requires Associated Data Cells | Phase 2: added |
| &lt;video&gt; Elements Require &lt;track&gt; For Captions | Phase 2: added |
| &lt;video&gt; or &lt;audio&gt; Elements Must Not Auto-play | Phase 2: added |
| ARIA Attribute Must Be Used As Specified For Role | Phase 2: added |
| ARIA Attributes Require Valid Names | Phase 2: added |
| ARIA Attributes Require Valid Values | Phase 2: added |
| ARIA Commands Require Accessible Name | Phase 2: added |
| ARIA Dialog & Alertdialog Require Accessible Name | Phase 2: added |
| ARIA Input Fields Require Accessible Name | Phase 2: added |
| ARIA Meter Nodes Require Accessible Name | Phase 2: added |
| ARIA Progressbar Nodes Require Accessible Name | Phase 2: added |
| ARIA Role Should Be Appropriate For Element | Phase 2: added |
| ARIA Roles Must Be Contained By Required Parent | Phase 2: added |
| ARIA Roles Require Valid Values | Phase 2: added |
| ARIA Toggle Fields Require Accessible Name | Phase 2: added |
| ARIA Tooltip Nodes Require Accessible Name | Phase 2: added |
| ARIA Treeitem Nodes Require Accessible Name | Phase 2: added |
| Accessibility Score Good | Phase 2: added |
| Accessibility Score Needs Improvement | Phase 2: added |
| Accessibility Score Poor | Phase 2: added |
| Accesskey Attribute Value Must Be Unique | Phase 2: added |
| Active &lt;area&gt; Elements Require Alternate Text | Phase 2: added |
| All Page Content Must Be Contained By Landmarks | Phase 2: added |
| Alt Text Should Not Be Repeated As Text | Phase 2: added |
| Aria-braille Require Non-braille Equivalent | Phase 2: added |
| Aria-hidden Elements Contains Focusable Elements | Phase 2: added |
| Aria-hidden=true Must Not Be Used In &lt;body&gt; | Phase 2: added |
| Autocomplete Attribute Must Be Used Correctly | Phase 2: added |
| Banner Landmark Must Not Be In Another Landmark | Phase 2: added |
| Best Practice Violation | Phase 2: renamed |
| Buttons Require Discernible Text | Phase 2: added |
| Certain ARIA Roles Must Contain Specific Children | Phase 2: added |
| Complementary Landmarks & Asides Must Be Top Level | Phase 2: added |
| Contentinfo Landmark Must Be Top Level Landmark | Phase 2: added |
| Delayed Meta Refresh Must Not Be Used | Phase 2: added |
| Deprecated &lt;marquee&gt; Element Must Not Be Used | Phase 2: added |
| Deprecated ARIA Roles Must Not Be Used | Phase 2: added |
| Elements Marked role=img Require Alternate Text | Phase 2: added |
| Elements Must Not Have Tabindex Greater Than Zero | Phase 2: added |
| Elements Must Only Use Permitted ARIA Attributes | Phase 2: added |
| Elements Must Use Allowed ARIA Attributes | Phase 2: added |
| Ensure Elements Marked Presentational Are Ignored | Phase 2: added |
| Form &lt;input&gt; Elements Require Labels | Phase 2: added |
| Form Elements Should Have Visible Label | Phase 2: added |
| Form Field Must Not Have Multiple Label Elements | Phase 2: added |
| Frames Require Title Attribute | Phase 2: added |
| Frames Require Unique Title Attribute | Phase 2: added |
| Frames Should Be Tested With axe-core | Phase 2: added |
| Frames With Focusable Content Must Not Use tabindex=-1 | Phase 2: added |
| HTML Element Lang Attribute Value Must Be Valid | Phase 2: added |
| HTML Element Requires Lang Attribute | Phase 2: added |
| HTML Lang & XML Lang Value Should Match | Phase 2: added |
| Heading Levels Should Only Increase By One | Phase 2: added |
| Headings Should Not Be Empty | Phase 2: added |
| IDs Used In ARIA & Labels Must Be Unique | Phase 2: added |
| Image Button Requires Alternate Text | Phase 2: added |
| Images Require Alternate Text | Phase 2: added |
| Inline Text Spacing Must Be Adjustable | Phase 2: added |
| Input Buttons Require Discernible Text | Phase 2: added |
| Interactive Controls Must Not Be Nested | Phase 2: added |
| Landmarks Require Unique Role Or Accessible Name | Phase 2: added |
| Lang Attribute Requires Valid Value | Phase 2: added |
| Links Must Be Distinguishable | Phase 2: added |
| Links Require Discernible Text | Phase 2: added |
| Links With Same Accessible Name | Phase 2: added |
| List Items Must Be Contained In List Elements | Phase 2: added |
| Lists Must Only Contain &lt;li&gt; Content Elements | Phase 2: added |
| Main Landmark Must Not Be In Another Landmark | Phase 2: added |
| Meta Viewport Should Allow Zoom & Scale Up to 500% | Phase 2: added |
| Meta Viewport Zoom & Scaling Disabled | Phase 2: added |
| Page Must Contain &lt;h1&gt; | Phase 2: added |
| Page Must Contain &lt;title&gt; | Phase 2: added |
| Page Must Not Have More Than One Banner Landmark | Phase 2: added |
| Page Must Not Have Multiple Contentinfo Landmarks | Phase 2: added |
| Page Requires At Most One Main Landmark | Phase 2: added |
| Page Requires Means To Bypass Repeated Blocks | Phase 2: added |
| Page Requires One Main Landmark | Phase 2: added |
| Required ARIA Attributes Must Be Provided | Phase 2: added |
| Role=text Should Have No Focusable Descendants | Phase 2: added |
| SVG Images & Graphics Require Accessible Text | Phase 2: added |
| Scope Attribute Should Be Used Correctly On Tables | Phase 2: added |
| Scrollable Region Requires Keyboard Access | Phase 2: added |
| Select Element Requires Accessible Name | Phase 2: added |
| Server-Side Image Maps Must Not Be Used | Phase 2: added |
| Skip-link Target Should Exist & Be Focusable | Phase 2: added |
| Summary Elements Require Discernible Text | Phase 2: added |
| Table Header Attr Must Refer To Cell In Same Table | Phase 2: added |
| Table Headers Require Discernible Text | Phase 2: added |
| Table With Identical Summary & Caption Text | Phase 2: added |
| Text Requires Higher Color Contrast Ratio | Phase 2: added |
| Text Requires Higher Color Contrast to Background | Phase 2: added |
| Timed Meta Refresh Must Not Exist | Phase 2: added |
| Touch Targets Require Sufficient Size & Spacing | Phase 2: added |
| WCAG 2.0 A Violation | Phase 2: renamed |
| WCAG 2.0 AA Violation | Phase 2: renamed |
| WCAG 2.0 AAA Violation | Phase 2: renamed |
| WCAG 2.1 AA Violation | Phase 2: renamed |
| WCAG 2.2 AA Violation | Phase 2: renamed |

## Content

| Filter | Status |
|---|---|
| Low Content Pages | Phase 2: renamed |
| Low Relevance Content | Phase 3: added |
| Semantically Similar | Phase 3: added |

## Depth (Clicks from Start URL)

| Filter | Status |
|---|---|
| 0 | Phase 2: Crawl Overview section |
| 1 | Phase 2: Crawl Overview section |
| 10+ | Phase 2: Crawl Overview section |
| 2 | Phase 2: Crawl Overview section |
| 3 | Phase 2: Crawl Overview section |
| 4 | Phase 2: Crawl Overview section |
| 5 | Phase 2: Crawl Overview section |
| 6 | Phase 2: Crawl Overview section |
| 7 | Phase 2: Crawl Overview section |
| 8 | Phase 2: Crawl Overview section |
| 9 | Phase 2: Crawl Overview section |

## External

| Filter | Status |
|---|---|
| Fonts | Phase 3: added |
| Plugins | Phase 3: added |
| XML | Phase 3: added |

## Images

| Filter | Status |
|---|---|
| Missing Size Attributes | Phase 3: added |

## Inlinks (Top 20 URLs)

| Filter | Status |
|---|---|
| 1 | Phase 2: Crawl Overview section |
| 10 | Phase 2: Crawl Overview section |
| 11 | Phase 2: Crawl Overview section |
| 2 | Phase 2: Crawl Overview section |
| 3 | Phase 2: Crawl Overview section |
| 4 | Phase 2: Crawl Overview section |
| 5 | Phase 2: Crawl Overview section |
| 6 | Phase 2: Crawl Overview section |
| 7 | Phase 2: Crawl Overview section |
| 8 | Phase 2: Crawl Overview section |
| 9 | Phase 2: Crawl Overview section |

## Internal

| Filter | Status |
|---|---|
| Fonts | Phase 3: added |
| Plugins | Phase 3: added |
| XML | Phase 3: added |

## JavaScript

| Filter | Status |
|---|---|
| Pages with Chrome Issues | Phase 3: added |
| Pages with JavaScript Warnings | Phase 3: added |

## Mobile

| Filter | Status |
|---|---|
| Contains Unsupported Plugins | Phase 3: added |
| Content Not Sized Correctly | Phase 3: added |
| Illegible Font Size | Phase 3: added |
| Mobile Alternate Link | Phase 3: added |
| Target Size | Phase 3: added |
| Viewport Not Set | Phase 3: added |

## PageSpeed

| Filter | Status |
|---|---|
| Request Errors | Phase 2: renamed |

## Pagination

| Filter | Status |
|---|---|
| Non-200 Pagination URLs | Phase 2: renamed |
| Pagination URL Not in Anchor Tag | Phase 2: added |
| Unlinked Pagination URLs | Phase 2: renamed |

## Response Codes

| Filter | Status |
|---|---|
| Blocked Resource | Phase 3: added |
| External All | Phase 2: added |
| External Blocked Resource | Phase 3: added |
| External Redirection (HTTP Refresh) | Phase 3: added |
| External Redirection (JavaScript) | Phase 3: added |
| Internal All | Phase 2: added |
| Internal Blocked Resource | Phase 3: added |
| Internal Redirection (HTTP Refresh) | Phase 3: added |
| Internal Redirection (JavaScript) | Phase 3: added |
| Redirection (HTTP Refresh) | Phase 3: added |
| Redirection (JavaScript) | Phase 3: added |

## Search Console

| Filter | Status |
|---|---|
| AMP URL Invalid | Skipped (deprecated by Google) |
| Page is Not Mobile Friendly | Skipped (deprecated by Google) |
| URL is on Google But Has Issues | Phase 2: renamed |

## Security

| Filter | Status |
|---|---|
| Bad Content Type | Phase 3: added |

## Sitemaps

| Filter | Status |
|---|---|
| XML Sitemap over 50MB | Phase 2: renamed |

## Structured Data

| Filter | Status |
|---|---|
| Rich Result Feature Detected | Phase 3: added |

## URL

| Filter | Status |
|---|---|
| Broken Bookmark | Phase 3: added |
| Contains Space | Phase 2: renamed |

## Validation

| Filter | Status |
|---|---|
| &lt;body&gt; Element Preceding &lt;html&gt; | Phase 3: added |
| &lt;head&gt; Not First In &lt;html&gt; Element | Phase 3: added |
| HTML Document Over 2MB | Phase 2: renamed |
| High Carbon Rating | Phase 2: renamed |
| Invalid HTML Elements in &lt;head&gt; | Phase 3: added |
| Resource Over 2MB | Phase 2: renamed |

The Inlinks (Top 20 URLs) rows in Screaming Frog are the 20 most linked-to pages of the crawled site (here, client URLs), so they are left out of this list.
## How the phase 3 checks work

- **Needs JavaScript rendering** (Configuration > Spider > Rendering: JavaScript): Redirection (JavaScript), Blocked Resource (also needs robots.txt respected), Pages with JavaScript Warnings and Pages with Chrome Issues (both also need Enable JavaScript Error Reporting), and Mobile's Content Not Sized Correctly, Illegible Font Size and Target Size (Target Size also needs Extraction > Accessibility).
- **Mobile tab**: Screaming Frog fills this tab from PageSpeed Insights (Lighthouse). Toad works the checks out itself so they need no API key: Viewport Not Set from the source, Content Not Sized Correctly when the rendered page is wider than the viewport, Illegible Font Size when more than 40% of the visible text is under 12px (Lighthouse's old rule), Target Size from axe-core's target-size rule, Contains Unsupported Plugins from `<embed>`/`<object>`, and Mobile Alternate Link from `rel="alternate" media` links (needs Spider > Crawl > Mobile Alternate: Store, off by default as in Screaming Frog).
- **Validation**: the three source checks read the HTML as served, since the parser (and the browser) repair it.
- **Broken Bookmark** lists the target pages; Reports > Broken Bookmarks lists each source link. Fragments that are empty, `#top`, hashbangs, SPA routes (`#/…`), text fragments and `key=value` state are ignored.
- **Bad Content Type**: the header isn't a valid MIME type, or names a different kind of file from the body (HTML served as an image, script or stylesheet; an image or PDF served as text). Image subtypes aren't compared.
- **Accessibility**: axe-core leaves `target-size` (WCAG 2.2 AA) and its AAA rules (`color-contrast-enhanced`, `identical-links-same-purpose`, `meta-refresh-no-exceptions`) off by default; Toad switches them on, since Screaming Frog reports them.
- **Content**: Semantically Similar and Low Relevance Content were already built from embeddings; they're now always listed, empty unless API Access > Embeddings is on.
