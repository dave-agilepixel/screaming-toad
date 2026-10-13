//! Crawl Analysis: work that needs the whole crawl.

use rusqlite::{params, Connection};
use std::collections::{HashMap, HashSet};
use toad_core::{minhash, CrawlConfig};

/// `full`: also the Crawl Analysis > Configure areas (Link Score, near
/// duplicates); otherwise only the link counts and redirect chains.
pub fn run(conn: &mut Connection, cfg: &CrawlConfig, full: bool) -> anyhow::Result<()> {
    let tx = conn.transaction()?;

    tx.execute_batch(
        "UPDATE links SET target_id = (SELECT u.id FROM urls u WHERE u.address = links.target)
         WHERE target_id IS NULL;

         UPDATE urls SET
           inlinks = (SELECT COUNT(*) FROM links l WHERE l.target_id = urls.id),
           unique_inlinks = (SELECT COUNT(DISTINCT l.source_id) FROM links l WHERE l.target_id = urls.id AND l.kind = 'Hyperlink'),
           outlinks = (SELECT COUNT(*) FROM links l WHERE l.source_id = urls.id AND l.internal = 1 AND l.kind = 'Hyperlink'),
           unique_outlinks = (SELECT COUNT(DISTINCT l.target) FROM links l WHERE l.source_id = urls.id AND l.internal = 1 AND l.kind = 'Hyperlink'),
           external_outlinks = (SELECT COUNT(*) FROM links l WHERE l.source_id = urls.id AND l.internal = 0 AND l.kind = 'Hyperlink'),
           unique_external_outlinks = (SELECT COUNT(DISTINCT l.target) FROM links l WHERE l.source_id = urls.id AND l.internal = 0 AND l.kind = 'Hyperlink');",
    )?;

    let total_pages: i64 = tx.query_row(
        "SELECT COUNT(*) FROM urls WHERE is_internal = 1 AND is_html = 1 AND status_code = 200",
        [],
        |r| r.get(0),
    )?;
    if total_pages > 0 {
        tx.execute(
            "UPDATE urls SET pct_of_total = round(unique_inlinks * 100.0 / ?1, 2) WHERE is_internal = 1",
            params![total_pages],
        )?;
    }

    redirect_chains(&tx)?;
    let areas = &cfg.crawl_analysis;
    if full && areas.link_score {
        link_score(&tx)?;
    }
    if full && areas.content && cfg.near_duplicates {
        near_duplicates(&tx, cfg.near_duplicate_threshold, cfg.near_duplicates_indexable_only)?;
    }
    tx.commit()?;
    Ok(())
}

/// Follow each redirect to its final destination, counting hops and
/// detecting loops.
fn redirect_chains(conn: &Connection) -> anyhow::Result<()> {
    let mut next: HashMap<String, String> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT address, redirect_url FROM urls WHERE redirect_url IS NOT NULL")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (from, to) = row?;
            next.insert(from, to);
        }
    }
    let mut update = conn.prepare(
        "UPDATE urls SET redirect_chain_length = ?2, redirect_loop = ?3, final_destination = ?4 WHERE address = ?1",
    )?;
    for (start, first) in &next {
        let mut visited: HashSet<&str> = HashSet::from([start.as_str()]);
        let mut hops = 1;
        let mut current = first.as_str();
        let mut looped = false;
        loop {
            if !visited.insert(current) {
                looped = true;
                break;
            }
            match next.get(current) {
                Some(n) if hops < 100 => {
                    hops += 1;
                    current = n.as_str();
                }
                Some(_) => {
                    looped = true;
                    break;
                }
                None => break,
            }
        }
        update.execute(params![start, hops, looped, current])?;
    }
    Ok(())
}

/// Link Score: internal PageRank (d = 0.85, 10 iterations) over followed
/// internal hyperlinks and redirects. Nofollow links count towards a page's
/// outlinks but pass nothing on. Scaled 1–100 on a log scale.
fn link_score(conn: &Connection) -> anyhow::Result<()> {
    const DAMPING: f64 = 0.85;
    const ITERATIONS: usize = 10;

    let mut ids: Vec<i64> = Vec::new();
    {
        let mut stmt = conn.prepare(
            "SELECT id FROM urls WHERE is_internal = 1 AND ((is_html = 1 AND status_code = 200) OR status_code BETWEEN 300 AND 399)",
        )?;
        for id in stmt.query_map([], |r| r.get::<_, i64>(0))? {
            ids.push(id?);
        }
    }
    let n = ids.len();
    if n == 0 {
        return Ok(());
    }
    let index: HashMap<i64, usize> = ids.iter().enumerate().map(|(i, id)| (*id, i)).collect();

    let mut edges: Vec<(usize, usize, bool)> = Vec::new();
    let mut out_degree = vec![0usize; n];
    {
        let mut stmt = conn.prepare(
            "SELECT source_id, target_id, follow FROM links
             WHERE target_id IS NOT NULL AND internal = 1 AND kind IN ('Hyperlink', 'Redirect')",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, bool>(2)?)))?;
        for row in rows {
            let (s, t, follow) = row?;
            if let (Some(&si), Some(&ti)) = (index.get(&s), index.get(&t)) {
                if si != ti {
                    out_degree[si] += 1;
                    edges.push((si, ti, follow));
                }
            }
        }
    }

    let mut rank = vec![1.0 / n as f64; n];
    for _ in 0..ITERATIONS {
        let mut next = vec![(1.0 - DAMPING) / n as f64; n];
        for &(s, t, follow) in &edges {
            if follow {
                next[t] += DAMPING * rank[s] / out_degree[s] as f64;
            }
        }
        rank = next;
    }

    let logs: Vec<f64> = rank.iter().map(|r| r.max(f64::MIN_POSITIVE).ln()).collect();
    let (min, max) = logs.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let mut update = conn.prepare("UPDATE urls SET link_score = ?2 WHERE id = ?1")?;
    for (i, id) in ids.iter().enumerate() {
        let score = if max > min { 1.0 + 99.0 * (logs[i] - min) / (max - min) } else { 100.0 };
        update.execute(params![id, (score * 10.0).round() / 10.0])?;
    }
    Ok(())
}

/// Near duplicates: MinHash signatures banded for locality-sensitive
/// hashing (32 bands of 4 rows), candidate pairs checked against the
/// threshold. Exact duplicates (same hash) are left to the Exact filter.
fn near_duplicates(conn: &Connection, threshold: f64, indexable_only: bool) -> anyhow::Result<()> {
    const BANDS: usize = 32;
    const ROWS: usize = minhash::SIGNATURE_LEN / BANDS;
    /// Buckets bigger than this are template boilerplate; skip them rather
    /// than compare every pair.
    const MAX_BUCKET: usize = 500;

    conn.execute("DELETE FROM near_duplicates", [])?;
    conn.execute("UPDATE urls SET closest_similarity = NULL, near_duplicate_count = 0", [])?;

    let mut pages: Vec<(i64, Option<String>, Vec<u32>)> = Vec::new();
    {
        let sql = format!(
            "SELECT id, hash, minhash FROM urls WHERE is_internal = 1 AND is_html = 1 AND status_code = 200 AND minhash IS NOT NULL{}",
            if indexable_only { " AND indexable = 1" } else { "" }
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, Option<String>>(1)?, r.get::<_, Vec<u8>>(2)?)))?;
        for row in rows {
            let (id, hash, blob) = row?;
            let sig = minhash::from_bytes(&blob);
            if sig.len() == minhash::SIGNATURE_LEN {
                pages.push((id, hash, sig));
            }
        }
    }

    let mut candidates: HashSet<(usize, usize)> = HashSet::new();
    for band in 0..BANDS {
        let mut buckets: HashMap<&[u32], Vec<usize>> = HashMap::new();
        for (i, (_, _, sig)) in pages.iter().enumerate() {
            buckets.entry(&sig[band * ROWS..(band + 1) * ROWS]).or_default().push(i);
        }
        for members in buckets.values().filter(|m| m.len() > 1 && m.len() <= MAX_BUCKET) {
            for (k, &a) in members.iter().enumerate() {
                for &b in &members[k + 1..] {
                    candidates.insert((a, b));
                }
            }
        }
    }

    let mut best: HashMap<usize, (f64, u32)> = HashMap::new();
    let mut insert = conn.prepare("INSERT INTO near_duplicates (a_id, b_id, similarity) VALUES (?1, ?2, ?3)")?;
    for (a, b) in candidates {
        let (ida, hasha, siga) = &pages[a];
        let (idb, hashb, sigb) = &pages[b];
        if hasha.is_some() && hasha == hashb {
            continue;
        }
        let sim = minhash::similarity(siga, sigb);
        if sim < threshold {
            continue;
        }
        insert.execute(params![ida, idb, sim])?;
        for i in [a, b] {
            let e = best.entry(i).or_insert((0.0, 0));
            e.0 = e.0.max(sim);
            e.1 += 1;
        }
    }
    let mut update = conn.prepare("UPDATE urls SET closest_similarity = ?2, near_duplicate_count = ?3 WHERE id = ?1")?;
    for (i, (sim, count)) in best {
        update.execute(params![pages[i].0, sim, count])?;
    }
    Ok(())
}
