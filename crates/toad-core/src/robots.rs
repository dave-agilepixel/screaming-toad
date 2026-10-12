//! robots.txt parsing and matching, following Google's rules:
//! the most specific user-agent group applies (falling back to `*`),
//! `*` and `$` wildcards are supported, the longest matching rule wins,
//! and Allow wins a tie.

use regex::Regex;

#[derive(Debug, Clone)]
struct Rule {
    allow: bool,
    pattern: String,
    regex: Regex,
    line: String,
}

#[derive(Debug, Clone, Default)]
struct Group {
    agents: Vec<String>,
    rules: Vec<Rule>,
}

#[derive(Debug, Clone, Default)]
pub struct Robots {
    groups: Vec<Group>,
    pub sitemaps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allowed,
    /// Blocked, with the robots.txt line that matched.
    Blocked(String),
}

impl Robots {
    pub fn allow_all() -> Self {
        Self::default()
    }

    pub fn parse(text: &str) -> Self {
        let mut groups: Vec<Group> = Vec::new();
        let mut sitemaps = Vec::new();
        let mut current: Option<Group> = None;
        let mut last_was_agent = false;

        for raw in text.lines() {
            let line = raw.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once(':') else { continue };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match key.as_str() {
                "user-agent" => {
                    if !last_was_agent {
                        if let Some(g) = current.take() {
                            groups.push(g);
                        }
                        current = Some(Group::default());
                    }
                    if let Some(g) = current.as_mut() {
                        g.agents.push(value.to_ascii_lowercase());
                    }
                    last_was_agent = true;
                }
                "allow" | "disallow" => {
                    last_was_agent = false;
                    // An empty Disallow allows everything, so it adds no rule.
                    if value.is_empty() {
                        continue;
                    }
                    if let (Some(g), Some(regex)) = (current.as_mut(), pattern_regex(value)) {
                        g.rules.push(Rule {
                            allow: key == "allow",
                            pattern: value.to_string(),
                            regex,
                            line: line.to_string(),
                        });
                    }
                }
                "sitemap" => sitemaps.push(value.to_string()),
                _ => last_was_agent = false,
            }
        }
        if let Some(g) = current.take() {
            groups.push(g);
        }
        Self { groups, sitemaps }
    }

    /// `path` is the URL path plus query string.
    pub fn check(&self, user_agent: &str, path: &str) -> Verdict {
        let Some(group) = self.group_for(user_agent) else { return Verdict::Allowed };
        let mut best: Option<&Rule> = None;
        for rule in &group.rules {
            if !rule.regex.is_match(path) {
                continue;
            }
            best = match best {
                None => Some(rule),
                Some(b) => {
                    let (len, blen) = (rule.pattern.len(), b.pattern.len());
                    if len > blen || (len == blen && rule.allow && !b.allow) {
                        Some(rule)
                    } else {
                        Some(b)
                    }
                }
            };
        }
        match best {
            Some(r) if !r.allow => Verdict::Blocked(r.line.clone()),
            _ => Verdict::Allowed,
        }
    }

    /// A group naming our product token beats `*`. Among named groups the
    /// longest matching name wins.
    fn group_for(&self, user_agent: &str) -> Option<&Group> {
        let ua = user_agent.to_ascii_lowercase();
        let mut best: Option<(&Group, usize)> = None;
        for g in &self.groups {
            for a in &g.agents {
                if a != "*" && !a.is_empty() && ua.contains(a.as_str()) {
                    if best.map_or(true, |(_, l)| a.len() > l) {
                        best = Some((g, a.len()));
                    }
                }
            }
        }
        best.map(|(g, _)| g)
            .or_else(|| self.groups.iter().find(|g| g.agents.iter().any(|a| a == "*")))
    }
}

fn pattern_regex(pattern: &str) -> Option<Regex> {
    let (body, anchored) = match pattern.strip_suffix('$') {
        Some(b) => (b, true),
        None => (pattern, false),
    };
    let mut re = String::from("^");
    for (i, part) in body.split('*').enumerate() {
        if i > 0 {
            re.push_str(".*");
        }
        re.push_str(&regex::escape(part));
    }
    if anchored {
        re.push('$');
    }
    Regex::new(&re).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TXT: &str = "User-agent: *\nDisallow: /private/\nAllow: /private/ok\nDisallow: /*.pdf$\n\nUser-agent: Screaming Toad\nDisallow: /toad-only/\n";

    #[test]
    fn longest_match_and_wildcards() {
        let r = Robots::parse(TXT);
        assert_eq!(r.check("Googlebot", "/private/x"), Verdict::Blocked("Disallow: /private/".into()));
        assert_eq!(r.check("Googlebot", "/private/ok/1"), Verdict::Allowed);
        assert!(matches!(r.check("Googlebot", "/a/file.pdf"), Verdict::Blocked(_)));
        assert_eq!(r.check("Googlebot", "/a/file.pdf?x=1"), Verdict::Allowed);
    }

    #[test]
    fn specific_group_overrides_star() {
        let r = Robots::parse(TXT);
        assert!(matches!(r.check("Screaming Toad", "/toad-only/a"), Verdict::Blocked(_)));
        // The named group replaces `*` entirely, so /private/ is allowed for it.
        assert_eq!(r.check("Screaming Toad", "/private/x"), Verdict::Allowed);
    }
}
