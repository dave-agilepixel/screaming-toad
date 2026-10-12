//! URL resolution, normalisation, rewriting and crawl scope.

use crate::config::CrawlConfig;
use url::Url;

/// Resolve an href found on a page. Returns None for non-HTTP schemes
/// (mailto:, tel:, javascript:, data:) and unparseable values.
pub fn resolve(base: &Url, href: &str) -> Option<Url> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }
    let lower = href.to_ascii_lowercase();
    if ["mailto:", "tel:", "javascript:", "data:", "sms:", "ftp:", "file:"]
        .iter()
        .any(|s| lower.starts_with(s))
    {
        return None;
    }
    let url = base.join(href).ok()?;
    matches!(url.scheme(), "http" | "https").then_some(url)
}

/// Apply URL Rewriting settings and fragment handling.
pub fn normalize(mut url: Url, cfg: &CrawlConfig) -> Url {
    if !cfg.crawl_fragment_identifiers {
        url.set_fragment(None);
    }
    if !cfg.remove_parameters.is_empty() && url.query().is_some() {
        let kept: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| !cfg.remove_parameters.iter().any(|p| p == k.as_ref()))
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        if kept.is_empty() {
            url.set_query(None);
        } else {
            url.query_pairs_mut().clear().extend_pairs(kept);
        }
    }
    if cfg.lowercase_urls {
        if let Ok(lower) = Url::parse(&url.as_str().to_lowercase()) {
            url = lower;
        }
    }
    url
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathType {
    Absolute,
    ProtocolRelative,
    RootRelative,
    PathRelative,
}

impl PathType {
    pub fn of(href: &str) -> Self {
        let h = href.trim();
        if h.starts_with("//") {
            PathType::ProtocolRelative
        } else if h.starts_with('/') {
            PathType::RootRelative
        } else if h.contains("://") {
            PathType::Absolute
        } else {
            PathType::PathRelative
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            PathType::Absolute => "Absolute",
            PathType::ProtocolRelative => "Protocol-Relative",
            PathType::RootRelative => "Root-Relative",
            PathType::PathRelative => "Path-Relative",
        }
    }
}

/// `/` = 0, `/a/` = 1, `/a/page.html` = 1, `/a/b/` = 2.
pub fn folder_depth(url: &Url) -> u32 {
    url.path().matches('/').count().saturating_sub(1) as u32
}

pub fn query_param_count(url: &Url) -> u32 {
    url.query()
        .map(|q| q.split('&').filter(|p| !p.is_empty()).count() as u32)
        .unwrap_or(0)
}

/// Decides what is Internal, and whether the crawl may go there.
#[derive(Debug, Clone)]
pub struct Scope {
    host: String,
    root_domain: String,
    include_subdomains: bool,
    start_folder: String,
}

impl Scope {
    pub fn new(start: &Url, cfg: &CrawlConfig) -> Self {
        let host = start.host_str().unwrap_or_default().to_ascii_lowercase();
        let root_domain = host.strip_prefix("www.").unwrap_or(&host).to_string();
        // A start URL with no subdomain crawls every subdomain, like a site: query.
        let include_subdomains = cfg.crawl_all_subdomains || !has_subdomain(&host);
        let path = start.path();
        let start_folder = match path.rfind('/') {
            Some(i) => path[..=i].to_string(),
            None => "/".to_string(),
        };
        Self { host, root_domain, include_subdomains, start_folder }
    }

    pub fn is_internal(&self, url: &Url) -> bool {
        let host = match url.host_str() {
            Some(h) => h.to_ascii_lowercase(),
            None => return false,
        };
        if host == self.host {
            return true;
        }
        self.include_subdomains
            && (host == self.root_domain || host.ends_with(&format!(".{}", self.root_domain)))
    }

    pub fn in_start_folder(&self, url: &Url) -> bool {
        url.path().starts_with(&self.start_folder)
    }
}

/// Rough check for a subdomain. A public-suffix list would be exact; this
/// handles the common two-part suffixes (co.uk, com.au and so on).
fn has_subdomain(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if host.parse::<std::net::IpAddr>().is_ok() || labels.len() < 3 {
        return false;
    }
    let n = labels.len();
    let two_part_suffix = labels[n - 1].len() == 2
        && ["co", "com", "org", "net", "ac", "gov", "edu", "ltd", "plc", "me"]
            .contains(&labels[n - 2]);
    if two_part_suffix {
        labels.len() > 3
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_depth_matches_screaming_frog() {
        let d = |s: &str| folder_depth(&Url::parse(s).unwrap());
        assert_eq!(d("https://a.com/"), 0);
        assert_eq!(d("https://a.com/a/"), 1);
        assert_eq!(d("https://a.com/a/page.html"), 1);
        assert_eq!(d("https://a.com/a/b/"), 2);
    }

    #[test]
    fn scope_subdomains() {
        let cfg = CrawlConfig::default();
        let s = Scope::new(&Url::parse("https://example.com/").unwrap(), &cfg);
        assert!(s.is_internal(&Url::parse("https://blog.example.com/x").unwrap()));
        let s = Scope::new(&Url::parse("https://www.example.co.uk/").unwrap(), &cfg);
        assert!(!s.is_internal(&Url::parse("https://blog.example.co.uk/x").unwrap()));
        assert!(s.is_internal(&Url::parse("https://www.example.co.uk/x").unwrap()));
    }

    #[test]
    fn removes_parameters() {
        let mut cfg = CrawlConfig::default();
        cfg.remove_parameters = vec!["utm_source".into()];
        let u = normalize(Url::parse("https://a.com/p?utm_source=x&id=2#top").unwrap(), &cfg);
        assert_eq!(u.as_str(), "https://a.com/p?id=2");
    }
}
