//! MCP server (Model Context Protocol): read-only tools over saved crawls,
//! so an AI assistant can list crawls, read issues, query any tab and
//! filter, look up URLs and links, run reports and compare two crawls.
//!
//! Transports: STDIO (`toad mcp`, newline-delimited JSON-RPC) and
//! Streamable HTTP (`POST /mcp` on 127.0.0.1, port 11435 by default, the
//! desktop app's MCP Server setting). HTTP answers each request with a
//! single JSON response and rejects cross-site browser requests.

use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use toad_store::{RowQuery, Store, Table};

pub const DEFAULT_PORT: u16 = 11435;
const PROTOCOL: &str = "2025-06-18";
const SUPPORTED: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

/// The server's state: where crawls are listed from.
#[derive(Debug, Clone)]
pub struct Server {
    pub crawls_dir: PathBuf,
}

fn text_table(t: &Table, limit: usize) -> String {
    let mut out = t.columns.join("\t");
    for row in t.rows.iter().take(limit) {
        out.push('\n');
        out.push_str(&row.iter().map(|v| v.replace(['\t', '\n'], " ")).collect::<Vec<_>>().join("\t"));
    }
    if (t.rows.len() as u64) < t.total || t.rows.len() > limit {
        out.push_str(&format!("\n… {} rows in total (use offset/limit for more)", t.total.max(t.rows.len() as u64)));
    }
    out
}

fn arg_str<'a>(args: &'a Value, k: &str) -> anyhow::Result<&'a str> {
    args[k].as_str().filter(|s| !s.trim().is_empty()).ok_or_else(|| anyhow::anyhow!("missing argument '{k}'"))
}

fn arg_usize(args: &Value, k: &str, default: usize, max: usize) -> usize {
    args[k].as_u64().map(|v| v as usize).unwrap_or(default).min(max)
}

fn tool(name: &str, description: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": description, "inputSchema": {"type": "object", "properties": props, "required": required}})
}

fn crawl_prop() -> Value {
    json!({"type": "string", "description": "Path of a crawl file (.toad), from list_crawls"})
}

pub fn tools() -> Vec<Value> {
    vec![
        tool("list_crawls", "List saved crawls (newest first) with name, project, start URL, status and URL count.", json!({"folder": {"type": "string", "description": "Folder to list (default: the crawls folder)"}}), &[]),
        tool("list_tabs", "Tabs and their filters (issue type and priority, description) for a crawl.", json!({"crawl": crawl_prop()}), &["crawl"]),
        tool("crawl_overview", "Every tab and filter with its URL count.", json!({"crawl": crawl_prop(), "non_zero_only": {"type": "boolean"}}), &["crawl"]),
        tool("crawl_issues", "Issues, warnings and opportunities with URL counts, most important first.", json!({"crawl": crawl_prop()}), &["crawl"]),
        tool(
            "query_urls",
            "Rows of a tab and filter (like the main grid): e.g. tab 'Response Codes', filter 'Internal Client Error (4XX)'.",
            json!({
                "crawl": crawl_prop(),
                "tab": {"type": "string"},
                "filter": {"type": "string", "description": "Default: All"},
                "search": {"type": "string", "description": "Only addresses containing this text"},
                "segment": {"type": "string", "description": "Only URLs in this segment"},
                "offset": {"type": "integer"},
                "limit": {"type": "integer", "description": "Default 50, max 1000"}
            }),
            &["crawl", "tab"],
        ),
        tool("url_details", "Every stored field of one URL.", json!({"crawl": crawl_prop(), "url": {"type": "string"}}), &["crawl", "url"]),
        tool("inlinks", "Links pointing to a URL.", json!({"crawl": crawl_prop(), "url": {"type": "string"}, "limit": {"type": "integer"}}), &["crawl", "url"]),
        tool("outlinks", "Links from a URL.", json!({"crawl": crawl_prop(), "url": {"type": "string"}, "limit": {"type": "integer"}}), &["crawl", "url"]),
        tool("list_reports", "Reports and bulk exports available for a crawl.", json!({"crawl": crawl_prop()}), &["crawl"]),
        tool("run_report", "Run a report or bulk export, e.g. 'Redirect Chains' or 'Crawl Overview'.", json!({"crawl": crawl_prop(), "name": {"type": "string"}, "limit": {"type": "integer"}}), &["crawl", "name"]),
        tool(
            "compare_crawls",
            "Compare two crawls: issues whose URL counts changed between the previous and current crawl.",
            json!({"current": crawl_prop(), "previous": crawl_prop()}),
            &["current", "previous"],
        ),
    ]
}

impl Server {
    pub fn new(crawls_dir: impl Into<PathBuf>) -> Self {
        Self { crawls_dir: crawls_dir.into() }
    }

    fn open(&self, args: &Value, key: &str) -> anyhow::Result<Store> {
        let p = arg_str(args, key)?;
        let path = Path::new(p);
        let path = if path.is_absolute() || path.exists() { path.to_path_buf() } else { self.crawls_dir.join(p) };
        anyhow::ensure!(path.exists(), "no crawl file at {}", path.display());
        Store::open(&path)
    }

    /// Run one tool; the text it returns.
    pub fn call(&self, name: &str, args: &Value) -> anyhow::Result<String> {
        match name {
            "list_crawls" => {
                let dir = args["folder"].as_str().map(PathBuf::from).unwrap_or_else(|| self.crawls_dir.clone());
                let list = toad_store::crawls::list(&dir);
                if list.is_empty() {
                    return Ok(format!("No crawls in {}.", dir.display()));
                }
                Ok(list
                    .iter()
                    .map(|c| format!("{}\t{}\t{}\t{:?}\t{} URLs\t{}", c.name, c.project, c.start_url.clone().unwrap_or_default(), c.status, c.urls, c.path))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "list_tabs" => {
                let s = self.open(args, "crawl")?;
                let mut out = String::new();
                for t in s.tabs() {
                    out.push_str(&format!("{}\n", t.name));
                    for f in &t.filters {
                        let kind = f.issue.map(|(k, p)| format!(" [{k:?}, {p:?}]")).unwrap_or_default();
                        out.push_str(&format!("  {}{kind}: {}\n", f.name, f.description));
                    }
                }
                Ok(out)
            }
            "crawl_overview" => {
                let s = self.open(args, "crawl")?;
                let non_zero = args["non_zero_only"].as_bool().unwrap_or(true);
                Ok(s.overview()?
                    .into_iter()
                    .filter(|r| !non_zero || r.urls > 0)
                    .map(|r| format!("{}\t{}\t{}", r.tab, r.filter, r.urls))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "crawl_issues" => {
                let s = self.open(args, "crawl")?;
                let issues = s.issues()?;
                if issues.is_empty() {
                    return Ok("No issues.".into());
                }
                Ok(issues
                    .iter()
                    .map(|i| format!("{:?}\t{:?}\t{} URLs ({:.1}%)\t{}: {}\t{}", i.issue_type, i.priority, i.urls, i.pct_of_total, i.tab, i.filter, i.description))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "query_urls" => {
                let mut s = self.open(args, "crawl")?;
                if let Some(seg) = args["segment"].as_str().filter(|v| !v.is_empty()) {
                    s.set_segment_filter(Some(seg.to_string()));
                }
                let limit = arg_usize(args, "limit", 50, 1000);
                let t = s.rows(&RowQuery {
                    tab: arg_str(args, "tab")?.into(),
                    filter: args["filter"].as_str().unwrap_or("All").into(),
                    search: args["search"].as_str().map(str::to_string),
                    offset: args["offset"].as_u64().unwrap_or(0),
                    limit: Some(limit as u64),
                    ..Default::default()
                })?;
                Ok(text_table(&t, limit))
            }
            "url_details" => {
                let s = self.open(args, "crawl")?;
                let url = arg_str(args, "url")?;
                let d = s.url_details(url)?;
                anyhow::ensure!(!d.is_empty(), "{url} is not in this crawl");
                Ok(d.into_iter().filter(|(k, v)| !v.is_empty() && k != "minhash" && k != "headers_json").map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join("\n"))
            }
            "inlinks" | "outlinks" => {
                let s = self.open(args, "crawl")?;
                let url = arg_str(args, "url")?;
                let t = if name == "inlinks" { s.inlinks(url)? } else { s.outlinks(url)? };
                Ok(text_table(&t, arg_usize(args, "limit", 100, 2000)))
            }
            "list_reports" => {
                let s = self.open(args, "crawl")?;
                Ok(s.reports().iter().map(|r| format!("{}\t{:?}\t{}", r.name, r.kind, r.description)).collect::<Vec<_>>().join("\n"))
            }
            "run_report" => {
                let s = self.open(args, "crawl")?;
                let t = s.report(arg_str(args, "name")?)?;
                Ok(text_table(&t, arg_usize(args, "limit", 100, 2000)))
            }
            "compare_crawls" => {
                let cur = self.open(args, "current")?;
                let prev = self.open(args, "previous")?;
                let (cp, pp) = (PathBuf::from(arg_str(args, "current")?), PathBuf::from(arg_str(args, "previous")?));
                drop((cur, prev));
                let resolve = |p: PathBuf| if p.exists() { p } else { self.crawls_dir.join(p) };
                let out = std::env::temp_dir().join(format!("toad-mcp-{}.toadcmp", std::process::id()));
                let cmp = toad_store::Comparison::build(&resolve(cp), &resolve(pp), &out, &toad_store::CompareConfig::default())?;
                let rows: Vec<String> = cmp
                    .issues()?
                    .into_iter()
                    .filter(|i| i.current != i.previous)
                    .map(|i| format!("{}: {}\tcurrent {}\tprevious {}\tchange {:+}\t(added {}, new {}, removed {}, missing {})", i.tab, i.filter, i.current, i.previous, i.change, i.added, i.new, i.removed, i.missing))
                    .collect();
                drop(cmp);
                let _ = std::fs::remove_file(&out);
                Ok(if rows.is_empty() { "No issue counts changed.".into() } else { rows.join("\n") })
            }
            other => anyhow::bail!("unknown tool '{other}'"),
        }
    }

    /// Handle one JSON-RPC message; None for notifications.
    pub fn handle(&self, msg: &Value) -> Option<Value> {
        let id = msg.get("id").cloned();
        let method = msg["method"].as_str().unwrap_or("");
        let id = id?; // Notifications get no response.
        let result = match method {
            "initialize" => {
                let asked = msg["params"]["protocolVersion"].as_str().unwrap_or(PROTOCOL);
                let version = if SUPPORTED.contains(&asked) { asked } else { PROTOCOL };
                Ok(json!({
                    "protocolVersion": version,
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "screaming-toad", "title": "Screaming Toad SEO Spider", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": "Read-only access to Screaming Toad crawls. Start with list_crawls, then crawl_issues or query_urls on a crawl path."
                }))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = msg["params"]["name"].as_str().unwrap_or("");
                let args = msg["params"].get("arguments").cloned().unwrap_or(json!({}));
                // Tool failures are results with isError, so the model can see them.
                Ok(match self.call(name, &args) {
                    Ok(text) => json!({"content": [{"type": "text", "text": text}], "isError": false}),
                    Err(e) => json!({"content": [{"type": "text", "text": format!("Error: {e:#}")}], "isError": true}),
                })
            }
            _ => Err((-32601, format!("method not found: {method}"))),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
        })
    }

    /// STDIO transport: one JSON-RPC message per line on stdin/stdout.
    pub async fn serve_stdio(&self) -> anyhow::Result<()> {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        let mut out = tokio::io::stdout();
        while let Some(line) = lines.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }
            let reply = match serde_json::from_str::<Value>(&line) {
                Ok(v) => self.handle_any(&v),
                Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}})),
            };
            if let Some(r) = reply {
                out.write_all(format!("{r}\n").as_bytes()).await?;
                out.flush().await?;
            }
        }
        Ok(())
    }

    /// A single message or a batch.
    fn handle_any(&self, v: &Value) -> Option<Value> {
        match v.as_array() {
            Some(batch) => {
                let replies: Vec<Value> = batch.iter().filter_map(|m| self.handle(m)).collect();
                (!replies.is_empty()).then(|| Value::Array(replies))
            }
            None => self.handle(v),
        }
    }

    /// Streamable HTTP transport on 127.0.0.1:`port`, until `stop` resolves.
    pub async fn serve_http(self, port: u16, stop: impl std::future::Future<Output = ()>) -> anyhow::Result<()> {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
        self.serve_listener(listener, stop).await
    }

    /// Streamable HTTP on an already bound listener.
    pub async fn serve_listener(self, listener: tokio::net::TcpListener, stop: impl std::future::Future<Output = ()>) -> anyhow::Result<()> {
        let server = std::sync::Arc::new(self);
        tokio::pin!(stop);
        loop {
            tokio::select! {
                _ = &mut stop => return Ok(()),
                accepted = listener.accept() => {
                    let Ok((sock, _)) = accepted else { continue };
                    let server = server.clone();
                    tokio::spawn(async move { let _ = server.http_conn(sock).await; });
                }
            }
        }
    }

    async fn http_conn(&self, mut sock: tokio::net::TcpStream) -> anyhow::Result<()> {
        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        let header_end = loop {
            let n = sock.read(&mut chunk).await?;
            anyhow::ensure!(n > 0, "closed");
            buf.extend_from_slice(&chunk[..n]);
            if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                break p + 4;
            }
            anyhow::ensure!(buf.len() < 64 * 1024, "headers too large");
        };
        let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
        let mut lines = head.split("\r\n");
        let first = lines.next().unwrap_or("");
        let mut parts = first.split_whitespace();
        let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let headers: Vec<(String, String)> = lines.filter_map(|l| l.split_once(':')).map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())).collect();
        let header = |k: &str| headers.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        let len: usize = header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
        anyhow::ensure!(len <= 8 * 1024 * 1024, "body too large");
        while buf.len() < header_end + len {
            let n = sock.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let body = &buf[header_end..(header_end + len).min(buf.len())];

        // DNS-rebinding protection: browsers send Origin; only local pages may call.
        let origin_ok = header("origin").is_none_or(|o| {
            let o = o.to_ascii_lowercase();
            o.starts_with("http://127.0.0.1") || o.starts_with("http://localhost") || o.starts_with("https://localhost") || o == "null"
        });
        let (status, ctype, out) = if !origin_ok {
            (403, "text/plain", "Forbidden origin".to_string())
        } else if path.split('?').next() != Some("/mcp") {
            (404, "text/plain", "Not found: the MCP endpoint is /mcp".to_string())
        } else if method != "POST" {
            (405, "text/plain", "Use POST (this server doesn't open SSE streams)".to_string())
        } else {
            match serde_json::from_slice::<Value>(body) {
                Ok(v) => match self.handle_any(&v) {
                    Some(r) => (200, "application/json", r.to_string()),
                    None => (202, "application/json", String::new()),
                },
                Err(e) => (400, "application/json", json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}}).to_string()),
            }
        };
        let reason = match status {
            200 => "OK",
            202 => "Accepted",
            400 => "Bad Request",
            403 => "Forbidden",
            404 => "Not Found",
            _ => "Method Not Allowed",
        };
        let allow = if status == 405 { "Allow: POST\r\n" } else { "" };
        let resp = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\n{allow}Connection: close\r\n\r\n{out}", out.len());
        sock.write_all(resp.as_bytes()).await?;
        let _ = sock.shutdown().await;
        Ok(())
    }
}
