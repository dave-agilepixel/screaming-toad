//! Configuration > Authentication and the network settings (proxy and
//! trusted certificates).
//!
//! These are kept out of `CrawlConfig` on purpose: the crawl config is saved
//! into every `.toad` file and configuration profile, and credentials must
//! not be. The desktop app holds them for the session (and in its settings
//! for the proxy); the CLI reads them from an encrypted auth file.
//!
//! - Standards-based: Basic and Digest (RFC 7616: MD5, MD5-sess, SHA-256,
//!   qop=auth) challenges are answered with stored credentials, or by asking
//!   the user through the prompter.
//! - Forms-based: cookies captured after logging in through a browser window
//!   are sent with every matching request, and set in the renderer.

use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{mpsc, oneshot};
use url::Url;

/// One standards-based (Basic/Digest) login.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Credential {
    /// A host (`example.com`, `example.com:8443`) or a URL prefix.
    pub url: String,
    pub username: String,
    pub password: String,
}

impl Credential {
    pub fn matches(&self, url: &Url) -> bool {
        let want = self.url.trim();
        if want.is_empty() {
            return false;
        }
        if want.contains("://") {
            return url.as_str().starts_with(want);
        }
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        let want = want.trim_end_matches('/').to_ascii_lowercase();
        match url.port() {
            Some(p) => want == format!("{host}:{p}") || want == host,
            None => want == host,
        }
    }
}

/// A cookie captured by a forms-based login.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthCookie {
    pub name: String,
    pub value: String,
    /// Cookie domain; a leading dot (or `host_only = false`) covers subdomains.
    pub domain: String,
    pub path: String,
    pub secure: bool,
    pub http_only: bool,
    pub host_only: bool,
}

impl AuthCookie {
    pub fn matches(&self, url: &Url) -> bool {
        let host = url.host_str().unwrap_or("").to_ascii_lowercase();
        let domain = self.domain.trim_start_matches('.').to_ascii_lowercase();
        let domain_ok = host == domain || (!self.host_only && host.ends_with(&format!(".{domain}")));
        let path = if self.path.is_empty() { "/" } else { self.path.as_str() };
        let path_ok = url.path().starts_with(path);
        domain_ok && path_ok && (!self.secure || url.scheme() == "https")
    }
}

/// One forms-based login: the page it started from and the cookies it left.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FormLogin {
    pub name: String,
    pub login_url: String,
    pub cookies: Vec<AuthCookie>,
}

/// Configuration > Authentication.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    /// Standards Based > Enable authentication prompts during the crawl.
    pub prompt: bool,
    pub standards: Vec<Credential>,
    pub forms: Vec<FormLogin>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self { prompt: true, standards: vec![], forms: vec![] }
    }
}

impl AuthConfig {
    pub fn is_empty(&self) -> bool {
        self.standards.is_empty() && self.forms.is_empty()
    }

    /// Forms-based cookies for a URL, as a Cookie header value.
    pub fn cookie_header(&self, url: &Url) -> Option<String> {
        let parts: Vec<String> = self
            .forms
            .iter()
            .flat_map(|f| f.cookies.iter())
            .filter(|c| c.matches(url))
            .map(|c| format!("{}={}", c.name, c.value))
            .collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProxyKind {
    #[default]
    Http,
    Https,
    Socks5,
}

/// File > Settings > Proxy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// Hosts that bypass the proxy (comma or newline separated in the UI).
    pub no_proxy: Vec<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self { enabled: false, kind: ProxyKind::Http, host: String::new(), port: 8080, username: String::new(), password: String::new(), no_proxy: vec![] }
    }
}

impl ProxyConfig {
    /// `scheme://host:port`, without credentials.
    pub fn url(&self) -> Option<String> {
        if !self.enabled || self.host.trim().is_empty() {
            return None;
        }
        let scheme = match self.kind {
            ProxyKind::Http => "http",
            ProxyKind::Https => "https",
            ProxyKind::Socks5 => "socks5h",
        };
        Some(format!("{scheme}://{}:{}", self.host.trim(), self.port))
    }

    /// Parse `scheme://[user:pass@]host:port` (the CLI's `--proxy`).
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let u = Url::parse(s.trim())?;
        let kind = match u.scheme() {
            "http" => ProxyKind::Http,
            "https" => ProxyKind::Https,
            "socks5" | "socks5h" => ProxyKind::Socks5,
            other => anyhow::bail!("unsupported proxy scheme {other}"),
        };
        Ok(Self {
            enabled: true,
            kind,
            host: u.host_str().ok_or_else(|| anyhow::anyhow!("proxy needs a host"))?.to_string(),
            port: u.port_or_known_default().unwrap_or(8080),
            username: percent_decode(u.username()),
            password: percent_decode(u.password().unwrap_or("")),
            no_proxy: vec![],
        })
    }
}

fn percent_decode(s: &str) -> String {
    url::form_urlencoded::parse(format!("x={s}").as_bytes()).next().map(|(_, v)| v.into_owned()).unwrap_or_default()
}

/// App-level network settings: File > Settings > Proxy and Trusted Certificates.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkSettings {
    pub proxy: ProxyConfig,
    /// PEM or DER certificate files trusted in addition to the system store.
    pub trusted_certificates: Vec<String>,
}

/// A standards-based challenge the user is asked to answer.
#[derive(Debug, Clone, Serialize)]
pub struct AuthChallenge {
    pub url: String,
    pub realm: String,
    /// "Basic" or "Digest".
    pub scheme: String,
}

pub struct AuthPrompt {
    pub challenge: AuthChallenge,
    /// Some((username, password)) to log in; None to skip this realm.
    pub reply: oneshot::Sender<Option<(String, String)>>,
}

/// Everything the fetcher and renderer need for the network side of a
/// crawl. Shared by every request; standards credentials can be added while
/// the crawl runs (answered prompts).
pub struct Network {
    pub settings: NetworkSettings,
    auth: RwLock<AuthConfig>,
    prompter: Option<mpsc::Sender<AuthPrompt>>,
    prompt_lock: tokio::sync::Mutex<()>,
    declined: Mutex<HashSet<String>>,
    /// Per-origin challenge cache, so later requests authenticate up front.
    known: Mutex<HashMap<String, Known>>,
}

#[derive(Clone)]
struct Known {
    challenge: Challenge,
    nc: u32,
}

impl Default for Network {
    fn default() -> Self {
        Self::new(NetworkSettings::default(), AuthConfig::default(), None)
    }
}

impl Network {
    pub fn new(settings: NetworkSettings, auth: AuthConfig, prompter: Option<mpsc::Sender<AuthPrompt>>) -> Self {
        Self {
            settings,
            auth: RwLock::new(auth),
            prompter,
            prompt_lock: tokio::sync::Mutex::new(()),
            declined: Mutex::default(),
            known: Mutex::default(),
        }
    }

    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn auth(&self) -> AuthConfig {
        self.auth.read().map(|a| a.clone()).unwrap_or_default()
    }

    pub fn cookie_header(&self, url: &Url) -> Option<String> {
        self.auth.read().ok().and_then(|a| a.cookie_header(url))
    }

    fn credential(&self, url: &Url) -> Option<Credential> {
        self.auth.read().ok().and_then(|a| a.standards.iter().find(|c| c.matches(url)).cloned())
    }

    /// The Authorization header to send up front, when this origin has
    /// challenged before and we hold credentials for it.
    pub fn preemptive(&self, url: &Url, method: &str) -> Option<String> {
        let cred = self.credential(url)?;
        let mut known = self.known.lock().ok()?;
        let k = known.get_mut(&url.origin().ascii_serialization())?;
        k.nc += 1;
        Some(k.challenge.authorization(&cred, method, &request_uri(url), k.nc))
    }

    /// After a 401: the Authorization header to retry with, if any. Asks the
    /// user when there are no stored credentials and prompts are on.
    pub async fn respond(&self, url: &Url, method: &str, www_authenticate: &[String]) -> Option<String> {
        let challenge = www_authenticate.iter().flat_map(|h| parse_challenges(h)).find(|c| c.supported())?;
        let origin = url.origin().ascii_serialization();
        let cred = match self.credential(url) {
            Some(c) => c,
            None => self.ask(url, &origin, &challenge).await?,
        };
        let mut known = self.known.lock().ok()?;
        known.insert(origin.clone(), Known { challenge: challenge.clone(), nc: 1 });
        Some(challenge.authorization(&cred, method, &request_uri(url), 1))
    }

    async fn ask(&self, url: &Url, origin: &str, challenge: &Challenge) -> Option<Credential> {
        let prompter = self.prompter.as_ref()?;
        if !self.auth.read().map(|a| a.prompt).unwrap_or(false) {
            return None;
        }
        let key = format!("{origin} {}", challenge.realm());
        if self.declined.lock().ok()?.contains(&key) {
            return None;
        }
        // One prompt at a time; another request may have been answered meanwhile.
        let _g = self.prompt_lock.lock().await;
        if let Some(c) = self.credential(url) {
            return Some(c);
        }
        if self.declined.lock().ok()?.contains(&key) {
            return None;
        }
        let (tx, rx) = oneshot::channel();
        let prompt = AuthPrompt {
            challenge: AuthChallenge { url: url.to_string(), realm: challenge.realm().to_string(), scheme: challenge.scheme.clone() },
            reply: tx,
        };
        prompter.send(prompt).await.ok()?;
        match tokio::time::timeout(std::time::Duration::from_secs(600), rx).await {
            Ok(Ok(Some((username, password)))) => {
                let cred = Credential { url: origin.to_string(), username, password };
                if let Ok(mut a) = self.auth.write() {
                    a.standards.push(cred.clone());
                }
                Some(cred)
            }
            _ => {
                if let Ok(mut d) = self.declined.lock() {
                    d.insert(key);
                }
                None
            }
        }
    }
}

fn request_uri(url: &Url) -> String {
    match url.query() {
        Some(q) => format!("{}?{q}", url.path()),
        None => url.path().to_string(),
    }
}

/// One challenge from a WWW-Authenticate header.
#[derive(Debug, Clone, PartialEq)]
pub struct Challenge {
    pub scheme: String,
    pub params: HashMap<String, String>,
}

impl Challenge {
    fn realm(&self) -> &str {
        self.params.get("realm").map(String::as_str).unwrap_or("")
    }

    fn supported(&self) -> bool {
        let algo = self.params.get("algorithm").map(|a| a.to_ascii_uppercase()).unwrap_or_else(|| "MD5".into());
        self.scheme.eq_ignore_ascii_case("basic")
            || (self.scheme.eq_ignore_ascii_case("digest") && matches!(algo.as_str(), "MD5" | "MD5-SESS" | "SHA-256" | "SHA-256-SESS"))
    }

    /// The Authorization header value for this challenge.
    pub fn authorization(&self, cred: &Credential, method: &str, uri: &str, nc: u32) -> String {
        use base64::Engine;
        if self.scheme.eq_ignore_ascii_case("basic") {
            let raw = format!("{}:{}", cred.username, cred.password);
            return format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(raw));
        }
        let cnonce = format!("{:016x}", rand_u64());
        self.digest(cred, method, uri, nc, &cnonce)
    }

    fn digest(&self, cred: &Credential, method: &str, uri: &str, nc: u32, cnonce: &str) -> String {
        let p = |k: &str| self.params.get(k).cloned().unwrap_or_default();
        let realm = p("realm");
        let nonce = p("nonce");
        let algorithm = self.params.get("algorithm").cloned().unwrap_or_else(|| "MD5".into());
        let upper = algorithm.to_ascii_uppercase();
        let h = |s: &str| -> String {
            if upper.starts_with("SHA-256") {
                format!("{:x}", sha2::Sha256::digest(s.as_bytes()))
            } else {
                format!("{:x}", md5::compute(s.as_bytes()))
            }
        };
        let mut ha1 = h(&format!("{}:{realm}:{}", cred.username, cred.password));
        if upper.ends_with("-SESS") {
            ha1 = h(&format!("{ha1}:{nonce}:{cnonce}"));
        }
        let ha2 = h(&format!("{method}:{uri}"));
        let qop = p("qop").split(',').map(str::trim).find(|q| q.eq_ignore_ascii_case("auth")).map(str::to_string);
        let nc = format!("{nc:08x}");
        let response = match &qop {
            Some(q) => h(&format!("{ha1}:{nonce}:{nc}:{cnonce}:{q}:{ha2}")),
            None => h(&format!("{ha1}:{nonce}:{ha2}")),
        };
        let mut out = format!(r#"Digest username="{}", realm="{realm}", nonce="{nonce}", uri="{uri}", response="{response}""#, cred.username);
        if self.params.contains_key("algorithm") {
            out.push_str(&format!(", algorithm={algorithm}"));
        }
        if let Some(opaque) = self.params.get("opaque") {
            out.push_str(&format!(r#", opaque="{opaque}""#));
        }
        if let Some(q) = qop {
            out.push_str(&format!(r#", qop={q}, nc={nc}, cnonce="{cnonce}""#));
        }
        out
    }
}

fn rand_u64() -> u64 {
    let mut b = [0u8; 8];
    getrandom::getrandom(&mut b).ok();
    u64::from_le_bytes(b)
}

/// Parse a WWW-Authenticate header: one or more `Scheme k=v, k="v"` challenges.
pub fn parse_challenges(header: &str) -> Vec<Challenge> {
    let mut out: Vec<Challenge> = Vec::new();
    let s = header.trim();
    let bytes: Vec<char> = s.chars().collect();
    let mut i = 0;
    let skip_ws = |i: &mut usize| {
        while *i < bytes.len() && (bytes[*i].is_whitespace() || bytes[*i] == ',') {
            *i += 1;
        }
    };
    while i < bytes.len() {
        skip_ws(&mut i);
        let start = i;
        while i < bytes.len() && !bytes[i].is_whitespace() && bytes[i] != '=' && bytes[i] != ',' {
            i += 1;
        }
        let token: String = bytes[start..i].iter().collect();
        if token.is_empty() {
            break;
        }
        let mut j = i;
        while j < bytes.len() && bytes[j] == ' ' {
            j += 1;
        }
        if j < bytes.len() && bytes[j] == '=' && !out.is_empty() {
            // A parameter of the current challenge.
            i = j + 1;
            while i < bytes.len() && bytes[i] == ' ' {
                i += 1;
            }
            let value = if i < bytes.len() && bytes[i] == '"' {
                i += 1;
                let mut v = String::new();
                while i < bytes.len() && bytes[i] != '"' {
                    if bytes[i] == '\\' && i + 1 < bytes.len() {
                        i += 1;
                    }
                    v.push(bytes[i]);
                    i += 1;
                }
                i += 1;
                v
            } else {
                let vs = i;
                while i < bytes.len() && bytes[i] != ',' {
                    i += 1;
                }
                bytes[vs..i].iter().collect::<String>().trim().to_string()
            };
            if let Some(c) = out.last_mut() {
                c.params.insert(token.to_ascii_lowercase(), value);
            }
        } else {
            out.push(Challenge { scheme: token, params: HashMap::new() });
        }
    }
    out
}

// ------------------------------------------------------------------ export

const KDF_ITERATIONS: u32 = 600_000;

#[derive(Serialize, Deserialize)]
struct Sealed {
    format: String,
    kdf: String,
    iterations: u32,
    salt: String,
    nonce: String,
    ciphertext: String,
}

/// Authentication > Export: the auth config as JSON, encrypted with
/// AES-256-GCM under a PBKDF2-SHA256 key from `password`.
pub fn export_encrypted(auth: &AuthConfig, password: &str) -> anyhow::Result<String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use base64::Engine;
    anyhow::ensure!(!password.is_empty(), "a password is needed to encrypt the authentication file");
    let b64 = base64::engine::general_purpose::STANDARD;
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut salt).map_err(|e| anyhow::anyhow!("{e}"))?;
    getrandom::getrandom(&mut nonce).map_err(|e| anyhow::anyhow!("{e}"))?;
    let key = derive_key(password, &salt, KDF_ITERATIONS);
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow::anyhow!("{e}"))?;
    let plain = serde_json::to_vec(auth)?;
    let ct = cipher.encrypt(aes_gcm::Nonce::from_slice(&nonce), plain.as_slice()).map_err(|_| anyhow::anyhow!("encryption failed"))?;
    let sealed = Sealed {
        format: "screaming-toad-auth/1".into(),
        kdf: "pbkdf2-sha256".into(),
        iterations: KDF_ITERATIONS,
        salt: b64.encode(salt),
        nonce: b64.encode(nonce),
        ciphertext: b64.encode(ct),
    };
    Ok(serde_json::to_string_pretty(&sealed)?)
}

/// Authentication > Import (and the CLI's `--auth-config`).
pub fn import_encrypted(text: &str, password: &str) -> anyhow::Result<AuthConfig> {
    use aes_gcm::aead::{Aead, KeyInit};
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD;
    let sealed: Sealed = serde_json::from_str(text).map_err(|_| anyhow::anyhow!("not a Screaming Toad authentication file"))?;
    anyhow::ensure!(sealed.format == "screaming-toad-auth/1" && sealed.kdf == "pbkdf2-sha256", "unsupported authentication file format");
    let key = derive_key(password, &b64.decode(&sealed.salt)?, sealed.iterations);
    let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).map_err(|e| anyhow::anyhow!("{e}"))?;
    let nonce = b64.decode(&sealed.nonce)?;
    anyhow::ensure!(nonce.len() == 12, "corrupt authentication file");
    let plain = cipher
        .decrypt(aes_gcm::Nonce::from_slice(&nonce), b64.decode(&sealed.ciphertext)?.as_slice())
        .map_err(|_| anyhow::anyhow!("wrong password, or the file is corrupt"))?;
    Ok(serde_json::from_slice(&plain)?)
}

fn derive_key(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc2617_digest_example() {
        let c = &parse_challenges(r#"Digest realm="testrealm@host.com", qop="auth,auth-int", nonce="dcd98b7102dd2f0e8b11d0f600bfb0c093", opaque="5ccc069c403ebaf9f0171e9517f40e41""#)[0];
        let cred = Credential { url: "host.com".into(), username: "Mufasa".into(), password: "Circle Of Life".into() };
        let h = c.digest(&cred, "GET", "/dir/index.html", 1, "0a4f113b");
        assert!(h.contains(r#"response="6629fae49393a05397450978507c4ef1""#), "{h}");
        assert!(h.contains("qop=auth, nc=00000001"), "{h}");
        assert!(h.contains(r#"opaque="5ccc069c403ebaf9f0171e9517f40e41""#));
    }

    #[test]
    fn rfc7616_sha256_example() {
        let c = &parse_challenges(r#"Digest realm="http-auth@example.org", qop="auth, auth-int", algorithm=SHA-256, nonce="7ypf/xlj9XXwfDPEoM4URrv/xwf94BcCAzFZH4GiTo0v", opaque="FQhe/qaU925kfnzjCev0ciny7QMkPqMAFRtzCUYo5tdS""#)[0];
        let cred = Credential { url: "example.org".into(), username: "Mufasa".into(), password: "Circle of Life".into() };
        let h = c.digest(&cred, "GET", "/dir/index.html", 1, "f2/wE4q74E6zIJEtWaHKaf5wv/H5QzzpXusqGemxURZJ");
        assert!(h.contains(r#"response="753927fa0e85d155564e2e272a28d1802ca10daf4496794697cf8db5856cb6c1""#), "{h}");
    }

    #[test]
    fn parses_multiple_challenges() {
        let cs = parse_challenges(r#"Negotiate, Basic realm="Staging", charset="UTF-8""#);
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[1].scheme, "Basic");
        assert_eq!(cs[1].realm(), "Staging");
        assert!(cs[1].supported() && !cs[0].supported());
    }

    #[test]
    fn credential_and_cookie_matching() {
        let u = Url::parse("https://shop.example.com:8443/a/b").unwrap();
        assert!(Credential { url: "shop.example.com:8443".into(), ..Default::default() }.matches(&u));
        assert!(Credential { url: "shop.example.com".into(), ..Default::default() }.matches(&u));
        assert!(Credential { url: "https://shop.example.com:8443/a/".into(), ..Default::default() }.matches(&u));
        assert!(!Credential { url: "example.com".into(), ..Default::default() }.matches(&u));
        let c = AuthCookie { name: "s".into(), value: "1".into(), domain: ".example.com".into(), path: "/a".into(), secure: true, ..Default::default() };
        assert!(c.matches(&u));
        assert!(!c.matches(&Url::parse("http://shop.example.com/a").unwrap()), "secure cookie over http");
        assert!(!c.matches(&Url::parse("https://shop.example.com/z").unwrap()), "other path");
        let host_only = AuthCookie { domain: "example.com".into(), host_only: true, ..c.clone() };
        assert!(!host_only.matches(&u));
    }

    #[test]
    fn encrypted_export_round_trip() {
        let auth = AuthConfig {
            prompt: false,
            standards: vec![Credential { url: "example.com".into(), username: "u".into(), password: "p".into() }],
            forms: vec![FormLogin { name: "Shop".into(), login_url: "https://example.com/login".into(), cookies: vec![] }],
        };
        let sealed = export_encrypted(&auth, "secret").unwrap();
        assert!(!sealed.contains("example.com"), "credentials are not readable in the file");
        assert_eq!(import_encrypted(&sealed, "secret").unwrap(), auth);
        assert!(import_encrypted(&sealed, "wrong").is_err());
    }

    #[test]
    fn proxy_parse() {
        let p = ProxyConfig::parse("http://bob:p%40ss@10.0.0.1:3128").unwrap();
        assert_eq!((p.host.as_str(), p.port, p.username.as_str(), p.password.as_str()), ("10.0.0.1", 3128, "bob", "p@ss"));
        assert_eq!(p.url().unwrap(), "http://10.0.0.1:3128");
        assert_eq!(ProxyConfig::parse("socks5://h:1080").unwrap().url().unwrap(), "socks5h://h:1080");
    }
}
