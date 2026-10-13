//! File > Crawls: the crawls saved in a storage folder, and what can be
//! done to them (rename, project, lock, duplicate, delete, retention).
//!
//! Everything lives in each crawl's own `meta` table, so a `.toad` file is
//! self-describing and can be copied between machines.

use crate::Store;
use toad_core::model::{Link, LinkKind};
use toad_core::PendingLink;
use anyhow::{bail, Context};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Where a crawl got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CrawlStatus {
    /// Running, or the app stopped without finishing it (a crash): resumable.
    Crawling,
    /// Stopped by the user before the queue was empty: resumable.
    Stopped,
    Complete,
}

impl CrawlStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Crawling => "crawling",
            Self::Stopped => "stopped",
            Self::Complete => "complete",
        }
    }

    fn parse(s: &str) -> Self {
        match s {
            "crawling" => Self::Crawling,
            "stopped" => Self::Stopped,
            _ => Self::Complete,
        }
    }
}

/// One row of File > Crawls.
#[derive(Debug, Clone, Serialize)]
pub struct CrawlEntry {
    pub path: String,
    pub name: String,
    pub project: String,
    pub start_url: Option<String>,
    pub mode: String,
    pub urls: u64,
    /// URLs still queued when the crawl stopped (0 once complete).
    pub queued: u64,
    /// Crawled / (crawled + queued) × 100.
    pub pct_complete: f64,
    pub status: CrawlStatus,
    /// Seconds since the Unix epoch.
    pub created: u64,
    pub modified: u64,
    pub size_bytes: u64,
    pub version: String,
    pub locked: bool,
}

pub fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

fn modified(path: &Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs())
}

impl Store {
    /// Record that a crawl has started: name, mode, start time, status.
    pub fn mark_started(&self, name: &str, mode: &str) -> anyhow::Result<()> {
        if self.meta("name")?.is_none() {
            self.set_meta("name", name)?;
        }
        if self.meta("created")?.is_none() {
            self.set_meta("created", &now().to_string())?;
        }
        self.set_meta("mode", mode)?;
        self.set_meta("status", CrawlStatus::Crawling.as_str())?;
        // New pages make earlier analysis stale.
        self.set_meta("analysed", "false")?;
        Ok(())
    }

    /// Record where a crawl ended. `queued` > 0 means it was stopped early.
    pub fn mark_finished(&self, queued: u64) -> anyhow::Result<()> {
        let status = if queued > 0 { CrawlStatus::Stopped } else { CrawlStatus::Complete };
        self.set_meta("status", status.as_str())?;
        self.set_meta("queued", &queued.to_string())?;
        self.set_meta("finished_at", &now().to_string())?;
        Ok(())
    }

    /// List mode: keep the uploaded URLs in order, duplicates included.
    pub fn set_list_inputs(&self, inputs: &[String]) -> anyhow::Result<()> {
        self.conn.execute("DELETE FROM list_inputs", [])?;
        let mut stmt = self.conn.prepare("INSERT INTO list_inputs (ord, url, address) VALUES (?1, ?2, ?3)")?;
        for (i, url) in inputs.iter().enumerate() {
            // Crawled as given, minus any fragment (see Crawler::run).
            let address = url::Url::parse(url.trim()).ok().map(|mut u| {
                u.set_fragment(None);
                u.to_string()
            });
            stmt.execute(rusqlite::params![i as i64, url, address])?;
        }
        Ok(())
    }

    pub fn status(&self) -> anyhow::Result<CrawlStatus> {
        Ok(self.meta("status")?.map_or(CrawlStatus::Complete, |s| CrawlStatus::parse(&s)))
    }

    pub fn entry(&self, path: &Path) -> anyhow::Result<CrawlEntry> {
        let get = |k: &str| self.meta(k).map(|v| v.unwrap_or_default());
        let urls = self.url_count()?;
        let status = self.status()?;
        let queued: u64 = if status == CrawlStatus::Complete { 0 } else { get("queued")?.parse().unwrap_or(0) };
        let start_url = self.meta("start_url")?;
        let name = match get("name")? {
            n if !n.is_empty() => n,
            _ => path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        };
        let file_modified = modified(path);
        Ok(CrawlEntry {
            path: path.to_string_lossy().into_owned(),
            name,
            project: get("project")?,
            start_url,
            mode: match get("mode")?.as_str() {
                "" => "spider".into(),
                m => m.to_string(),
            },
            urls,
            queued,
            pct_complete: if urls + queued == 0 { 100.0 } else { (urls as f64 * 1000.0 / (urls + queued) as f64).round() / 10.0 },
            status,
            created: get("created")?.parse().unwrap_or(file_modified),
            modified: file_modified,
            size_bytes: std::fs::metadata(path).map_or(0, |m| m.len()),
            version: get("toad_version")?,
            locked: get("locked")? == "true",
        })
    }
}

impl Store {
    /// What a resumed crawl starts from: every URL already crawled, and every
    /// recorded link (plus meta refresh and sitemap URLs) whose target was
    /// never crawled.
    pub fn resume_state(&self) -> anyhow::Result<(Vec<String>, Vec<PendingLink>)> {
        let crawled: Vec<String> = {
            let mut stmt = self.conn.prepare("SELECT address FROM urls")?;
            let rows = stmt.query_map([], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let nofollow = "(coalesce(s.directives, '') LIKE '%,nofollow,%' OR coalesce(s.directives, '') LIKE '%,none,%')";
        let sql = format!(
            "SELECT l.kind, l.target, l.anchor, l.alt, l.follow, l.rel, s.depth, {nofollow}
             FROM links l JOIN urls s ON s.id = l.source_id
             WHERE NOT EXISTS (SELECT 1 FROM urls t WHERE t.address = l.target)
             UNION ALL
             SELECT 'Meta Refresh', s.redirect_url, '', NULL, 1, '', s.depth, {nofollow}
             FROM urls s
             WHERE s.redirect_type = 'Meta Refresh Redirect' AND s.redirect_url IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM urls t WHERE t.address = s.redirect_url)
             UNION ALL
             SELECT DISTINCT 'Hyperlink', e.url, '', NULL, 1, '', 0, 0
             FROM sitemap_entries e
             WHERE NOT EXISTS (SELECT 1 FROM urls t WHERE t.address = e.url)"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, bool>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, bool>(7)?,
            ))
        })?;
        let mut pending = Vec::new();
        for row in rows {
            let (kind, to, anchor, alt, follow, rel, depth, source_nofollow) = row?;
            let Some(kind) = LinkKind::parse(&kind) else { continue };
            pending.push(PendingLink {
                link: Link {
                    kind,
                    href: to.clone(),
                    to,
                    anchor: anchor.unwrap_or_default(),
                    alt,
                    follow,
                    rel: rel.unwrap_or_default(),
                    target: String::new(),
                    path_type: String::new(),
                    link_path: String::new(),
                    position: String::new(),
                    hreflang: None,
                    origin: String::new(),
                },
                source_depth: depth.max(0) as u32,
                source_nofollow,
            });
        }
        Ok((crawled, pending))
    }
}

/// Every crawl in `dir`, newest first. Files that cannot be opened are skipped.
pub fn list(dir: &Path) -> Vec<CrawlEntry> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<CrawlEntry> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toad"))
        .filter_map(|p| Store::open(&p).and_then(|s| s.entry(&p)).ok())
        .collect();
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

pub fn entry(path: &Path) -> anyhow::Result<CrawlEntry> {
    Store::open(path)?.entry(path)
}

pub fn rename(path: &Path, name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() {
        bail!("a crawl needs a name");
    }
    Store::open(path)?.set_meta("name", name.trim())
}

/// Put a crawl in a project ("" removes it from any project).
pub fn set_project(path: &Path, project: &str) -> anyhow::Result<()> {
    Store::open(path)?.set_meta("project", project.trim())
}

/// Locked crawls cannot be deleted and are skipped by Crawl Retention.
pub fn set_locked(path: &Path, locked: bool) -> anyhow::Result<()> {
    Store::open(path)?.set_meta("locked", if locked { "true" } else { "false" })
}

/// Duplicate a crawl next to the original, named "<name> (copy)". Returns the new path.
pub fn duplicate(path: &Path) -> anyhow::Result<PathBuf> {
    let store = Store::open(path)?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "crawl".into());
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut n = 1;
    let target = loop {
        let candidate = dir.join(format!("{stem}-copy{}.toad", if n == 1 { String::new() } else { n.to_string() }));
        if !candidate.exists() {
            break candidate;
        }
        n += 1;
    };
    store.save_copy(&target)?;
    let copy = Store::open(&target)?;
    let name = store.entry(path)?.name;
    copy.set_meta("name", &format!("{name} (copy)"))?;
    copy.set_meta("locked", "false")?;
    copy.set_meta("created", &now().to_string())?;
    Ok(target)
}

/// Delete a crawl file. Locked crawls are refused.
pub fn delete(path: &Path) -> anyhow::Result<()> {
    if entry(path)?.locked {
        bail!("this crawl is locked; unlock it before deleting");
    }
    for suffix in ["", "-journal", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{suffix}", path.display()));
        if p.exists() {
            std::fs::remove_file(&p).with_context(|| format!("deleting {}", p.display()))?;
        }
    }
    Ok(())
}

/// Crawl Retention: delete unlocked, non-running crawls created more than
/// `days` ago, except `keep` (the crawl that is open). Returns what was deleted.
pub fn apply_retention(dir: &Path, days: u64, keep: Option<&Path>) -> Vec<PathBuf> {
    let cutoff = now().saturating_sub(days * 86_400);
    let keep = keep.and_then(|k| std::fs::canonicalize(k).ok());
    list(dir)
        .into_iter()
        .filter(|e| !e.locked && e.created < cutoff && e.status != CrawlStatus::Crawling)
        .map(|e| PathBuf::from(e.path))
        .filter(|p| keep.as_ref().is_none_or(|k| std::fs::canonicalize(p).ok().as_ref() != Some(k)))
        .filter(|p| delete(p).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use toad_core::CrawlConfig;
    use url::Url;

    fn crawl(dir: &Path, file: &str) -> PathBuf {
        let path = dir.join(file);
        let store = Store::create(&path, &Url::parse("https://example.com/").unwrap(), &CrawlConfig::default()).unwrap();
        store.mark_started("example.com", "spider").unwrap();
        path
    }

    #[test]
    fn list_mode_export_keeps_order_and_duplicates() {
        let dir = std::env::temp_dir().join(format!("toad-list-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = crawl(&dir, "list.toad");
        let store = Store::open(&path).unwrap();
        let inputs = ["https://example.com/b", "https://example.com/a#top", "https://example.com/b", "not a url"].map(String::from);
        store.set_list_inputs(&inputs).unwrap();
        let t = store.report("List Mode Export").unwrap();
        let col: Vec<&str> = t.rows.iter().map(|r| r[0].as_str()).collect();
        assert_eq!(col, inputs.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        assert_eq!(t.columns[0], "Original URL");
    }

    #[test]
    fn manage_crawls() {
        let dir = std::env::temp_dir().join(format!("toad-crawls-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let a = crawl(&dir, "a.toad");
        let b = crawl(&dir, "b.toad");
        Store::open(&b).unwrap().mark_finished(0).unwrap();
        Store::open(&a).unwrap().mark_finished(5).unwrap();

        let entries = list(&dir);
        assert_eq!(entries.len(), 2);
        let ea = entry(&a).unwrap();
        assert_eq!((ea.status, ea.queued, ea.pct_complete), (CrawlStatus::Stopped, 5, 0.0));
        assert_eq!(entry(&b).unwrap().status, CrawlStatus::Complete);

        rename(&a, "Staging").unwrap();
        set_project(&a, "Client X").unwrap();
        set_locked(&a, true).unwrap();
        let ea = entry(&a).unwrap();
        assert_eq!((ea.name.as_str(), ea.project.as_str(), ea.locked), ("Staging", "Client X", true));
        assert!(delete(&a).is_err(), "locked crawls are not deleted");

        let copy = duplicate(&a).unwrap();
        let ec = entry(&copy).unwrap();
        assert_eq!((ec.name.as_str(), ec.locked, ec.project.as_str()), ("Staging (copy)", false, "Client X"));

        // Retention skips locked crawls and the open one.
        for p in [&a, &b, &copy] {
            Store::open(p).unwrap().set_meta("created", "1").unwrap();
        }
        let deleted = apply_retention(&dir, 7, Some(&b));
        assert_eq!(deleted, vec![copy.clone()]);
        assert!(a.exists() && b.exists() && !copy.exists());
    }
}
