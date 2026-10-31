//! How To Fix text for the Issues Overview report: one remediation per
//! issue filter, keyed by tab and filter name.

/// The fix for an issue filter, or "" when there isn't one.
pub fn how_to_fix(tab: &str, filter: &str) -> &'static str {
    let tab = tab.trim().to_lowercase();
    let filter = filter.trim().to_lowercase();
    let (t, f) = (tab.as_str(), filter.as_str());
    match t {
        "response codes" => response_codes(f),
        "security" => security(f),
        "url" => url(f),
        "page titles" => page_titles(f),
        "meta description" => meta_description(f),
        "h1" => h1(f),
        "h2" => h2(f),
        "content" => content(f),
        "images" => images(f),
        "canonicals" => canonicals(f),
        "pagination" => pagination(f),
        "directives" => directives(f),
        "hreflang" => hreflang(f),
        "javascript" => javascript(f),
        "accessibility" => accessibility(f),
        "links" => links(f),
        "validation" => validation(f),
        "sitemaps" => sitemaps(f),
        "structured data" => structured_data(f),
        "pagespeed" => pagespeed(f),
        "search console" => search_console(f),
        "analytics" => analytics(f),
        "link metrics" => "Check the API credentials and quota for the link metrics provider, then re-run the request for the affected URLs.",
        "ai" => "Check the AI provider key, model name and rate limits, then re-run the prompts for the affected URLs.",
        _ => "",
    }
}

fn response_codes(f: &str) -> &'static str {
    let f = f.strip_prefix("internal ").or_else(|| f.strip_prefix("external ")).unwrap_or(f);
    match f {
        "blocked by robots.txt" => "Allow the URL in robots.txt if it should be crawled and indexed. If the block is intentional, remove internal links to it.",
        "no response" => "Check the URL for typos, DNS or SSL faults and server timeouts. Fix or remove links that point at addresses which never answer.",
        "redirection (3xx)" => "Update internal links to point straight at the final destination URL rather than relying on the redirect.",
        "redirection (meta refresh)" => "Replace the meta refresh with a server-side 301 redirect, and update internal links to the final URL.",
        "redirect chain" => "Redirect the first URL directly to the final destination in a single hop, and update internal links to match.",
        "redirect loop" => "Break the cycle so the redirect rules end at a page that returns a 200 response.",
        "client error (4xx)" => "Restore the missing page, or 301 redirect it to the closest relevant URL. Update or remove any links pointing at it.",
        "server error (5xx)" => "Look at the server logs to find why the request fails. Fix the fault, then recrawl to confirm the URL returns a 200 response.",
        _ => "",
    }
}

fn security(f: &str) -> &'static str {
    match f {
        "http urls" => "Serve the page over HTTPS and 301 redirect the HTTP version to it. Update internal links, canonicals and sitemaps to use HTTPS.",
        "mixed content" => "Load every image, script and stylesheet on HTTPS pages over HTTPS, so browsers do not block or warn about them.",
        "form url insecure" => "Change the form action to an HTTPS address so submitted data is encrypted in transit.",
        "form on http url" => "Move the page containing the form to HTTPS before visitors enter any details.",
        "unsafe cross-origin links" => "Add rel=\"noopener\" (and noreferrer where suitable) to external links that open in a new tab.",
        "protocol-relative resource links" => "Replace protocol-relative (//) resource URLs with explicit HTTPS addresses.",
        "missing hsts header" => "Send a Strict-Transport-Security header on HTTPS responses so browsers always use a secure connection.",
        "missing content-security-policy header" => "Add a Content-Security-Policy header that limits where scripts and other resources may load from, to reduce injection risk.",
        "missing x-content-type-options header" => "Send X-Content-Type-Options: nosniff so browsers respect the declared content type.",
        "missing x-frame-options header" => "Send X-Frame-Options (or a CSP frame-ancestors rule) to stop other sites embedding the page in a frame.",
        "missing secure referrer-policy header" => "Send a Referrer-Policy such as strict-origin-when-cross-origin to limit what URL detail leaks to other sites.",
        _ => "",
    }
}

fn url(f: &str) -> &'static str {
    match f {
        "non ascii characters" => "Use plain ASCII letters, digits and hyphens in URLs, or percent-encode unavoidable characters consistently, then redirect the old address.",
        "underscores" => "Use hyphens to separate words in URLs, and 301 redirect the old address to the new one.",
        "uppercase" => "Standardise on lowercase URLs and redirect mixed-case variants to the lowercase version.",
        "multiple slashes" => "Remove the repeated slashes from the link and redirect the malformed address to the clean one.",
        "repetitive path" => "Check for a faulty relative link or rewrite rule that keeps repeating a folder, and correct it.",
        "contains a space" => "Replace spaces with hyphens in the path and redirect the old address, since spaces appear as %20.",
        "internal search" => "Keep internal search result pages out of the index with noindex or robots.txt, and avoid linking to them.",
        "parameters" => "Avoid crawlable parameter URLs where possible. Canonicalise them to the clean version or stop search engines crawling them.",
        "ga tracking parameters" => "Remove analytics tracking parameters from internal links and canonicalise to the clean URL.",
        _ if f.starts_with("over ") && f.ends_with(" characters") => "Shorten the URL by removing filler words and unnecessary folders, and redirect the old address if it changes.",
        _ => "",
    }
}

fn page_titles(f: &str) -> &'static str {
    match f {
        "missing" => "Write a unique, descriptive title for the page that includes its main topic.",
        "duplicate" => "Give each page its own title that reflects what makes that page different.",
        "multiple" => "Keep a single title element in the head and remove the extras.",
        "outside <head>" => "Move the title element into the head. Titles in the body are often ignored.",
        "same as h1" => "Vary the title so it adds context or a call to action rather than repeating the heading word for word.",
        _ if f.starts_with("over ") && f.ends_with(" characters") => "Shorten the title so the key words appear before any truncation in search results.",
        _ if f.starts_with("below ") && f.ends_with(" characters") => "Expand the title with relevant descriptive detail so it uses the space available.",
        _ if f.starts_with("over ") && f.ends_with(" pixels") => "Trim the title, as wide letters mean it may be cut off in search results.",
        _ if f.starts_with("below ") && f.ends_with(" pixels") => "Lengthen the title with useful detail so it makes better use of the display width.",
        _ => "",
    }
}

fn meta_description(f: &str) -> &'static str {
    match f {
        "missing" => "Write a unique summary of the page that encourages searchers to click.",
        "duplicate" => "Write a distinct description for each page rather than reusing the same text.",
        "multiple" => "Keep one meta description tag and delete the others.",
        "outside <head>" => "Move the meta description into the head so search engines read it reliably.",
        _ if f.starts_with("over ") && f.ends_with(" characters") => "Shorten the description so the important message is not truncated in search results.",
        _ if f.starts_with("below ") && f.ends_with(" characters") => "Add more detail about what the page offers so the description fills the available space.",
        _ if f.starts_with("over ") && f.ends_with(" pixels") => "Cut the description down, since it is likely to be truncated at this width.",
        _ if f.starts_with("below ") && f.ends_with(" pixels") => "Extend the description with relevant detail so it uses more of the display width.",
        _ => "",
    }
}

fn h1(f: &str) -> &'static str {
    match f {
        "missing" => "Add one H1 that clearly describes the page's main subject.",
        "duplicate" => "Give each page its own H1 that identifies its specific topic.",
        "multiple" => "Use a single H1 for the main heading, and demote the others to H2 or lower.",
        "alt text in h1" => "Use real text in the H1 where you can, so the heading does not depend on an image's alt text.",
        "non-sequential" => "Start the heading order with an H1 and avoid skipping levels, so the outline stays logical.",
        _ if f.starts_with("over ") && f.ends_with(" characters") => "Shorten the H1 so it stays concise and easy to scan.",
        _ => "",
    }
}

fn h2(f: &str) -> &'static str {
    match f {
        "missing" => "Break the content up with H2 headings that label each main section.",
        "duplicate" => "Reword repeated H2s so each one describes its own section.",
        "multiple" => "Multiple H2s are normal. Check that each marks a distinct section and the page structure still reads clearly.",
        "non-sequential" => "Order headings logically, so an H2 does not appear before the page's H1.",
        _ if f.starts_with("over ") && f.ends_with(" characters") => "Shorten the H2 to a brief, descriptive section label.",
        _ => "",
    }
}

fn content(f: &str) -> &'static str {
    match f {
        "exact duplicates" => "Consolidate identical pages: remove the copies, 301 redirect them, or canonicalise them to the preferred URL.",
        "near duplicates" => "Rewrite the pages so each has distinct content, or merge them and canonicalise to the strongest one.",
        "soft 404 pages" => "Return a real 404 or 410 status for missing content, or replace the thin page with genuinely useful content.",
        "readability difficult" | "readability very difficult" => "Use shorter sentences and simpler words so the intended audience can read the page more easily.",
        "spelling errors" => "Correct the misspelt words, then recrawl to confirm the page is clean.",
        "grammar errors" => "Edit the flagged sentences for grammar, then recrawl to confirm the page is clean.",
        "lorem ipsum placeholder" => "Replace the placeholder text with real copy before the page is published or indexed.",
        "high carbon rating" => "Reduce page weight by compressing images, trimming scripts and caching assets, to cut the emissions per view.",
        "semantically similar" => "Review the closely matching pages for overlapping topics, then merge, differentiate or canonicalise them.",
        "low relevance content" => "Check whether the page fits the site's focus. Rework it to match the topic, or consider removing it.",
        _ if f.starts_with("low content pages") => "Expand the page with useful, original content, or noindex or remove it if it has no standalone value.",
        _ => "",
    }
}

fn images(f: &str) -> &'static str {
    match f {
        "missing alt text" => "Add alt text describing meaningful images. Decorative ones should have an empty alt attribute on purpose.",
        "missing alt attribute" => "Add an alt attribute to every image, with descriptive text or an empty value for decorative images.",
        "incorrectly sized images" => "Serve images close to the dimensions they are shown at, using responsive srcset sizes.",
        _ if f.starts_with("over ") && f.ends_with(" kb") => "Compress the image, use a modern format such as WebP or AVIF, and resize it to its display size.",
        _ if f.starts_with("alt text over ") && f.ends_with(" characters") => "Shorten the alt text to a brief description of what the image shows.",
        _ => "",
    }
}

fn canonicals(f: &str) -> &'static str {
    match f {
        "canonicalised" => "Check that the canonical points to the right page, as this URL will be left out in favour of the target.",
        "missing" => "Add a self-referencing canonical to indexable pages to protect against duplicate URL variants.",
        "multiple" => "Keep one canonical declaration per page and remove the duplicates.",
        "multiple conflicting" => "Make the HTML and header canonicals agree on a single URL.",
        "non-indexable canonical" => "Point the canonical at a live, indexable URL that returns 200, not a redirect, error or noindex page.",
        "canonical is relative" => "Use an absolute URL in the canonical to avoid it being resolved against the wrong base.",
        "canonical chain" => "Point the canonical directly at the final preferred URL, not at a page that canonicalises elsewhere.",
        "unlinked" => "Link to the canonical URL from within the site so search engines can discover and trust it.",
        "invalid attribute in annotation" => "Remove unsupported attributes from the canonical link element, and keep to rel and href.",
        "contains fragment url" => "Remove the #fragment from the canonical, since search engines ignore it.",
        "outside <head>" => "Move the canonical link into the head, where search engines expect it.",
        _ => "",
    }
}

fn pagination(f: &str) -> &'static str {
    match f {
        "non-200 pagination url" => "Point rel next and prev at pages that return 200, and fix or remove links to broken pages in the series.",
        "non-indexable" => "Make paginated pages indexable and avoid noindex or canonicals that point away from the series.",
        "multiple pagination urls" => "Declare a single next and a single previous URL per page.",
        "unlinked pagination url" => "Add normal crawlable links to the next and previous pages so they can be found.",
        "pagination loop" => "Correct the markup so the sequence moves forward without returning to an earlier page.",
        "sequence error" => "Check that each page's next and previous URLs match the pages around it in the series.",
        _ => "",
    }
}

fn directives(f: &str) -> &'static str {
    match f {
        "noindex" => "Confirm the page should stay out of search results. If not, remove the noindex directive.",
        "nofollow" => "Remove nofollow from the page directive unless you really want its links ignored.",
        "none" => "The none directive blocks indexing and link following. Remove it if the page should appear in search.",
        "nosnippet" => "Remove nosnippet if you want search engines to show a text description of the page.",
        "noodp" | "noydir" => "This directive is obsolete and has no effect. Remove it from the markup.",
        "noimageindex" => "Remove noimageindex if the page's images should be eligible for image search.",
        "notranslate" => "Remove notranslate if you want searchers to be offered a translation of the page.",
        "unavailable_after" => "Check the expiry date is intended, as the page will leave search results after it.",
        "outside <head>" => "Move meta robots tags into the head, where search engines will respect them.",
        _ => "",
    }
}

fn hreflang(f: &str) -> &'static str {
    match f {
        "non-200 hreflang urls" => "Point hreflang annotations only at live URLs that return 200.",
        "missing return links" => "Have each alternate page link back to this one, as hreflang must be reciprocal.",
        "inconsistent language & region return links" => "Align the language and region codes so each pair of pages references the other consistently.",
        "non-canonical return links" => "Use canonical URLs in hreflang annotations rather than variants that canonicalise elsewhere.",
        "noindex return links" => "Remove hreflang references to pages marked noindex, or make those pages indexable.",
        "unlinked hreflang urls" => "Link to the alternate language pages from within the site so they can be discovered.",
        "multiple entries" => "Include only one hreflang entry per language and region combination.",
        "not using canonical" => "Make the page's hreflang URL match its canonical URL.",
        "incorrect language & region codes" => "Use valid ISO 639-1 language codes with optional ISO 3166-1 region codes.",
        "missing self reference" => "Add an hreflang entry for the page itself alongside its alternates.",
        "missing x-default" => "Add an x-default entry for visitors whose language isn't covered by the others.",
        "outside <head>" => "Move hreflang link elements into the head so they are read reliably.",
        _ => "",
    }
}

fn javascript(f: &str) -> &'static str {
    match f {
        "pages with blocked resources" => "Allow the blocked scripts, styles and images in robots.txt so search engines can render the page fully.",
        "contains javascript links" => "Use standard anchor tags with href attributes for important links, so they work without script.",
        "contains javascript content" => "Put key content in the initial HTML, or use server-side rendering, so it doesn't rely on script execution.",
        "noindex only in original html" => "Make the original HTML and rendered page agree on indexing. Search engines may see noindex before script removes it.",
        "nofollow only in original html" => "Align the directive in the raw and rendered HTML so link following is consistent.",
        "canonical only in rendered html" => "Output the canonical in the server HTML rather than adding it with script.",
        "canonical mismatch" => "Stop script rewriting the canonical, or make the original and rendered values match.",
        "page title only in rendered html" => "Include the title in the initial HTML instead of injecting it with script.",
        "page title updated by javascript" => "Set the final title in the server response so search engines see the same value either way.",
        "meta description only in rendered html" => "Put the meta description in the original HTML rather than adding it client-side.",
        "meta description updated by javascript" => "Output the final meta description from the server, so no script rewrite is needed.",
        "h1 only in rendered html" => "Include the H1 in the original HTML so it exists before scripts run.",
        "h1 updated by javascript" => "Serve the final H1 in the initial HTML, rather than changing it with script.",
        "uses old ajax crawling scheme urls" => "Move to clean, crawlable URLs using the History API, since hashbang URLs are no longer supported.",
        "uses old ajax crawling scheme meta fragment tag" => "Remove the fragment meta tag. The AJAX crawling scheme is deprecated and ignored.",
        "pages with javascript errors" => "Open the browser console on the page, fix the failing scripts, and recrawl to confirm they run cleanly.",
        _ => "",
    }
}

fn accessibility(f: &str) -> &'static str {
    match f {
        "best practice violations" => "Work through the best practice failures in the All Accessibility Violations export. They go beyond the WCAG requirements but improve usability.",
        "wcag 2.0 a violations" => "Fix these level A failures first, as they block basic access for many users. Use the All Accessibility Violations export to find the elements.",
        "wcag 2.0 aa violations" => "Correct the level AA failures listed in the All Accessibility Violations export, since this level is the usual legal target.",
        "wcag 2.0 aaa violations" => "Review the level AAA failures in the All Accessibility Violations export, and fix those practical for your content.",
        "wcag 2.1 a violations" => "Fix the WCAG 2.1 level A failures shown in the All Accessibility Violations export, starting with the highest impact.",
        "wcag 2.1 aa violations" => "Fix the WCAG 2.1 level AA failures shown in the All Accessibility Violations export, working through the elements flagged.",
        "wcag 2.2 aa violations" => "Fix the WCAG 2.2 level AA failures shown in the All Accessibility Violations export, working through the elements flagged.",
        _ => "Fix the failing elements listed in the All Accessibility Violations export, following the linked axe-core rule help for each one.",
    }
}

fn links(f: &str) -> &'static str {
    match f {
        "pages with high crawl depth" => "Link to deep pages from higher-level pages, menus or hubs so they sit closer to the home page.",
        "pages without internal outlinks" => "Add links to relevant pages so visitors and crawlers can carry on from here.",
        "pages with uncrawlable internal outlinks" => "Replace links that rely on script or missing hrefs with standard anchor tags.",
        "internal nofollow outlinks" => "Remove nofollow from internal links unless you intend to stop their value flowing.",
        "internal outlinks with no anchor text" => "Give each link descriptive anchor text, or an accessible label if it wraps an image.",
        "non-descriptive anchor text in internal outlinks" => "Replace generic anchors such as 'click here' with text that describes the destination.",
        "pages with high external outlinks" => "Review the external links and remove any that are low quality or unnecessary.",
        "pages with high internal outlinks" => "Cut back the number of internal links so the important ones stand out.",
        "follow & nofollow internal inlinks to page" => "Make internal links to the page consistent, and use follow unless there is a reason not to.",
        "internal nofollow inlinks only" => "Add at least one normal followed internal link to the page.",
        "outlinks to localhost" => "Replace localhost links left over from development with the live site's URLs.",
        "non-indexable page inlinks only" => "Link to the page from at least one indexable page so it can be discovered.",
        _ => "",
    }
}

fn validation(f: &str) -> &'static str {
    match f {
        "missing <head> tag" => "Add a head element to the document so metadata is placed correctly.",
        "multiple <head> tags" => "Keep a single head element. Check for stray markup that closes it early.",
        "missing <body> tag" => "Add a body element around the page content.",
        "multiple <body> tags" => "Keep a single body element and fix any markup that ends it too soon.",
        _ if f.starts_with("html document over ") => "Reduce the HTML size by removing inline bloat and unused markup, and paginating or lazy-loading long content.",
        _ if f.starts_with("resource over ") => "Compress or split the oversized resource so it loads faster and stays within crawl limits.",
        _ => "",
    }
}

fn sitemaps(f: &str) -> &'static str {
    match f {
        "urls not in sitemap" => "Add indexable pages to the XML sitemap so search engines can find them.",
        "orphan urls" => "Link to these sitemap URLs from the site's navigation or content, or remove them from the sitemap.",
        "non-indexable urls in sitemap" => "Remove noindexed, redirected and error URLs from the sitemap, leaving only indexable pages.",
        "urls in multiple sitemaps" => "List each URL in one sitemap only, to keep the files simple to maintain.",
        "xml sitemap with over 50k urls" => "Split the sitemap into several files and reference them from a sitemap index.",
        "xml sitemap with over 50mb" => "Break the file into smaller sitemaps, or compress them, so each stays under the size limit.",
        _ => "",
    }
}

fn structured_data(f: &str) -> &'static str {
    match f {
        "missing" => "Add structured data suited to the page type, such as Article, Product or Organization, to qualify for rich results.",
        "validation errors" => "Correct the schema errors reported, supplying all required properties with valid values.",
        "validation warnings" => "Add the recommended properties shown in the warnings to make the markup more complete.",
        "parse errors" => "Fix the syntax of the markup, such as stray commas or unclosed brackets in JSON-LD.",
        "rich result validation errors" => "Supply the required properties for the rich result type so the page becomes eligible.",
        "rich result validation warnings" => "Add the recommended properties for the rich result to improve its eligibility and detail.",
        _ => "",
    }
}

fn pagespeed(f: &str) -> &'static str {
    match f {
        "failed core web vitals assessment" => "Improve whichever of LCP, INP or CLS is poor for real users. Use the PageSpeed detail to find the cause.",
        "performance score below 50" => "Work through the highest-impact Lighthouse opportunities for the page, such as images, scripts and render-blocking resources.",
        "request error" => "Check the PageSpeed API key, quota and that the URL is reachable, then re-run the request.",
        "render blocking requests" => "Defer or inline critical CSS and scripts so they do not hold up the first paint.",
        "document request latency" => "Reduce server response time with caching, a faster host or a CDN, and avoid redirects on the document request.",
        "improve image delivery" => "Compress images, use modern formats and serve sizes matched to the display.",
        "lcp request discovery" => "Make the largest content element discoverable in the HTML, and preload it rather than loading it late.",
        "lcp breakdown" => "Find the slowest phase of LCP in the breakdown and shorten it.",
        "layout shift culprits" => "Reserve space for images, embeds and late-loading content so the layout does not jump.",
        "optimize dom size" => "Simplify the markup and remove unneeded nested elements to bring the DOM size down.",
        "duplicated javascript" => "Remove duplicate modules from the bundles so each library ships once.",
        "font display" => "Set font-display: swap (or optional) so text stays visible while web fonts load.",
        "inp breakdown" => "Shorten input delay, processing time and presentation delay by reducing main-thread work after interactions.",
        "legacy javascript" => "Stop transpiling and polyfilling features modern browsers support, to ship less code.",
        "modern http" => "Serve assets over HTTP/2 or HTTP/3 rather than HTTP/1.1.",
        "network dependency tree" => "Shorten request chains by preloading key resources and removing unnecessary dependencies.",
        "3rd parties" => "Remove or defer third-party scripts that are not essential, and load the rest asynchronously.",
        "use efficient cache lifetimes" => "Set long cache lifetimes on static assets, using versioned file names so updates still propagate.",
        "optimize viewport for mobile" => "Add a responsive viewport meta tag and avoid settings that delay tap responses.",
        "forced reflow" => "Batch layout reads and writes in scripts so the browser isn't forced to recalculate layout repeatedly.",
        "reduce unused css" => "Strip unused rules from stylesheets, or split CSS so each page loads only what it needs.",
        "reduce unused javascript" => "Split bundles and lazy-load code that is not needed on first load.",
        "minify css" => "Minify CSS files as part of the build.",
        "minify javascript" => "Minify JavaScript files as part of the build.",
        "minimize main-thread work" => "Cut down script, style and layout work, and break up heavy tasks.",
        "reduce javascript execution time" => "Ship less JavaScript and defer heavy work until it is needed.",
        "avoid long main-thread tasks" => "Split long-running scripts into smaller chunks so the page stays responsive.",
        "avoid non-composited animations" => "Animate transform and opacity instead of properties that trigger layout or paint.",
        "image elements do not have explicit width & height" => "Set width and height attributes on images so the browser can reserve space before they load.",
        "avoid enormous network payloads" => "Reduce total page weight by compressing assets and removing resources the page does not need.",
        _ => "",
    }
}

fn search_console(f: &str) -> &'static str {
    match f {
        "non-indexable with search analytics data" => "Check whether the page should rank. If so, make it indexable. Otherwise redirect it to a relevant indexable page.",
        "orphan urls" => "Add internal links to pages that earn impressions, or redirect or remove them if they are no longer wanted.",
        "url is not on google" => "Find out why the URL is excluded using the inspection result, then fix the cause and request indexing.",
        "indexable url not indexed" => "Improve the page's content and internal links, make sure it is in the sitemap, then request indexing.",
        "url is on google, but has issues" => "Open the URL inspection details and fix the enhancement problems reported, such as rich results markup.",
        "user-declared canonical not selected" => "Make the page's content and signals support the canonical you declared, or update the canonical to the URL Google prefers.",
        "rich result invalid" => "Correct the structured data errors that the URL Inspection reports so the rich result becomes valid.",
        "request error" => "Check the Search Console connection, property access and API quota, then retry the request.",
        _ => "",
    }
}

fn analytics(f: &str) -> &'static str {
    match f {
        "bounce rate above 70%" => "Review the page against the visitor's intent. Improve the content, speed and next-step links so more visits stay engaged.",
        "non-indexable with ga data" => "If the page matters to visitors, make it indexable. If not, redirect it to a relevant indexable page.",
        "orphan urls" => "Link to pages that receive visits from within the site, or redirect them if they are obsolete.",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::how_to_fix;

    #[test]
    fn known_filters_have_a_fix() {
        assert!(!how_to_fix("Page Titles", "Missing").is_empty());
        assert!(!how_to_fix("Security", "HTTP URLs").is_empty());
        assert!(!how_to_fix("PageSpeed", "Minify CSS").is_empty());
    }

    #[test]
    fn matching_ignores_case() {
        assert_eq!(how_to_fix("page titles", "MISSING"), how_to_fix("Page Titles", "Missing"));
    }

    #[test]
    fn numbered_filters_match_by_shape() {
        assert!(!how_to_fix("Page Titles", "Over 70 Characters").is_empty());
        assert!(!how_to_fix("Page Titles", "Below 25 Pixels").is_empty());
        assert!(!how_to_fix("Images", "Over 250 KB").is_empty());
        assert!(!how_to_fix("Images", "Alt Text Over 80 Characters").is_empty());
        assert!(!how_to_fix("Content", "Low Content Pages (under 150 words)").is_empty());
        assert!(!how_to_fix("Validation", "HTML Document Over 3 MB").is_empty());
    }

    #[test]
    fn response_codes_cover_internal_and_external() {
        assert_eq!(how_to_fix("Response Codes", "Internal Client Error (4XX)"), how_to_fix("Response Codes", "External Client Error (4XX)"));
        assert!(!how_to_fix("Response Codes", "Internal Redirect Chain").is_empty());
    }

    #[test]
    fn accessibility_falls_back_to_the_export() {
        assert!(how_to_fix("Accessibility", "color-contrast").contains("All Accessibility Violations"));
        assert!(!how_to_fix("Accessibility", "WCAG 2.0 AA Violations").is_empty());
    }

    #[test]
    fn unknown_is_empty() {
        assert_eq!(how_to_fix("Page Titles", "No Such Filter"), "");
        assert_eq!(how_to_fix("No Such Tab", "Missing"), "");
    }
}
