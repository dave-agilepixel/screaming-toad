//! Crawl a small local site with known problems and check the filters find them.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use toad_core::config::{CustomExtractor, CustomSearch};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler};
use toad_store::{RowQuery, SitemapOptions, Store};
use url::Url;

/// 80 words shared by /a and /orphan, so they are near (not exact) duplicates.
const SHARED: &str = "Toads are amphibians that live on land for most of the year and return to ponds to breed in spring. \
They have dry warty skin, short legs and a broad body, and they walk rather than hop. \
Common toads eat insects, slugs and worms, and they can live for many years in a garden. \
Females lay long strings of eggs that wrap around plants in shallow water, and tadpoles hatch after about two weeks.";

fn page(title: &str, body: &str, head_extra: &str) -> String {
    format!("<!doctype html><html lang=\"en\"><head><title>{title}</title>{head_extra}</head><body>{body}</body></html>")
}

fn respond(path: &str, origin: &str) -> (u16, &'static str, Vec<(&'static str, String)>, String) {
    match path {
        "/" => (200, "text/html", vec![], page(
            "Home page of the test site for crawling",
            r#"<h1>Home</h1><p>Welcome.</p><script>gtag('config', 'G-1');</script>
               <a href="/a">A</a> <a href="/b">B</a> <a href="/c">C</a> <a href="/missing">Missing</a>
               <a href="/redirect">Old</a> <a href="/private/x">Private</a> <a href="/a">click here</a>
               <a href="/fr">Français</a> <img src="/img.png">"#,
            r#"<meta name="description" content="Home">
               <script type="application/ld+json">{"@context":"https://schema.org","@type":"Product","name":"Toad"}</script>"#,
        )),
        "/a" => (200, "text/html", vec![], page(
            "Dup",
            &format!("<h1>A</h1><p>{SHARED}</p><a href=\"/\">Home</a>"),
            r#"<link rel="alternate" hreflang="en" href="/a"><link rel="alternate" hreflang="fr" href="/fr">"#,
        )),
        "/fr" => (200, "text/html", vec![], page(
            "Page en français pour le test hreflang",
            "<h1>Bonjour</h1><a href=\"/\">Accueil</a>",
            r#"<link rel="alternate" hreflang="fr" href="/fr"><link rel="alternate" hreflang="en-gb" href="/a">"#,
        )),
        "/orphan" => (200, "text/html", vec![], page(
            "An orphan page that is only in the sitemap",
            &format!("<h1>Orphan</h1><p>{SHARED}</p><a href=\"/\">Home</a>"),
            "",
        )),
        "/b" => (200, "text/html", vec![], page("Dup", "<h1>B</h1><a href=\"/\">Home</a>", r#"<meta name="robots" content="noindex">"#)),
        "/c" => (200, "text/html", vec![], page("Dup", "<h2>No H1 here</h2><a href=\"/\">Home</a>", "")),
        "/redirect" => (301, "text/html", vec![("Location", "/a".to_string())], String::new()),
        "/img.png" => (200, "image/png", vec![], "PNGDATA".to_string()),
        "/robots.txt" => (200, "text/plain", vec![], format!("User-agent: *\nDisallow: /private/\nSitemap: {origin}/sitemap.xml\n")),
        "/sitemap.xml" => (200, "application/xml", vec![], format!(
            "<?xml version=\"1.0\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\
             <url><loc>{origin}/</loc></url><url><loc>{origin}/a</loc></url>\
             <url><loc>{origin}/b</loc></url><url><loc>{origin}/orphan</loc></url></urlset>"
        )),
        _ => (404, "text/html", vec![], page("Not Found", "<h1>404</h1>", "")),
    }
}

async fn serve(listener: TcpListener, origin: String) {
    loop {
        let Ok((mut sock, _)) = listener.accept().await else { return };
        let origin = origin.clone();
        tokio::spawn(async move {
            let mut buf = vec![0u8; 8192];
            let n = sock.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let (status, ctype, headers, body) = respond(&path, &origin);
            let mut head = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            );
            for (k, v) in headers {
                head.push_str(&format!("{k}: {v}\r\n"));
            }
            head.push_str("\r\n");
            let _ = sock.write_all(head.as_bytes()).await;
            let _ = sock.write_all(body.as_bytes()).await;
            let _ = sock.shutdown().await;
        });
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn crawl_finds_known_issues() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let origin = format!("http://127.0.0.1:{port}");
    tokio::spawn(serve(listener, origin.clone()));

    let start = Url::parse(&format!("{origin}/")).unwrap();
    let mut cfg = CrawlConfig::default();
    cfg.crawl_linked_sitemaps = true;
    cfg.near_duplicates = true;
    cfg.custom_search = vec![CustomSearch { name: "GA tag".into(), query: "gtag(".into(), ..Default::default() }];
    cfg.custom_extraction = vec![CustomExtractor { name: "H1 text".into(), kind: toad_core::config::ExtractorKind::Css, expression: "h1".into(), ..Default::default() }];

    let dir = std::env::temp_dir().join(format!("toad-e2e-{port}"));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("crawl.toad");
    let mut store = Store::create(&db, &start, &cfg).unwrap();

    let (tx, mut rx) = tokio::sync::mpsc::channel(256);
    let crawler = Crawler::new(cfg, &start).unwrap();
    let handle = tokio::spawn(crawler.run(vec![start.clone()], tx, CrawlControl::default()));
    let mut records = Vec::new();
    let mut sitemaps = Vec::new();
    while let Some(ev) = rx.recv().await {
        match ev {
            CrawlEvent::Page(r) => records.push(*r),
            CrawlEvent::Sitemap { sitemap, urls } => sitemaps.push((sitemap, urls)),
            CrawlEvent::Progress(_) => {}
            CrawlEvent::Finished(_) => break,
        }
    }
    handle.await.unwrap().unwrap();
    store.insert_batch(&records).unwrap();
    for (sitemap, urls) in &sitemaps {
        store.insert_sitemap(sitemap, urls).unwrap();
    }
    store.analyse().unwrap();

    let count = |tab: &str, filter: &str| store.count(tab, filter).unwrap();

    // Response codes and robots.txt.
    assert_eq!(count("Response Codes", "Internal Client Error (4XX)"), 1, "/missing");
    assert_eq!(count("Response Codes", "Internal Redirection (3XX)"), 1, "/redirect");
    assert_eq!(count("Response Codes", "Blocked by Robots.txt"), 1, "/private/x");

    // On-page. /b is noindex, so with "ignore non-indexable for issues" only /a and /c count.
    assert_eq!(count("Page Titles", "Duplicate"), 2);
    assert_eq!(count("Page Titles", "Below 30 Characters"), 2);
    assert_eq!(count("Directives", "Noindex"), 1);
    assert_eq!(count("H1", "Missing"), 1, "/c");
    assert_eq!(count("Images", "Missing Alt Attribute"), 1);
    assert_eq!(count("Links", "Non-Descriptive Anchor Text In Internal Outlinks"), 1);
    assert!(count("Meta Description", "Missing") >= 2);

    // Sitemaps: found through robots.txt.
    assert_eq!(sitemaps.len(), 1);
    assert_eq!(count("Sitemaps", "URLs In Sitemap"), 4);
    assert_eq!(count("Sitemaps", "Orphan URLs"), 1, "/orphan");
    assert_eq!(count("Sitemaps", "Non-Indexable URLs in Sitemap"), 1, "/b");
    assert_eq!(count("Sitemaps", "URLs Not In Sitemap"), 2, "/c and /fr");

    // Near duplicates: /a and /orphan share their body text.
    assert_eq!(count("Content", "Near Duplicates"), 2);
    let near = store.near_duplicates(&format!("{origin}/a")).unwrap();
    assert_eq!(near.rows.len(), 1);
    assert_eq!(near.rows[0][0], format!("{origin}/orphan"));

    // Custom search and extraction.
    assert_eq!(count("Custom Search", "GA tag"), 1, "only the home page has gtag(");
    assert_eq!(count("Custom Extraction", "H1 text"), 5, "/, /a, /b, /fr and /orphan have an H1");
    let extracted = store
        .rows(&RowQuery { tab: "Custom Extraction".into(), filter: "H1 text".into(), search: Some(format!("{origin}/")), ..Default::default() })
        .unwrap();
    let h1_col = extracted.columns.iter().position(|c| c == "H1 text 1").unwrap();
    assert!(extracted.rows.iter().any(|r| r[h1_col] == "Home"));

    // Links and Link Score.
    let inlinks = store.inlinks(&format!("{origin}/a")).unwrap();
    assert!(inlinks.rows.len() >= 3, "home (twice) and the redirect link to /a");
    let home_score: f64 = store
        .url_details(start.as_str())
        .unwrap()
        .into_iter()
        .find(|(k, _)| k == "link_score")
        .and_then(|(_, v)| v.parse().ok())
        .unwrap();
    assert!(home_score > 50.0, "home page should have the top Link Score, got {home_score}");

    let issues = store.issues().unwrap();
    assert!(issues.iter().any(|i| i.filter == "Internal Client Error (4xx)"));

    // Structured data: the home page's Product has no offers, review or rating.
    assert_eq!(count("Structured Data", "Contains Structured Data"), 1);
    // Google rich result issues and schema.org issues are separate filters.
    assert_eq!(count("Structured Data", "Rich Result Validation Errors"), 1);
    assert_eq!(count("Structured Data", "Validation Errors"), 0, "valid schema.org");
    let sd = store.structured_details(start.as_str()).unwrap();
    assert!(sd.rows.iter().any(|r| r[0] == "Error" && r[1] == "Google" && r[3] == "Product"));
    let rich = store.report("Google Rich Results Features Summary").unwrap();
    assert_eq!(rich.rows, vec![vec!["Product".to_string(), "1".into(), "1".into(), "1".into(), "0".into()]]);

    // Hreflang: /fr links back to /a as en-gb, but /a calls itself en.
    assert_eq!(count("Hreflang", "Contains Hreflang"), 2);
    assert_eq!(count("Hreflang", "Inconsistent Language & Region Return Links"), 1, "/a");
    assert_eq!(count("Hreflang", "Missing Return Links"), 0);
    assert_eq!(count("Hreflang", "Missing X-Default"), 2);

    // Reports and bulk exports.
    let report_rows = |name: &str| store.report(name).unwrap().rows.len();
    assert_eq!(report_rows("All Redirects"), 1);
    assert_eq!(report_rows("Redirect Chains"), 0, "/redirect is a single hop");
    assert_eq!(report_rows("Orphan Pages"), 1);
    assert_eq!(report_rows("Hreflang: All Hreflang URLs"), 4);
    assert!(report_rows("Client Error (4XX) Inlinks") >= 1, "home links to /missing");
    assert!(report_rows("All Inlinks") > 10);
    for r in store.reports() {
        store.report(r.name).unwrap_or_else(|e| panic!("report {} failed: {e}", r.name));
    }

    // XML sitemap generation: indexable 200 pages only.
    let out = dir.join("sitemap");
    let files = store.generate_sitemap(&out, &SitemapOptions::default()).unwrap();
    assert_eq!(files.len(), 1);
    let xml = std::fs::read_to_string(&files[0]).unwrap();
    for included in ["/", "/a", "/c", "/fr", "/orphan"] {
        assert!(xml.contains(&format!("<loc>{origin}{included}</loc>")), "{included} should be in the sitemap");
    }
    for excluded in ["/b", "/redirect", "/missing", "/img.png"] {
        assert!(!xml.contains(&format!("<loc>{origin}{excluded}</loc>")), "{excluded} should not be in the sitemap");
    }

    // Images and hreflang inside the sitemap, other status classes, and
    // per-depth priority / change frequency.
    let mut opts = SitemapOptions {
        include_images: true,
        include_hreflang: true,
        include_3xx: true,
        include_4xx: true,
        change_freq: toad_store::ChangeFreq::ByDepth,
        ..SitemapOptions::default()
    };
    opts.images.include_non_200 = true;
    opts.priority_by_depth = [1.0, 0.8, 0.5, 0.5, 0.5, 0.5];
    opts.change_freq_by_depth[0] = "hourly".into();
    let out = dir.join("sitemap-full");
    let xml = std::fs::read_to_string(&store.generate_sitemap(&out, &opts).unwrap()[0]).unwrap();
    assert!(xml.contains("xmlns:image=") && xml.contains("xmlns:xhtml="));
    assert!(xml.contains(&format!("<image:loc>{origin}/img.png</image:loc>")), "{xml}");
    assert!(xml.contains(&format!(r#"<xhtml:link rel="alternate" hreflang="fr" href="{origin}/fr"/>"#)), "{xml}");
    assert!(xml.contains(&format!("<loc>{origin}/redirect</loc>")) && xml.contains(&format!("<loc>{origin}/missing</loc>")));
    assert!(xml.contains("<changefreq>hourly</changefreq>") && xml.contains("<priority>0.8</priority>"));

    drop(store);
    let _ = std::fs::remove_dir_all(&dir);
}
