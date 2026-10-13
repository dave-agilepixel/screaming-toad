//! `toad`: command-line interface for Screaming Toad.

use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;
use toad_core::config::{CustomExtractor, CustomSearch, ExtractorKind, QueryKind, RobotsMode, SearchMode};
use toad_core::{CrawlConfig, CrawlControl, CrawlEvent, Crawler, PageRecord};
use toad_store::{ChangeElement, ChangeFreq, CompareConfig, CompareState, Comparison, LastMod, RowQuery, SitemapOptions, Store, UrlMapping};
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
    /// List the crawls in a folder (File > Crawls), newest first.
    Crawls {
        /// Folder to list (default: the current folder).
        #[arg(default_value = ".")]
        dir: PathBuf,
    },
    /// Continue a stopped or interrupted crawl.
    Resume {
        crawl: PathBuf,
        /// Raise the URL limit the crawl was started with.
        #[arg(long)]
        max_urls: Option<u64>,
        #[arg(long)]
        threads: Option<usize>,
        #[command(flatten)]
        net: NetworkArgs,
    },
    /// Compare two crawls (Compare mode) and save the comparison.
    Compare(CompareArgs),
    /// Show a saved comparison: overview, issues or Change Detection.
    CompareShow {
        comparison: PathBuf,
        /// Show the Issues panel with deltas instead of the Overview.
        #[arg(long)]
        issues: bool,
        /// Show only the Change Detection tab.
        #[arg(long)]
        change_detection: bool,
        /// Write the Overview (Reports > Crawl Overview) to CSV instead of printing.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Export one filter in one compare state from a saved comparison.
    CompareExport {
        comparison: PathBuf,
        #[arg(long)]
        tab: String,
        #[arg(long, default_value = "All")]
        filter: String,
        /// Current, Previous, Added, New, Removed or Missing.
        #[arg(long, default_value = "Current")]
        state: String,
        #[arg(long)]
        search: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
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
struct CompareArgs {
    /// The current (newer) crawl.
    current: PathBuf,
    /// The previous (older) crawl.
    previous: PathBuf,
    /// Comparison file to write.
    #[arg(long, short, default_value = "comparison.toadcmp")]
    out: PathBuf,
    /// Pick current and previous by file date instead of argument order.
    #[arg(long)]
    by_date: bool,
    /// URL Mapping for previous-crawl URLs, "REGEX=>REPLACEMENT" ($1 groups
    /// allowed). Repeatable; applied in order.
    #[arg(long)]
    map: Vec<String>,
    /// Change Detection elements to compare, comma-separated (default: all).
    #[arg(long, value_delimiter = ',')]
    elements: Vec<String>,
    /// Content: flag pages whose content changed by more than this %.
    #[arg(long, default_value_t = 10.0)]
    similarity_change: f64,
    /// Ignore non-indexable URLs for Change Detection.
    #[arg(long)]
    ignore_non_indexable: bool,
}

/// Authentication, proxy and trusted certificates.
#[derive(Args, Clone)]
struct NetworkArgs {
    /// Encrypted authentication file exported from Configuration >
    /// Authentication. Its password is read from TOAD_AUTH_PASSWORD.
    #[arg(long)]
    auth_config: Option<PathBuf>,
    /// Proxy as scheme://[user:pass@]host:port (http, https or socks5).
    #[arg(long)]
    proxy: Option<String>,
    /// Hosts that bypass the proxy.
    #[arg(long, value_delimiter = ',')]
    no_proxy: Vec<String>,
    /// A PEM or DER certificate to trust (repeatable).
    #[arg(long = "trusted-cert")]
    trusted_certs: Vec<String>,
}

impl NetworkArgs {
    fn network(&self) -> anyhow::Result<std::sync::Arc<toad_core::auth::Network>> {
        use toad_core::auth::{AuthConfig, Network, NetworkSettings, ProxyConfig};
        let auth = match &self.auth_config {
            Some(path) => {
                let password = std::env::var("TOAD_AUTH_PASSWORD")
                    .map_err(|_| anyhow::anyhow!("set TOAD_AUTH_PASSWORD to the password of {}", path.display()))?;
                let text = std::fs::read_to_string(path)?;
                toad_core::auth::import_encrypted(&text, &password)?
            }
            None => AuthConfig::default(),
        };
        let mut proxy = match &self.proxy {
            Some(p) => ProxyConfig::parse(p)?,
            None => ProxyConfig::default(),
        };
        proxy.no_proxy = self.no_proxy.clone();
        let settings = NetworkSettings { proxy, trusted_certificates: self.trusted_certs.clone() };
        // No prompts on the command line: unknown realms stay 401.
        Ok(Network::new(settings, AuthConfig { prompt: false, ..auth }, None).shared())
    }
}

#[derive(Args)]
struct CrawlArgs {
    /// Start URL (Spider mode).
    url: Option<String>,
    #[command(flatten)]
    net: NetworkArgs,
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
    /// Store HTML (View Source, page-source export, Compare > Content).
    #[arg(long)]
    store_html: bool,
    /// Read PDF text (word count, Custom Search, spelling on PDFs).
    #[arg(long)]
    store_pdf: bool,
    /// PDF Subject, Author, dates and page count columns.
    #[arg(long)]
    pdf_properties: bool,
    /// Follow and report links inside PDFs.
    #[arg(long)]
    pdf_links: bool,
    /// Spell check page text. Dictionaries are downloaded to ~/.toad/dictionaries
    /// on first use; words in ~/.toad/dictionary.txt are always accepted.
    #[arg(long)]
    spelling: bool,
    /// Grammar check with this LanguageTool server (e.g. http://localhost:8081).
    #[arg(long)]
    grammar_server: Option<String>,
    /// Spelling/grammar language (e.g. en-GB). With it, every page is checked
    /// in this language (Manual); without, each page's lang attribute is used.
    #[arg(long)]
    language: Option<String>,
    /// Custom search, "Name=text" (case-insensitive). Prefix the text with
    /// "!" for Does Not Contain. Repeatable.
    #[arg(long)]
    search: Vec<String>,
    /// Custom search with a regex, "Name=pattern". Repeatable.
    #[arg(long)]
    search_regex: Vec<String>,
    /// Custom extraction: "Name=xpath://h3", "Name=xpath:count(//a)",
    /// "Name=css:SELECTOR", "Name=css:SELECTOR@attribute"
    /// or "Name=regex:PATTERN". Repeatable.
    #[arg(long)]
    extract: Vec<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Crawl(args) => crawl(args).await,
        Command::Issues { crawl, json } => issues(&crawl, json),
        Command::Compare(args) => compare(args),
        Command::Crawls { dir } => list_crawls(&dir),
        Command::Resume { crawl, max_urls, threads, net } => resume(&crawl, max_urls, threads, &net).await,
        Command::CompareShow { comparison, issues, change_detection, out } => {
            let cmp = Comparison::open(&comparison)?;
            if let Some(path) = out {
                let table = cmp.overview_table()?;
                toad_store::write_csv(&table, &path)?;
                println!("Wrote {} rows to {}", table.rows.len(), path.display());
            } else if issues {
                print_compare_issues(&cmp, usize::MAX)?;
            } else {
                print_compare_overview(&cmp, change_detection)?;
            }
            Ok(())
        }
        Command::CompareExport { comparison, tab, filter, state, search, out } => {
            let cmp = Comparison::open(&comparison)?;
            let state = CompareState::from_name(&state)
                .ok_or_else(|| anyhow::anyhow!("--state must be Current, Previous, Added, New, Removed or Missing"))?;
            let n = cmp.export_csv(&RowQuery { tab, filter, search, ..Default::default() }, state, &out)?;
            println!("Wrote {n} rows to {}", out.display());
            Ok(())
        }
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
        } else if let Some(xp) = spec.strip_prefix("xpath:") {
            // count(), string-length() and other functions return a single value.
            let function = !xp.trim_start().starts_with('/') && !xp.trim_start().starts_with('(');
            let extract = if function { toad_core::config::ExtractWhat::FunctionValue } else { toad_core::config::ExtractWhat::InnerHtml };
            CustomExtractor { name, kind: ExtractorKind::Xpath, expression: xp.to_string(), extract, ..Default::default() }
        } else {
            bail!("--extract expects Name=xpath:EXPR, Name=css:SELECTOR or Name=regex:PATTERN, got '{x}'");
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

    let mut list_inputs: Option<Vec<String>> = None;
    let seeds: Vec<Url> = match (&args.url, &args.list) {
        (Some(u), None) => vec![parse_start_url(u)?],
        (None, Some(path)) => {
            cfg = cfg.for_list_mode();
            let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
            let inputs: Vec<String> = text
                .lines()
                .map(str::trim)
                .filter(|l| l.starts_with("http://") || l.starts_with("https://"))
                .map(str::to_string)
                .collect();
            let urls: Vec<Url> = inputs.iter().filter_map(|l| Url::parse(l).ok()).collect();
            if urls.is_empty() {
                bail!("no http(s) URLs found in {}", path.display());
            }
            list_inputs = Some(inputs);
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
    if args.store_html { cfg.store_html = true; }
    if args.spelling { cfg.spelling.spell_check = true; }
    if args.store_pdf { cfg.store_pdf = true; }
    if args.pdf_properties { cfg.extract_pdf_properties = true; }
    if args.pdf_links { cfg.extract_pdf_links = true; }
    if let Some(server) = &args.grammar_server {
        cfg.spelling.grammar_check = true;
        cfg.spelling.grammar_server = server.clone();
    }
    if let Some(lang) = &args.language {
        cfg.spelling.language = lang.clone();
        cfg.spelling.language_mode = toad_core::config::LanguageMode::Manual;
    }
    if cfg.spelling.spell_check || cfg.spelling.grammar_check {
        let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
        cfg.spelling.dictionaries_dir = Some(home.join(".toad").join("dictionaries"));
        cfg.spelling.dictionary_words = std::fs::read_to_string(home.join(".toad").join("dictionary.txt"))
            .map(|t| t.lines().map(str::trim).filter(|l| !l.is_empty()).map(str::to_string).collect())
            .unwrap_or_default();
    }
    custom_rules(&args, &mut cfg)?;
    cfg.include.extend(args.include.iter().cloned());
    cfg.exclude.extend(args.exclude.iter().cloned());

    let start = seeds[0].clone();
    let mut store = Store::create(&args.out, &start, &cfg)?;
    store.mark_started(start.host_str().unwrap_or("crawl"), if list_inputs.is_some() { "list" } else { "spider" })?;
    if let Some(inputs) = &list_inputs {
        store.set_list_inputs(inputs)?;
    }
    let crawler = Crawler::new(cfg, &start)?.with_network(args.net.network()?)?;
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    let handle = tokio::spawn(crawler.run(seeds, tx, CrawlControl::default()));
    println!("Crawling {start} → {}", args.out.display());
    drive(&mut store, rx, handle).await?;

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

fn compare(args: CompareArgs) -> anyhow::Result<()> {
    let (mut current, mut previous) = (args.current, args.previous);
    if args.by_date {
        let modified = |p: &Path| std::fs::metadata(p).and_then(|m| m.modified()).with_context(|| format!("reading {}", p.display()));
        if modified(&current)? < modified(&previous)? {
            std::mem::swap(&mut current, &mut previous);
        }
    }
    let url_mapping = args
        .map
        .iter()
        .map(|m| match m.split_once("=>") {
            Some((regex, replace)) => Ok(UrlMapping { regex: regex.to_string(), replace: replace.to_string() }),
            None => bail!("--map expects REGEX=>REPLACEMENT, got '{m}'"),
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let elements = if args.elements.is_empty() {
        ChangeElement::ALL.to_vec()
    } else {
        args.elements
            .iter()
            .map(|e| ChangeElement::from_name(e.trim()).ok_or_else(|| anyhow::anyhow!("unknown Change Detection element '{e}'")))
            .collect::<anyhow::Result<Vec<_>>>()?
    };
    let cfg = CompareConfig {
        elements,
        similarity_change_pct: args.similarity_change,
        ignore_non_indexable: args.ignore_non_indexable,
        url_mapping,
    };
    let started = Instant::now();
    let cmp = Comparison::build(&current, &previous, &args.out, &cfg)?;
    let s = cmp.summary()?;
    println!("Current:  {} ({} URLs)", current.display(), s.current_urls);
    println!("Previous: {} ({} URLs)", previous.display(), s.previous_urls);
    println!("URLs in both crawls: {}", s.in_both);
    if !s.content_available && cfg.elements.contains(&ChangeElement::Content) {
        println!("Note: Content change detection needs Near Duplicates (content signatures) in both crawls.");
    }
    println!("Compared in {:.1}s; saved to {}\n", started.elapsed().as_secs_f64(), args.out.display());
    print_compare_issues(&cmp, 25)?;
    println!();
    print_compare_overview(&cmp, true)
}

fn signed(n: i64) -> String {
    if n > 0 { format!("+{n}") } else { n.to_string() }
}

fn print_compare_overview(cmp: &Comparison, change_detection_only: bool) -> anyhow::Result<()> {
    println!(
        "{:<55} {:>8} {:>8} {:>7} {:>8} {:>6} {:>6} {:>8} {:>8}",
        "", "Current", "Previous", "Change", "Change%", "Added", "New", "Removed", "Missing"
    );
    let dash = |v: Option<u64>| v.map(|n| n.to_string()).unwrap_or_else(|| "-".into());
    let mut last_tab = String::new();
    for r in cmp.overview()? {
        if change_detection_only && r.tab != "Change Detection" {
            continue;
        }
        if r.tab != last_tab {
            println!("{}", r.tab);
            last_tab = r.tab.clone();
        }
        println!(
            "  {:<53} {:>8} {:>8} {:>7} {:>8} {:>6} {:>6} {:>8} {:>8}",
            r.filter,
            r.current,
            r.previous,
            signed(r.change),
            r.change_pct.map(|p| format!("{p:.2}%")).unwrap_or_default(),
            dash(r.added),
            dash(r.new),
            dash(r.removed),
            dash(r.missing)
        );
    }
    Ok(())
}

fn print_compare_issues(cmp: &Comparison, limit: usize) -> anyhow::Result<()> {
    let issues = cmp.issues()?;
    println!("{:<12} {:<7} {:>8} {:>8} {:>7}  Issue", "Type", "Prio", "Current", "Previous", "Change");
    for i in issues.iter().take(limit) {
        println!(
            "{:<12} {:<7} {:>8} {:>8} {:>7}  {}: {}",
            format!("{:?}", i.issue_type),
            format!("{:?}", i.priority),
            i.current,
            i.previous,
            signed(i.change),
            i.tab,
            i.filter
        );
    }
    if issues.len() > limit {
        println!("… and {} more (toad compare-show <comparison> --issues)", issues.len() - limit);
    }
    Ok(())
}

/// Write a crawl's events to the store, show progress, then record how it
/// ended and run crawl analysis.
async fn drive(
    store: &mut Store,
    mut rx: tokio::sync::mpsc::Receiver<CrawlEvent>,
    handle: tokio::task::JoinHandle<anyhow::Result<toad_core::Progress>>,
) -> anyhow::Result<toad_core::Progress> {
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
    println!("Crawled {} URLs in {:.1}s.", progress.crawled, started.elapsed().as_secs_f64());
    store.mark_finished(progress.queued)?;
    if progress.queued > 0 {
        println!("Stopped with {} URLs still queued; continue with `toad resume`.", progress.queued);
    }
    if store.config().crawl_analysis.auto_analyse {
        println!("Running crawl analysis…");
    }
    store.finish_crawl_analysis()?;
    Ok(progress)
}

async fn resume(path: &Path, max_urls: Option<u64>, threads: Option<usize>, net: &NetworkArgs) -> anyhow::Result<()> {
    let mut store = Store::open(path)?;
    let start = store
        .meta("start_url")?
        .and_then(|u| Url::parse(&u).ok())
        .ok_or_else(|| anyhow::anyhow!("{} has no start URL", path.display()))?;
    if store.status()? == toad_store::CrawlStatus::Complete {
        bail!("{} is already complete", path.display());
    }
    let mut cfg = store.config().clone();
    if let Some(v) = max_urls { cfg.max_urls = v; }
    if let Some(v) = threads { cfg.threads = v; }
    let (crawled, pending) = store.resume_state()?;
    println!("Resuming {start}: {} URLs crawled, {} links to follow up", crawled.len(), pending.len());
    let mode = store.meta("mode")?.unwrap_or_else(|| "spider".into());
    store.mark_started("", &mode)?;
    let crawler = Crawler::new(cfg, &start)?.with_network(net.network()?)?;
    let (tx, rx) = tokio::sync::mpsc::channel(1024);
    let handle = tokio::spawn(crawler.resume(crawled, pending, tx, CrawlControl::default()));
    drive(&mut store, rx, handle).await?;
    print_issues(&store, 15)
}

fn list_crawls(dir: &Path) -> anyhow::Result<()> {
    let entries = toad_store::crawls::list(dir);
    if entries.is_empty() {
        println!("No crawls in {}", dir.display());
        return Ok(());
    }
    println!("{:<30} {:<10} {:<8} {:>8} {:>7}  {:<20} Path", "Name", "Status", "Mode", "URLs", "Done", "Project");
    for e in entries {
        println!(
            "{:<30} {:<10} {:<8} {:>8} {:>6.1}%  {:<20} {}{}",
            e.name.chars().take(30).collect::<String>(),
            format!("{:?}", e.status),
            e.mode,
            e.urls,
            e.pct_complete,
            e.project.chars().take(20).collect::<String>(),
            e.path,
            if e.locked { "  [locked]" } else { "" }
        );
    }
    Ok(())
}
