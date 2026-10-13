//! File > Settings > Notifications: an email when a crawl (usually a
//! scheduled one) finishes, with its issues, and the changes since the last
//! crawl when it was compared (Auto Compare).
//!
//! Settings live in ~/.toad/notifications.json so the desktop app and the
//! CLI (scheduled crawls) share them; the SMTP password is a secret
//! (`smtp_password`).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toad_store::{IssueRow, Store};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmtpSecurity {
    /// STARTTLS on port 587.
    #[default]
    StartTls,
    /// Implicit TLS on port 465.
    Tls,
    /// No encryption (local relays only).
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub security: SmtpSecurity,
    pub username: String,
    pub from: String,
    /// Used when a task or `--email-on-complete` gives no addresses.
    pub default_recipients: Vec<String>,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self { smtp_host: String::new(), smtp_port: 587, security: SmtpSecurity::StartTls, username: String::new(), from: String::new(), default_recipients: vec![] }
    }
}

pub fn settings_path() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from).unwrap_or_default();
    home.join(".toad").join("notifications.json")
}

pub fn load_settings() -> NotificationSettings {
    std::fs::read_to_string(settings_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

pub fn save_settings(s: &NotificationSettings) -> anyhow::Result<()> {
    let p = settings_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, serde_json::to_string_pretty(s)?)?;
    Ok(())
}

/// One crawl's summary for the email.
#[derive(Debug, Clone, Default)]
pub struct CrawlSummary {
    pub name: String,
    pub start_url: String,
    pub urls: u64,
    pub issues: Vec<IssueRow>,
    /// Auto Compare: (issue, current, previous) for issues that changed.
    pub changes: Vec<(String, u64, u64)>,
    pub files: Vec<String>,
}

impl CrawlSummary {
    pub fn from_store(store: &Store) -> anyhow::Result<Self> {
        Ok(Self {
            name: store.meta("name")?.unwrap_or_default(),
            start_url: store.meta("start_url")?.unwrap_or_default(),
            urls: store.count("Internal", "All")? + store.count("External", "All")?,
            issues: store.issues()?,
            changes: vec![],
            files: vec![],
        })
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Subject, plain text and HTML bodies.
pub fn compose(s: &CrawlSummary) -> (String, String, String) {
    let site = url::Url::parse(&s.start_url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| s.start_url.clone());
    let subject = format!("Screaming Toad: crawl of {site} complete ({} URLs)", s.urls);
    let count = |t: &str| s.issues.iter().filter(|i| format!("{:?}", i.issue_type) == t).count();
    let mut text = format!(
        "Crawl: {}\nStart URL: {}\nURLs crawled: {}\nIssues: {}  Warnings: {}  Opportunities: {}\n\n",
        if s.name.is_empty() { &site } else { &s.name },
        s.start_url,
        s.urls,
        count("Issue"),
        count("Warning"),
        count("Opportunity")
    );
    let mut html = format!(
        "<h2>{}</h2><p>Start URL: {}<br>URLs crawled: <b>{}</b><br>Issues: <b>{}</b> · Warnings: <b>{}</b> · Opportunities: <b>{}</b></p>",
        esc(if s.name.is_empty() { &site } else { &s.name }),
        esc(&s.start_url),
        s.urls,
        count("Issue"),
        count("Warning"),
        count("Opportunity")
    );
    if !s.changes.is_empty() {
        text.push_str("Changes since the previous crawl:\n");
        html.push_str("<h3>Changes since the previous crawl</h3><table border=\"1\" cellpadding=\"4\" cellspacing=\"0\"><tr><th>Issue</th><th>Current</th><th>Previous</th><th>Change</th></tr>");
        for (name, cur, prev) in &s.changes {
            let d = *cur as i64 - *prev as i64;
            text.push_str(&format!("  {name}: {cur} (was {prev}, {d:+})\n"));
            html.push_str(&format!("<tr><td>{}</td><td>{cur}</td><td>{prev}</td><td>{d:+}</td></tr>", esc(name)));
        }
        html.push_str("</table>");
        text.push('\n');
    }
    text.push_str("Top issues:\n");
    html.push_str("<h3>Top issues</h3><table border=\"1\" cellpadding=\"4\" cellspacing=\"0\"><tr><th>Issue</th><th>Type</th><th>Priority</th><th>URLs</th></tr>");
    for i in s.issues.iter().take(25) {
        let name = format!("{}: {}", i.tab, i.filter);
        text.push_str(&format!("  {:?} {:?}  {:>6}  {name}\n", i.issue_type, i.priority, i.urls));
        html.push_str(&format!("<tr><td>{}</td><td>{:?}</td><td>{:?}</td><td>{}</td></tr>", esc(&name), i.issue_type, i.priority, i.urls));
    }
    html.push_str("</table>");
    if !s.files.is_empty() {
        text.push_str("\nFiles:\n");
        html.push_str("<h3>Files</h3><ul>");
        for f in &s.files {
            text.push_str(&format!("  {f}\n"));
            html.push_str(&format!("<li>{}</li>", esc(f)));
        }
        html.push_str("</ul>");
    }
    (subject, text, format!("<!doctype html><html><body style=\"font-family:sans-serif\">{html}</body></html>"))
}

/// Send an email (subject, text, HTML) to `to` (or the default recipients).
pub async fn send(settings: &NotificationSettings, password: Option<&str>, to: &[String], subject: &str, text: &str, html: &str) -> anyhow::Result<()> {
    use lettre::message::{Mailbox, MultiPart};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

    anyhow::ensure!(!settings.smtp_host.trim().is_empty(), "set up email in File > Settings > Notifications first");
    let recipients: Vec<&String> = if to.is_empty() { settings.default_recipients.iter().collect() } else { to.iter().collect() };
    anyhow::ensure!(!recipients.is_empty(), "no email address to send to");
    let from: Mailbox = if settings.from.trim().is_empty() { settings.username.parse()? } else { settings.from.parse()? };
    let mut msg = Message::builder().from(from).subject(subject);
    for r in recipients {
        msg = msg.to(r.trim().parse().map_err(|e| anyhow::anyhow!("'{r}' is not an email address ({e})"))?);
    }
    let email = msg.multipart(MultiPart::alternative_plain_html(text.to_string(), html.to_string()))?;
    let host = settings.smtp_host.trim();
    let mut builder = match settings.security {
        SmtpSecurity::StartTls => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host)?,
        SmtpSecurity::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(host)?,
        SmtpSecurity::None => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host),
    }
    .port(settings.smtp_port)
    .timeout(Some(std::time::Duration::from_secs(30)));
    if !settings.username.trim().is_empty() {
        builder = builder.credentials(Credentials::new(settings.username.trim().to_string(), password.unwrap_or("").to_string()));
    }
    builder.build().send(email).await?;
    Ok(())
}

/// Email a crawl summary.
pub async fn notify(settings: &NotificationSettings, password: Option<&str>, to: &[String], summary: &CrawlSummary) -> anyhow::Result<()> {
    let (subject, text, html) = compose(summary);
    send(settings, password, to, &subject, &text, &html).await
}
