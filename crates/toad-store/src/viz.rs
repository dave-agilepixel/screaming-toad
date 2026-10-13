//! Visualisations: graph data for the crawl and directory tree graphs, the
//! force-directed diagrams, the word clouds and the content cluster diagram.
//! The UI lays them out (d3); this module only shapes the data.

use crate::Store;
use rusqlite::params;
use serde::Serialize;
use std::collections::{BTreeMap, HashMap};

/// A page (or, in the directory tree, a folder) in a graph.
#[derive(Debug, Clone, Serialize, Default)]
pub struct VizNode {
    pub id: i64,
    /// The node's parent in a tree (None for the root).
    pub parent: Option<i64>,
    pub url: String,
    /// Short text: the last path segment, or the host for the root.
    pub label: String,
    pub depth: u32,
    pub status: i64,
    pub indexable: bool,
    pub link_score: Option<f64>,
    pub unique_inlinks: i64,
    pub segment: Option<String>,
    /// Directory tree: a folder that wasn't itself crawled.
    pub virtual_node: bool,
    /// Content clusters: the cluster number.
    pub cluster: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct VizGraph {
    pub nodes: Vec<VizNode>,
    /// Extra (non-tree) edges, e.g. content cluster neighbours: (from, to, weight).
    pub edges: Vec<(i64, i64, f64)>,
    /// More nodes existed than the limit.
    pub truncated: bool,
    /// Content clusters: a label per cluster (common title words).
    pub cluster_labels: Vec<String>,
}

fn label_of(url: &str) -> String {
    match url::Url::parse(url) {
        Ok(u) => {
            let segs: Vec<&str> = u.path().split('/').filter(|s| !s.is_empty()).collect();
            match segs.last() {
                Some(s) => {
                    let mut l = s.to_string();
                    if u.path().ends_with('/') {
                        l.push('/');
                    }
                    l
                }
                None => u.host_str().unwrap_or(url).to_string(),
            }
        }
        Err(_) => url.to_string(),
    }
}

impl Store {
    fn viz_rows(&self, limit: usize) -> anyhow::Result<(Vec<VizNode>, bool)> {
        let segment = crate::segments::case_sql(self.config()).unwrap_or_else(|| "NULL".into());
        // Internal URLs that are pages or redirects (not images, CSS, JS...), breadth-first.
        let sql = format!(
            "SELECT u.id, u.address, coalesce(u.depth, 0), u.status_code, u.indexable, u.link_score, coalesce(u.unique_inlinks, 0), {segment}
             FROM urls u
             WHERE u.is_internal = 1 AND (u.is_html = 1 OR u.status_code = 0 OR u.status_code >= 300)
             ORDER BY u.depth, u.crawl_order LIMIT ?1"
        );
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![limit as i64 + 1], |r| {
            Ok(VizNode {
                id: r.get(0)?,
                url: r.get(1)?,
                depth: r.get::<_, i64>(2)? as u32,
                status: r.get(3)?,
                indexable: r.get(4)?,
                link_score: r.get(5)?,
                unique_inlinks: r.get(6)?,
                segment: r.get(7)?,
                ..Default::default()
            })
        })?;
        let mut nodes: Vec<VizNode> = rows.collect::<Result<_, _>>()?;
        let truncated = nodes.len() > limit;
        nodes.truncate(limit);
        for n in &mut nodes {
            n.label = label_of(&n.url);
        }
        Ok((nodes, truncated))
    }

    /// Crawl Tree Graph / Force-Directed Crawl Diagram: each URL hangs off
    /// the earliest-crawled page one click shallower that links to it, so the
    /// tree is the shortest crawl path from the start URL.
    pub fn crawl_tree(&self, limit: usize) -> anyhow::Result<VizGraph> {
        let (mut nodes, truncated) = self.viz_rows(limit)?;
        let ids: std::collections::HashSet<i64> = nodes.iter().map(|n| n.id).collect();
        let mut stmt = self.conn.prepare(
            "SELECT s.id FROM links l JOIN urls s ON s.id = l.source_id
             WHERE l.target_id = ?1 AND s.depth = ?2 AND l.kind IN ('Hyperlink', 'Redirect', 'Canonical', 'Meta Refresh')
             ORDER BY s.crawl_order LIMIT 1",
        )?;
        for n in &mut nodes {
            if n.depth == 0 {
                continue;
            }
            let parent: Option<i64> = stmt.query_row(params![n.id, n.depth as i64 - 1], |r| r.get(0)).ok();
            n.parent = parent.filter(|p| ids.contains(p));
        }
        // Several seeds (List mode, orphans) can be depth 0: give the tree one root.
        Ok(VizGraph { nodes: with_single_root(nodes), edges: vec![], truncated, cluster_labels: vec![] })
    }

    /// Directory Tree Graph / Force-Directed Directory Tree Diagram: the URL
    /// path hierarchy, with folders that weren't crawled as virtual nodes.
    pub fn directory_tree(&self, limit: usize) -> anyhow::Result<VizGraph> {
        let (pages, truncated) = self.viz_rows(limit)?;
        let mut by_key: HashMap<String, usize> = HashMap::new();
        let mut nodes: Vec<VizNode> = Vec::new();
        let mut next_virtual = -1i64;
        // Key: origin + path prefix ending in '/', or the page's own path.
        let key_of = |u: &url::Url, segs: &[&str], n: usize, folder: bool| {
            let mut k = format!("{}/", u.origin().ascii_serialization());
            if n > 0 {
                k.push_str(&segs[..n].join("/"));
                if folder {
                    k.push('/');
                }
            }
            k
        };
        let mut page_nodes: Vec<VizNode> = pages;
        page_nodes.sort_by_key(|n| n.url.len());
        for page in page_nodes {
            let Ok(u) = url::Url::parse(&page.url) else { continue };
            let path = u.path().to_string();
            let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
            let is_folder = path.ends_with('/');
            // Ensure every ancestor folder exists.
            let mut parent: Option<i64> = None;
            // The folders above the page: the root, then /a/, /a/b/... (a
            // folder page's own folder is the page itself, below).
            for folder_len in 0..segs.len() {
                let key = key_of(&u, &segs, folder_len, true);
                if let Some(&idx) = by_key.get(&key) {
                    parent = Some(nodes[idx].id);
                    continue;
                }
                let n = VizNode {
                    id: next_virtual,
                    parent,
                    url: key.clone(),
                    label: if folder_len == 0 { u.host_str().unwrap_or("").to_string() } else { format!("{}/", segs[folder_len - 1]) },
                    depth: folder_len as u32,
                    virtual_node: true,
                    indexable: true,
                    ..Default::default()
                };
                next_virtual -= 1;
                by_key.insert(key, nodes.len());
                parent = Some(n.id);
                nodes.push(n);
            }
            let own_key = key_of(&u, &segs, segs.len(), is_folder);
            match by_key.get(&own_key) {
                // A crawled folder page replaces its virtual node.
                Some(&idx) if nodes[idx].virtual_node => {
                    let keep_parent = nodes[idx].parent;
                    let keep_id = nodes[idx].id;
                    let depth = nodes[idx].depth;
                    nodes[idx] = VizNode { parent: keep_parent, depth, label: nodes[idx].label.clone(), ..page };
                    // Children already point at the virtual id; repoint them.
                    let real = nodes[idx].id;
                    for n in nodes.iter_mut() {
                        if n.parent == Some(keep_id) {
                            n.parent = Some(real);
                        }
                    }
                }
                Some(_) => {}
                None => {
                    by_key.insert(own_key, nodes.len());
                    let depth = segs.len() as u32;
                    nodes.push(VizNode { parent, depth, ..page });
                }
            }
        }
        Ok(VizGraph { nodes: with_single_root(nodes), edges: vec![], truncated, cluster_labels: vec![] })
    }

    /// Inlink Anchor Text Word Cloud: words in the anchor text of links to `url`.
    pub fn anchor_words(&self, url: &str, limit: usize) -> anyhow::Result<Vec<(String, u32)>> {
        let mut stmt = self.conn.prepare("SELECT coalesce(anchor, ''), coalesce(alt, '') FROM links WHERE target = ?1")?;
        let mut text = String::new();
        for row in stmt.query_map(params![url], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (a, alt) = row?;
            text.push(' ');
            text.push_str(if a.trim().is_empty() { &alt } else { &a });
        }
        Ok(word_counts(&text, limit))
    }

    /// Body Text Word Cloud: words in a page's text (needs Store HTML, or
    /// page text kept for AI/embeddings).
    pub fn body_words(&self, url: &str, limit: usize) -> anyhow::Result<Vec<(String, u32)>> {
        use toad_core::api_config::AiContent;
        let text = match self.content_for(url, AiContent::PageText)? {
            Some(t) => t,
            None => match self.content_for(url, AiContent::ContentArea)? {
                Some(t) => t,
                None => {
                    let html = self.page_source(url)?.ok_or_else(|| anyhow::anyhow!("no text stored for this page: switch on Configuration > Spider > Extraction > Store HTML and crawl again"))?;
                    strip_tags(&html)
                }
            },
        };
        Ok(word_counts(&text, limit))
    }

    /// Content Cluster Diagram: pages grouped by their embeddings (Content >
    /// Embeddings): spherical k-means clusters, labelled by common title
    /// words, with edges to each page's nearest neighbours.
    pub fn content_clusters(&self, limit: usize, neighbours: usize) -> anyhow::Result<VizGraph> {
        let mut vecs = self.embeddings()?;
        anyhow::ensure!(!vecs.is_empty(), "no embeddings in this crawl: switch on Content > Embeddings and crawl (or Request API Data)");
        let truncated = vecs.len() > limit;
        vecs.truncate(limit);
        let n = vecs.len();
        let k = ((n as f64 / 2.0).sqrt().ceil() as usize).clamp(1, 12).min(n);
        let assignment = kmeans(&vecs.iter().map(|(_, v)| v.as_slice()).collect::<Vec<_>>(), k, 25);

        let mut nodes = Vec::with_capacity(n);
        let mut titles: Vec<Vec<String>> = vec![vec![]; k];
        let mut q = self.conn.prepare(
            "SELECT id, coalesce(depth, 0), status_code, indexable, link_score, coalesce(unique_inlinks, 0), coalesce(title_1, '') FROM urls WHERE address = ?1",
        )?;
        for (i, (url, _)) in vecs.iter().enumerate() {
            let row = q.query_row(params![url], |r| {
                Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?, r.get::<_, bool>(3)?, r.get::<_, Option<f64>>(4)?, r.get::<_, i64>(5)?, r.get::<_, String>(6)?))
            });
            let Ok((id, depth, status, indexable, link_score, inlinks, title)) = row else { continue };
            titles[assignment[i]].push(title);
            nodes.push(VizNode {
                id,
                url: url.clone(),
                label: label_of(url),
                depth: depth as u32,
                status,
                indexable,
                link_score,
                unique_inlinks: inlinks,
                cluster: Some(assignment[i]),
                ..Default::default()
            });
        }
        // Nearest neighbours by cosine (vectors are unit length).
        let mut edges = Vec::new();
        for i in 0..vecs.len() {
            let mut sims: Vec<(usize, f64)> = (0..vecs.len()).filter(|&j| j != i).map(|j| (j, crate::api::cosine(&vecs[i].1, &vecs[j].1))).collect();
            sims.sort_by(|a, b| b.1.total_cmp(&a.1));
            for (j, s) in sims.into_iter().take(neighbours) {
                if let (Some(a), Some(b)) = (nodes.iter().find(|n| n.url == vecs[i].0), nodes.iter().find(|n| n.url == vecs[j].0)) {
                    if a.id < b.id || !edges.iter().any(|(x, y, _)| *x == b.id && *y == a.id) {
                        edges.push((a.id, b.id, (s * 1000.0).round() / 1000.0));
                    }
                }
            }
        }
        let cluster_labels = titles.iter().map(|t| top_words(t, 3)).collect();
        Ok(VizGraph { nodes, edges, truncated, cluster_labels })
    }
}

/// Give a forest one root: extra roots hang off the first root (the start URL).
fn with_single_root(mut nodes: Vec<VizNode>) -> Vec<VizNode> {
    let root = nodes.iter().find(|n| n.parent.is_none()).map(|n| n.id);
    if let Some(r) = root {
        for n in nodes.iter_mut() {
            if n.parent.is_none() && n.id != r {
                n.parent = Some(r);
            }
        }
    }
    nodes
}

const STOP: &[&str] = &[
    "the", "and", "for", "are", "but", "not", "you", "all", "any", "can", "had", "her", "was", "one", "our", "out", "has", "have", "his", "how", "its",
    "may", "new", "now", "see", "two", "way", "who", "did", "get", "use", "with", "this", "that", "from", "they", "will", "your", "what", "when",
    "where", "which", "their", "there", "been", "were", "into", "more", "than", "then", "them", "these", "those", "some", "such", "also", "only",
    "over", "very", "just", "about", "after", "before", "here", "each", "other", "most", "much", "many", "should", "would", "could", "does",
    "being", "while", "because", "both", "between", "under", "again", "same", "own", "off", "too", "why", "yes", "via", "per", "les", "des", "der",
    "die", "und", "den", "das", "por", "para", "con", "una", "los", "las",
];

/// Word frequencies, lowercased, without stop words or 1–2 letter words.
pub fn word_counts(text: &str, limit: usize) -> Vec<(String, u32)> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for w in text.split(|c: char| !c.is_alphanumeric() && c != '\'' && c != '-') {
        let w = w.trim_matches(|c: char| c == '\'' || c == '-').to_lowercase();
        if w.chars().count() < 3 || w.chars().all(|c| c.is_ascii_digit()) || STOP.contains(&w.as_str()) {
            continue;
        }
        *counts.entry(w).or_default() += 1;
    }
    let mut v: Vec<(String, u32)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    v.truncate(limit);
    v
}

fn top_words(titles: &[String], n: usize) -> String {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for t in titles {
        let mut seen = std::collections::HashSet::new();
        for (w, _) in word_counts(t, 50) {
            if seen.insert(w.clone()) {
                *counts.entry(w).or_default() += 1;
            }
        }
    }
    let mut v: Vec<(String, u32)> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    let words: Vec<String> = v.into_iter().take(n).map(|(w, _)| w).collect();
    if words.is_empty() { "Untitled".into() } else { words.join(", ") }
}

fn strip_tags(html: &str) -> String {
    let doc = scraper::Html::parse_document(html);
    let sel = scraper::Selector::parse("body").expect("valid selector");
    let skip = ["script", "style", "noscript", "template"];
    let mut out = String::new();
    if let Some(body) = doc.select(&sel).next() {
        for node in body.descendants() {
            if let Some(t) = node.value().as_text() {
                let hidden = node.ancestors().any(|a| a.value().as_element().is_some_and(|e| skip.contains(&e.name())));
                if !hidden {
                    out.push_str(t);
                    out.push(' ');
                }
            }
        }
    }
    out
}

/// Spherical k-means (cosine) with deterministic farthest-point seeding.
fn kmeans(vecs: &[&[f32]], k: usize, iters: usize) -> Vec<usize> {
    let n = vecs.len();
    if n == 0 || k <= 1 {
        return vec![0; n];
    }
    let dims = vecs[0].len();
    let dot = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| *x as f64 * *y as f64).sum::<f64>();
    let mut centres: Vec<Vec<f32>> = vec![vecs[0].to_vec()];
    while centres.len() < k {
        // The point least similar to its nearest centre.
        let far = (0..n)
            .min_by(|&a, &b| {
                let sa = centres.iter().map(|c| dot(vecs[a], c)).fold(f64::MIN, f64::max);
                let sb = centres.iter().map(|c| dot(vecs[b], c)).fold(f64::MIN, f64::max);
                sa.total_cmp(&sb)
            })
            .unwrap_or(0);
        centres.push(vecs[far].to_vec());
    }
    let mut assign = vec![0usize; n];
    for _ in 0..iters {
        let mut changed = false;
        for i in 0..n {
            let best = (0..k).max_by(|&a, &b| dot(vecs[i], &centres[a]).total_cmp(&dot(vecs[i], &centres[b]))).unwrap_or(0);
            if best != assign[i] {
                assign[i] = best;
                changed = true;
            }
        }
        for (c, centre) in centres.iter_mut().enumerate() {
            let mut sum = vec![0f64; dims];
            for i in (0..n).filter(|&i| assign[i] == c) {
                for (s, x) in sum.iter_mut().zip(vecs[i]) {
                    *s += *x as f64;
                }
            }
            let norm = sum.iter().map(|x| x * x).sum::<f64>().sqrt();
            if norm > 0.0 {
                *centre = sum.iter().map(|x| (x / norm) as f32).collect();
            }
        }
        if !changed {
            break;
        }
    }
    assign
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_and_labels() {
        let w = word_counts("The toads and the TOADS hop; toads hop near ponds. 2024", 10);
        assert_eq!(w[0], ("toads".to_string(), 3));
        assert_eq!(w[1], ("hop".to_string(), 2));
        assert!(!w.iter().any(|(x, _)| x == "the" || x == "2024"));
        assert_eq!(label_of("https://x.com/a/b/"), "b/");
        assert_eq!(label_of("https://x.com/a/page"), "page");
        assert_eq!(label_of("https://x.com/"), "x.com");
    }

    #[test]
    fn kmeans_separates_groups() {
        let a = [1.0f32, 0.0, 0.0];
        let a2 = [0.95f32, 0.31, 0.0];
        let b = [0.0f32, 0.0, 1.0];
        let b2 = [0.0f32, 0.3, 0.95];
        let v: Vec<&[f32]> = vec![&a, &b, &a2, &b2];
        let as_ = kmeans(&v, 2, 10);
        assert_eq!(as_[0], as_[2]);
        assert_eq!(as_[1], as_[3]);
        assert_ne!(as_[0], as_[1]);
    }
}
