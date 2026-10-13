pub const SCHEMA: &str = r#"
PRAGMA journal_mode = WAL;
PRAGMA synchronous = NORMAL;

CREATE TABLE IF NOT EXISTS meta (
    key TEXT PRIMARY KEY,
    value TEXT
);

CREATE TABLE IF NOT EXISTS urls (
    id INTEGER PRIMARY KEY,
    address TEXT NOT NULL UNIQUE,
    crawl_order INTEGER,
    is_internal INTEGER NOT NULL DEFAULT 0,
    depth INTEGER,
    status_code INTEGER NOT NULL DEFAULT 0,
    status TEXT,
    content_type TEXT,
    is_html INTEGER NOT NULL DEFAULT 0,
    indexable INTEGER NOT NULL DEFAULT 0,
    indexability_status TEXT,
    size INTEGER,
    transferred INTEGER,
    response_time REAL,
    http_version TEXT,
    last_modified TEXT,
    redirect_url TEXT,
    redirect_type TEXT,
    hash TEXT,
    blocked_robots_line TEXT,
    error TEXT,
    headers_json TEXT,

    title_1 TEXT, title_1_length INTEGER, title_1_px INTEGER, title_2 TEXT,
    title_count INTEGER NOT NULL DEFAULT 0, title_outside_head INTEGER NOT NULL DEFAULT 0,
    meta_description_1 TEXT, meta_description_1_length INTEGER, meta_description_1_px INTEGER,
    meta_description_2 TEXT, meta_description_count INTEGER NOT NULL DEFAULT 0,
    description_outside_head INTEGER NOT NULL DEFAULT 0,
    meta_keywords_1 TEXT, meta_keywords_count INTEGER NOT NULL DEFAULT 0,
    h1_1 TEXT, h1_1_length INTEGER, h1_2 TEXT, h1_2_length INTEGER,
    h1_count INTEGER NOT NULL DEFAULT 0, h1_alt_text INTEGER NOT NULL DEFAULT 0,
    h1_non_sequential INTEGER NOT NULL DEFAULT 0,
    h2_1 TEXT, h2_1_length INTEGER, h2_2 TEXT, h2_2_length INTEGER,
    h2_count INTEGER NOT NULL DEFAULT 0, h2_non_sequential INTEGER NOT NULL DEFAULT 0,
    meta_robots_1 TEXT, meta_robots_2 TEXT, x_robots_tag_1 TEXT, x_robots_tag_2 TEXT,
    directives TEXT, robots_outside_head INTEGER NOT NULL DEFAULT 0,
    meta_refresh TEXT,
    canonical_1 TEXT, canonical_count INTEGER NOT NULL DEFAULT 0, canonical_distinct INTEGER NOT NULL DEFAULT 0,
    http_canonical TEXT, canonical_outside_head INTEGER NOT NULL DEFAULT 0,
    canonical_relative INTEGER NOT NULL DEFAULT 0, canonical_fragment INTEGER NOT NULL DEFAULT 0,
    canonical_invalid_attr INTEGER NOT NULL DEFAULT 0,
    rel_next TEXT, rel_prev TEXT, rel_next_count INTEGER NOT NULL DEFAULT 0, rel_prev_count INTEGER NOT NULL DEFAULT 0,
    hreflang_count INTEGER NOT NULL DEFAULT 0, hreflang_outside_head INTEGER NOT NULL DEFAULT 0,
    hreflang_invalid INTEGER NOT NULL DEFAULT 0,
    language TEXT, has_viewport INTEGER NOT NULL DEFAULT 0,
    word_count INTEGER, sentence_count INTEGER, avg_words_per_sentence REAL, flesch REAL,
    text_ratio REAL, lorem_ipsum INTEGER NOT NULL DEFAULT 0,
    head_tags INTEGER, body_tags INTEGER,
    hsts TEXT, csp TEXT, x_content_type_options TEXT, x_frame_options TEXT, referrer_policy TEXT,
    form_on_http INTEGER NOT NULL DEFAULT 0, form_action_insecure INTEGER NOT NULL DEFAULT 0,

    url_length INTEGER, folder_depth INTEGER, query_params INTEGER,
    url_non_ascii INTEGER NOT NULL DEFAULT 0, url_underscores INTEGER NOT NULL DEFAULT 0,
    url_uppercase INTEGER NOT NULL DEFAULT 0, url_multiple_slashes INTEGER NOT NULL DEFAULT 0,
    url_repetitive_path INTEGER NOT NULL DEFAULT 0, url_space INTEGER NOT NULL DEFAULT 0,
    url_internal_search INTEGER NOT NULL DEFAULT 0, url_ga_params INTEGER NOT NULL DEFAULT 0,

    -- Filled in by crawl analysis.
    inlinks INTEGER NOT NULL DEFAULT 0, unique_inlinks INTEGER NOT NULL DEFAULT 0,
    outlinks INTEGER NOT NULL DEFAULT 0, unique_outlinks INTEGER NOT NULL DEFAULT 0,
    external_outlinks INTEGER NOT NULL DEFAULT 0, unique_external_outlinks INTEGER NOT NULL DEFAULT 0,
    pct_of_total REAL, link_score REAL,
    redirect_chain_length INTEGER NOT NULL DEFAULT 0, redirect_loop INTEGER NOT NULL DEFAULT 0,
    final_destination TEXT,
    sitemap_url_count INTEGER,
    minhash BLOB,
    spelling_errors INTEGER, grammar_errors INTEGER,
    closest_similarity REAL,
    near_duplicate_count INTEGER NOT NULL DEFAULT 0,
    sd_count INTEGER NOT NULL DEFAULT 0,
    sd_types TEXT,
    sd_formats TEXT,
    sd_errors INTEGER NOT NULL DEFAULT 0,
    sd_warnings INTEGER NOT NULL DEFAULT 0,
    sd_parse_errors INTEGER NOT NULL DEFAULT 0,
    sd_rich_errors INTEGER NOT NULL DEFAULT 0,
    sd_rich_warnings INTEGER NOT NULL DEFAULT 0,
    pdf_title TEXT, pdf_subject TEXT, pdf_author TEXT, pdf_keywords TEXT,
    pdf_created TEXT, pdf_modified TEXT, pdf_pages INTEGER,
    rendered INTEGER NOT NULL DEFAULT 0,
    render_error TEXT,
    orig_title TEXT,
    orig_meta_description TEXT,
    orig_h1 TEXT,
    orig_canonical TEXT,
    orig_directives TEXT,
    orig_word_count INTEGER,
    orig_link_count INTEGER,
    js_link_count INTEGER,
    js_errors INTEGER,
    js_warnings INTEGER,
    blocked_resources INTEGER,
    co2_mg REAL,
    carbon_rating TEXT,
    ajax_fragment_meta INTEGER NOT NULL DEFAULT 0
);

-- Structured data items and validation issues per URL.
-- List mode: the URLs as uploaded, in order and with duplicates
-- (List Mode Export). `address` is the URL as crawled.
CREATE TABLE IF NOT EXISTS list_inputs (
    ord INTEGER PRIMARY KEY,
    url TEXT NOT NULL,
    address TEXT
);

-- Spider > Extraction > Cookies: Set-Cookie headers per URL.
CREATE TABLE IF NOT EXISTS cookies (
    id INTEGER PRIMARY KEY,
    url_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    value TEXT,
    domain TEXT,
    path TEXT,
    expires TEXT,
    secure INTEGER NOT NULL DEFAULT 0,
    http_only INTEGER NOT NULL DEFAULT 0,
    same_site TEXT
);
CREATE INDEX IF NOT EXISTS cookies_url ON cookies(url_id);

-- Content > Spelling & Grammar: one row per error per URL.
CREATE TABLE IF NOT EXISTS language_errors (
    id INTEGER PRIMARY KEY,
    url_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    text TEXT NOT NULL,
    suggestion TEXT,
    rule TEXT,
    message TEXT,
    language TEXT
);
CREATE INDEX IF NOT EXISTS language_errors_url ON language_errors(url_id);

-- Store PDF: deflate-compressed PDF text, one row per URL.
CREATE TABLE IF NOT EXISTS pdf_text (
    url_id INTEGER PRIMARY KEY,
    body BLOB NOT NULL
);

-- Store HTML: deflate-compressed page source, one row per URL.
-- Rendering: JavaScript.
CREATE TABLE IF NOT EXISTS console_log (
    url_id INTEGER NOT NULL,
    ord INTEGER NOT NULL,
    level TEXT NOT NULL,
    source TEXT NOT NULL,
    message TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS screenshots (
    url_id INTEGER PRIMARY KEY,
    png BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS rendered_html (
    url_id INTEGER PRIMARY KEY,
    body BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS blocked_resources (
    url_id INTEGER NOT NULL,
    resource TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS html (
    url_id INTEGER PRIMARY KEY,
    body BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS sd_items (
    url_id INTEGER NOT NULL,
    format TEXT NOT NULL,
    types TEXT,
    properties TEXT
);

CREATE TABLE IF NOT EXISTS sd_issues (
    url_id INTEGER NOT NULL,
    item_type TEXT,
    severity TEXT NOT NULL,
    property TEXT,
    message TEXT,
    source TEXT NOT NULL DEFAULT 'Google'
);

-- Page URLs listed in each crawled XML sitemap.
CREATE TABLE IF NOT EXISTS sitemap_entries (
    sitemap TEXT NOT NULL,
    url TEXT NOT NULL
);

-- Custom Search counts (kind 'search') and Custom Extraction values
-- (kind 'extract'); idx is the rule's position in the configuration.
CREATE TABLE IF NOT EXISTS custom_values (
    url_id INTEGER NOT NULL,
    kind TEXT NOT NULL,
    idx INTEGER NOT NULL,
    ord INTEGER NOT NULL,
    value TEXT
);

CREATE TABLE IF NOT EXISTS near_duplicates (
    a_id INTEGER NOT NULL,
    b_id INTEGER NOT NULL,
    similarity REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS links (
    id INTEGER PRIMARY KEY,
    source_id INTEGER NOT NULL,
    target TEXT NOT NULL,
    target_id INTEGER,
    internal INTEGER NOT NULL DEFAULT 0,
    kind TEXT NOT NULL,
    anchor TEXT,
    alt TEXT,
    follow INTEGER NOT NULL DEFAULT 1,
    rel TEXT,
    target_attr TEXT,
    path_type TEXT,
    link_path TEXT,
    position TEXT,
    hreflang TEXT,
    origin TEXT
);

CREATE INDEX IF NOT EXISTS links_source ON links(source_id);
CREATE INDEX IF NOT EXISTS links_target ON links(target);
CREATE INDEX IF NOT EXISTS links_target_id ON links(target_id);
CREATE INDEX IF NOT EXISTS urls_internal_html ON urls(is_internal, is_html, status_code);
CREATE INDEX IF NOT EXISTS sitemap_entries_url ON sitemap_entries(url);
CREATE INDEX IF NOT EXISTS custom_values_url ON custom_values(url_id, kind, idx);
CREATE INDEX IF NOT EXISTS near_duplicates_a ON near_duplicates(a_id);
CREATE INDEX IF NOT EXISTS sd_items_url ON sd_items(url_id);
CREATE INDEX IF NOT EXISTS sd_issues_url ON sd_issues(url_id);
CREATE INDEX IF NOT EXISTS console_log_url ON console_log(url_id);
CREATE INDEX IF NOT EXISTS blocked_resources_url ON blocked_resources(url_id);
"#;

/// Columns added to `urls` after the first release, as (name, type). Older
/// crawl files get them when opened, so they can be resumed and compared.
pub const ADDED_URL_COLUMNS: &[(&str, &str)] = &[
    ("spelling_errors", "INTEGER"),
    ("grammar_errors", "INTEGER"),
    ("sd_rich_errors", "INTEGER NOT NULL DEFAULT 0"),
    ("sd_rich_warnings", "INTEGER NOT NULL DEFAULT 0"),
    ("pdf_title", "TEXT"),
    ("pdf_subject", "TEXT"),
    ("pdf_author", "TEXT"),
    ("pdf_keywords", "TEXT"),
    ("pdf_created", "TEXT"),
    ("pdf_modified", "TEXT"),
    ("pdf_pages", "INTEGER"),
    ("rendered", "INTEGER NOT NULL DEFAULT 0"),
    ("render_error", "TEXT"),
    ("orig_title", "TEXT"),
    ("orig_meta_description", "TEXT"),
    ("orig_h1", "TEXT"),
    ("orig_canonical", "TEXT"),
    ("orig_directives", "TEXT"),
    ("orig_word_count", "INTEGER"),
    ("orig_link_count", "INTEGER"),
    ("js_link_count", "INTEGER"),
    ("js_errors", "INTEGER"),
    ("js_warnings", "INTEGER"),
    ("blocked_resources", "INTEGER"),
    ("co2_mg", "REAL"),
    ("carbon_rating", "TEXT"),
    ("ajax_fragment_meta", "INTEGER NOT NULL DEFAULT 0"),
];

/// Columns added to other tables: (table, column, type).
pub const ADDED_COLUMNS: &[(&str, &str, &str)] = &[("sd_issues", "source", "TEXT NOT NULL DEFAULT 'Google'"), ("links", "origin", "TEXT")];

fn columns_of(conn: &rusqlite::Connection, table: &str) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

pub fn migrate(conn: &rusqlite::Connection) -> rusqlite::Result<()> {
    let existing = columns_of(conn, "urls")?;
    for (name, ty) in ADDED_URL_COLUMNS {
        if !existing.iter().any(|c| c == name) {
            conn.execute_batch(&format!("ALTER TABLE urls ADD COLUMN {name} {ty}"))?;
        }
    }
    for (table, name, ty) in ADDED_COLUMNS {
        if !columns_of(conn, table)?.iter().any(|c| c == name) {
            conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {name} {ty}"))?;
        }
    }
    Ok(())
}
