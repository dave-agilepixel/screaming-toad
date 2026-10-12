//! `toad`: command-line interface for Screaming Toad.

use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use toad_core::config::{CustomExtractor, CustomSearch, ExtractorKind, QueryKind, RobotsMode, SearchMode};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord};
use toad_store::{ChangeFreq, LastMod, RowQuery, SitemapOptions, Store};
use url::Url;

#[derive(Parser)]
#[command(name = "toad", version, about = "Screaming Toad SEO Spider")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Crawl a site (Spider mode) or a list of URLs (List mode).
    Crawl(CrawlArgs),
    /// Show the Issues panel for a saved crawl.
    Issues {
        crawl: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Show every tab and filter with its URL count.
    Overview { crawl: PathBuf },
    /// List the tabs, filters and columns available.
    Tabs { crawl: PathBuf },
    /// Export a tab and filter to CSV.
    Export {
        crawl: PathBuf,
        #[arg(long, default_value = "Internal")]
        tab: String,
        #[arg(long, default_value = "All")]
        filter: String,
        #[arg(long)]
        search: Option<String>,
        #[arg(long, short)]
        out: PathBuf,
    },
    /// Show inlinks or outlinks for one URL.
    Links {
        crawl: PathBuf,
        url: String,
        #[arg(long)]
        outlinks: bool,
    },
    /// Re-run crawl analysis on a saved crawl.
    Analyse { crawl: PathBuf },
    /// Write the default configuration to a file, to edit and load with --config.
    Config { out: PathBuf },
    /// Generate an XML sitemap from a saved crawl.
    Sitemap(SitemapArgs),
    /// Run a report or bulk export (Reports / Bulk Export menus).
    Report {
        crawl: PathBuf,
        /// Report name, e.g. "Redirect Chains" or "All Inlinks".
        name: Option<String>,
        /// List the available reports.
        #[arg(long)]
        list: bool,
        /// Write CSV here instead of printing.
        #[arg(long, short)]
        out: Option<PathBuf>,
    },
}

#[derive(Args)]
struct SitemapArgs {
    crawl: PathBuf,
    /// Folder to write sitemap.xml (or sitemap1.xml… and sitemap_index.xml) into.
    #[arg(long, default_value = ".")]
    out_dir: PathBuf,
    #[arg(long)]
    include_noindex: bool,
    #[arg(long)]
    include_canonicalised: bool,
    #[arg(long)]
    include_paginated: bool,
    #[arg(long)]
    include_pdfs: bool,
    /// "server" (Last-Modified header) or a date such as 2026-10-12.
    #[arg(long)]
    lastmod: Option<String>,
    /// Add <priority> by crawl depth.
    #[arg(long)]
    priority: bool,
    /// "lastmod" or "depth".
    #[arg(long)]
    changefreq: Option<String>,
}

#[derive(Args)]
struct CrawlArgs {
    /// Start URL (Spider mode).
    url: Option<String>,
    /// File of URLs, one per line (List mode).
    #[arg(long, conflicts_with = "url")]
    list: Option<PathBuf>,
    /// Crawl file to write.
    #[arg(long, short, default_value = "crawl.toad")]
    out: PathBuf,
    /// Configuration profile (.toadconfig JSON).
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    max_urls: Option<u64>,
    #[arg(long)]
    max_depth: Option<u32>,
    #[arg(long)]
    threads: Option<usize>,
    /// Maximum URLs per second.
    #[arg(long)]
    rate: Option<f64>,
    #[arg(long)]
    user_agent: Option<String>,
    #[arg(long)]
    ignore_robots: bool,
    /// Regex; only matching internal URLs are crawled. Repeatable.
    #[arg(long)]
    include: Vec<String>,
    /// Regex; matching URLs are never requested. Repeatable.
    #[arg(long)]
    exclude: Vec<String>,
    /// Crawl every subdomain of the start domain.
    #[arg(long)]
    all_subdomains: bool,
    /// Exports after the crawl, e.g. "Internal:All,Response Codes:Internal Client Error (4XX)".
    #[arg(long)]
    export_tabs: Option<String>,
    /// Folder for exports.
    #[arg(long, default_value = "exports")]
    output_folder: PathBuf,
    /// Crawl XML sitemaps found in robots.txt (and any given with --sitemap-url).
    #[arg(long)]
    sitemaps: bool,
    /// A sitemap to crawl. Repeatable; implies --sitemaps.
    #[arg(long)]
    sitemap_url: Vec<String>,
    /// Detect near-duplicate content (MinHash, 90% by default).
    #[arg(long)]
    near_duplicates: bool,
    /// Custom search, "Name=text" (case-insensitive). Prefix the text with
    /// "!" for Does Not Contain. Repeatable.
    #[arg(long)]
    search: Vec<String>,
    /// Custom search with a regex, "Name=pattern". Repeatable.
    #[arg(long)]
    search_regex: Vec<String>,
    /// Custom extraction: "Name=css:SELECTOR", "Name=css:SELECTOR@attribute"
    /// or "Name=regex:PATTERN". Repeatable.
    #[arg(long)]
    extract: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Crawl(args) => crawl(args).await,
        Command::Issues { crawl, json } => issues(&crawl, json),
        Command::Overview { crawl } => {
            let store = Store::open(&crawl)?;
            let mut last_tab = String::new();
            for row in store.overview()? {
                if row.tab != last_tab {
                    println!("\n{}", row.tab);
                    last_tab = row.tab.clone();
                }
                println!("  {:<55} {:>8}", row.filter, row.urls);
            }
            Ok(())
        }
        Command::Tabs { crawl } => {
            let store = Store::open(&crawl)?;
            for tab in store.tabs() {
                println!("{}", tab.name);
                println!("  columns: {}", tab.columns.join(", "));
                for f in &tab.filters {
                    match f.issue {
                        Some((t, p)) => println!("  - {} [{t:?} / {p:?}]", f.name),
                        None => println!("  - {}", f.name),
                    }
                }
            }
            Ok(())
        }
        Command::Export { crawl, tab, filter, search, out } => {
            let store = Store::open(&crawl)?;
            let n = store.export_csv(&RowQuery { tab, filter, search, ..Default::default() }, &out)?;
            println!("Wrote {n} rows to {}", out.display());
            Ok(())
        }
        Command::Links { crawl, url, outlinks } => {
            let store = Store::open(&crawl)?;
            let table = if outlinks { store.outlinks(&url)? } else { store.inlinks(&url)? };
            println!("{}", table.columns.join("\t"));
            for row in table.rows {
                println!("{}", row.join("\t"));
            }
            Ok(())
        }
        Command::Analyse { crawl } => {
            let mut store = Store::open(&crawl)?;
            store.analyse()?;
            println!("Crawl analysis complete.");
            Ok(())
        }
        Command::Config { out } => {
            CrawlConfig::default().save(&out)?;
            println!("Wrote default configuration to {}", out.display());
            Ok(())
        }
        Command::Report { crawl, name, list, out } => {
            let store = Store::open(&crawl)?;
            let Some(name) = name.filter(|_| !list) else {
                for r in store.reports() {
                    println!("{:<48} {:?}  {}", r.name, r.kind, r.description);
                }
                return Ok(());
            };
            let table = store.report(&name)?;
            match out {
                Some(path) => {
                    toad_store::write_csv(&table, &path)?;
                    println!("Wrote {} rows to {}", table.rows.len(), path.display());
                }
                None => {
                    println!("{}", table.columns.join("\t"));
                    for row in table.rows {
                        println!("{}", row.join("\t"));
                    }
                }
            }
            Ok(())
        }
        Command::Sitemap(args) => {
            let store = Store::open(&args.crawl)?;
            let opts = SitemapOptions {
                include_noindex: args.include_noindex,
                include_canonicalised: args.include_canonicalised,
                include_paginated: args.include_paginated,
                include_pdfs: args.include_pdfs,
                last_mod: match args.lastmod.as_deref() {
                    None => LastMod::None,
                    Some("server") => LastMod::ServerResponse,
                    Some(date) => LastMod::Custom(date.to_string()),
                },
                priority: args.priority,
                change_freq: match args.changefreq.as_deref() {
                    None => ChangeFreq::None,
                    Some("lastmod") => ChangeFreq::FromLastModified,
                    Some("depth") => ChangeFreq::ByDepth,
                    Some(other) => bail!("--changefreq must be 'lastmod' or 'depth', not '{other}'"),
                },
                ..Default::default()
            };
            for path in store.generate_sitemap(&args.out_dir, &opts)? {
                println!("Wrote {}", path.display());
            }
            Ok(())
        }
    }
}

/// "Name=value" → (Name, value).
fn name_value(arg: &str, flag: &str) -> anyhow::Result<(String, String)> {
    match arg.split_once('=') {
        Some((n, v)) if !n.trim().is_empty() && !v.is_empty() => Ok((n.trim().to_string(), v.to_string())),
        _ => bail!("{flag} expects Name=value, got '{arg}'"),
    }
}

fn custom_rules(args: &CrawlArgs, cfg: &mut CrawlConfig) -> anyhow::Result<()> {
    for s in &args.search {
        let (name, query) = name_value(s, "--search")?;
        let (mode, query) = match query.strip_prefix('!') {
            Some(q) => (SearchMode::DoesNotContain, q.to_string()),
            None => (SearchMode::Contains, query),
        };
        cfg.custom_search.push(CustomSearch { name, mode, query, ..Default::default() });
    }
    for s in &args.search_regex {
        let (name, query) = name_value(s, "--search-regex")?;
        cfg.custom_search.push(CustomSearch { name, kind: QueryKind::Regex, query, ..Default::default() });
    }
    for x in &args.extract {
        let (name, spec) = name_value(x, "--extract")?;
        let extractor = if let Some(css) = spec.strip_prefix("css:") {
            let (selector, attribute) = match css.rsplit_once('@') {
                Some((sel, attr)) if !attr.contains(' ') && !attr.contains(']') => (sel.to_string(), Some(attr.to_string())),
                _ => (css.to_string(), None),
            };
            CustomExtractor { name, kind: ExtractorKind::Css, expression: selector, attribute, ..Default::default() }
        } else if let Some(re) = spec.strip_prefix("regex:") {
            CustomExtractor { name, kind: ExtractorKind::Regex, expression: re.to_string(), ..Default::default() }
        } else {
            bail!("--extract expects Name=css:SELECTOR or Name=regex:PATTERN, got '{x}'");
        };
        cfg.custom_extraction.push(extractor);
    }
    Ok(())
}

async fn crawl(args: CrawlArgs) -> anyhow::Result<()> {
    let mut cfg = match &args.config {
        Some(p) => CrawlConfig::load(p).with_context(|| format!("loading {}", p.display()))?,
        None => CrawlConfig::default(),
    };

    let seeds: Vec<Url> = match (&args.url, &args.list) {
        (Some(u), None) => vec![parse_start_url(u)?],
        (None, Some(path)) => {
            cfg = cfg.for_list_mode();
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let urls: Vec<Url> = text
                .lines()
                .map(str::trim)
                .filter(|l| l.starts_with("http://") || l.starts_with("https://"))
                .filter_map(|l| Url::parse(l).ok())
                .collect();
            if urls.is_empty() {
                bail!("no http(s) URLs found in {}", path.display());
            }
            urls
        }
        _ => bail!("give a start URL, or --list <file>"),
    };

    if let Some(v) = args.max_urls { cfg.max_urls = v; }
    if let Some(v) = args.max_depth { cfg.max_depth = Some(v); }
    if let Some(v) = args.threads { cfg.threads = v; }
    if let Some(v) = args.rate { cfg.max_urls_per_sec = Some(v); }
    if let Some(v) = args.user_agent.clone() { cfg.user_agent = v; }
    if args.ignore_robots { cfg.robots_mode = RobotsMode::Ignore; }
    if args.all_subdomains { cfg.crawl_all_subdomains = true; }
    if args.sitemaps || !args.sitemap_url.is_empty() { cfg.crawl_linked_sitemaps = true; }
    cfg.sitemap_urls.extend(args.sitemap_url.iter().cloned());
    if args.near_duplicates { cfg.near_duplicates = true; }
    custom_rules(&args, &mut cfg)?;
    cfg.include.extend(args.include.iter().cloned());
    cfg.exclude.extend(args.exclude.iter().cloned());

    let start = seeds[0].clone();
    let mut store = Store::create(&args.out, &start, &cfg)?;
    let crawler = Crawler::new(cfg, &start)?;
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    let control = CrawlControl::default();
    let handle = tokio::spawn(crawler.run(seeds, tx, control));

    println!("Crawling {start} → {}", args.out.display());
    let started = Instant::now();
    let mut batch: Vec<PageRecord> = Vec::with_capacity(200);
    while let Some(event) = rx.recv().await {
        match event {
            CrawlEvent::Page(rec) => {
                batch.push(*rec);
                if batch.len() >= 200 {
                    store.insert_batch(&batch)?;
                    batch.clear();
                }
            }
            CrawlEvent::Sitemap { sitemap, urls } => {
                store.insert_batch(&batch)?;
                batch.clear();
                store.insert_sitemap(&sitemap, &urls)?;
            }
            CrawlEvent::Progress(p) => {
                eprint!("\r  crawled {:>7}  queued {:>7}  {:>6.1} URL/s   ", p.crawled, p.queued, p.crawled as f64 / started.elapsed().as_secs_f64().max(0.001));
            }
            CrawlEvent::Finished(_) => break,
        }
    }
    store.insert_batch(&batch)?;
    let progress = handle.await??;
    eprintln!();
    println!("Crawled {} URLs in {:.1}s. Running crawl analysis…", progress.crawled, started.elapsed().as_secs_f64());
    store.analyse()?;
    store.set_meta("finished", &progress.crawled.to_string())?;

    print_issues(&store, 15)?;

    if let Some(spec) = &args.export_tabs {
        std::fs::create_dir_all(&args.output_folder)?;
        for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            let (tab, filter) = item.split_once(':').unwrap_or((item, "All"));
            let file = args.output_folder.join(format!("{}.csv", slug(&format!("{tab} {filter}"))));
            let n = store.export_csv(&RowQuery { tab: tab.into(), filter: filter.into(), ..Default::default() }, &file)?;
            println!("Exported {tab} › {filter}: {n} rows → {}", file.display());
        }
    }
    Ok(())
}

fn parse_start_url(input: &str) -> anyhow::Result<Url> {
    let with_scheme = if input.contains("://") { input.to_string() } else { format!("https://{input}") };
    let url = Url::parse(&with_scheme).with_context(|| format!("invalid URL: {input}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        bail!("only http and https URLs can be crawled");
    }
    Ok(url)
}

fn issues(crawl: &Path, json: bool) -> anyhow::Result<()> {
    let store = Store::open(crawl)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&store.issues()?)?);
        return Ok(());
    }
    print_issues(&store, usize::MAX)
}

fn print_issues(store: &Store, limit: usize) -> anyhow::Result<()> {
    let issues = store.issues()?;
    println!("\n{:<12} {:<7} {:>7} {:>7}  Issue", "Type", "Priority", "URLs", "%");
    for i in issues.iter().take(limit) {
        println!(
            "{:<12} {:<7} {:>7} {:>6.1}%  {}: {}",
            format!("{:?}", i.issue_type),
            format!("{:?}", i.priority),
            i.urls,
            i.pct_of_total,
            i.tab,
            i.filter
        );
    }
    if issues.len() > limit {
        println!("… and {} more (toad issues <crawl>)", issues.len() - limit);
    }
    Ok(())
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}
