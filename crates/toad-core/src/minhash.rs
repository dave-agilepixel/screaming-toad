//! MinHash signatures for near-duplicate detection. Pages are compared on
//! word 3-shingles of their content-area text; the share of matching
//! signature slots estimates the Jaccard similarity of the shingle sets.

pub const SIGNATURE_LEN: usize = 128;
const SHINGLE_WORDS: usize = 3;

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e3779b97f4a7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}

/// Empty when the text is too short to shingle.
pub fn signature(text: &str) -> Vec<u32> {
    let words: Vec<String> = text
        .split_whitespace()
        .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    if words.len() < SHINGLE_WORDS {
        return vec![];
    }
    let mut sig = vec![u32::MAX; SIGNATURE_LEN];
    for shingle in words.windows(SHINGLE_WORDS) {
        let base = fnv1a(shingle.join(" ").as_bytes());
        for (i, slot) in sig.iter_mut().enumerate() {
            let h = splitmix(base ^ (i as u64).wrapping_mul(0x9e3779b97f4a7c15)) as u32;
            if h < *slot {
                *slot = h;
            }
        }
    }
    sig
}

pub fn similarity(a: &[u32], b: &[u32]) -> f64 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b).filter(|(x, y)| x == y).count() as f64 / a.len() as f64
}

pub fn to_bytes(sig: &[u32]) -> Vec<u8> {
    sig.iter().flat_map(|v| v.to_le_bytes()).collect()
}

pub fn from_bytes(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks_exact(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_and_far() {
        let base = "the quick brown fox jumps over the lazy dog and runs into the forest to find the other foxes playing near the river bank all afternoon";
        let near = base.replace("afternoon", "evening");
        let far = "completely different content about toads amphibians and the ponds where they live and breed every spring in the united kingdom";
        let s = signature(base);
        assert_eq!(s.len(), SIGNATURE_LEN);
        assert!(similarity(&s, &signature(&near)) > 0.8);
        assert!(similarity(&s, &signature(far)) < 0.2);
        assert_eq!(from_bytes(&to_bytes(&s)), s);
    }
}
