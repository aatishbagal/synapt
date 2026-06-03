use crate::storage::FileRow;

/// Jaro-Winkler similarity in [0, 1], with prefix scaling p=0.1 over up to 4 chars.
pub fn jaro_winkler(s1: &str, s2: &str) -> f64 {
    let j = jaro(s1, s2);
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let mut prefix = 0usize;
    for i in 0..a.len().min(b.len()).min(4) {
        if a[i] == b[i] {
            prefix += 1;
        } else {
            break;
        }
    }
    j + prefix as f64 * 0.1 * (1.0 - j)
}

fn jaro(s1: &str, s2: &str) -> f64 {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let (la, lb) = (a.len(), b.len());
    if la == 0 && lb == 0 {
        return 1.0;
    }
    if la == 0 || lb == 0 {
        return 0.0;
    }
    let max_dist = (la.max(lb) / 2).saturating_sub(1);
    let mut a_match = vec![false; la];
    let mut b_match = vec![false; lb];
    let mut matches = 0usize;
    for i in 0..la {
        let start = i.saturating_sub(max_dist);
        let end = (i + max_dist + 1).min(lb);
        for j in start..end {
            if !b_match[j] && a[i] == b[j] {
                a_match[i] = true;
                b_match[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..la {
        if a_match[i] {
            while !b_match[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let t = (transpositions / 2) as f64;
    let m = matches as f64;
    (m / la as f64 + m / lb as f64 + (m - t) / m) / 3.0
}

/// Levenshtein edit distance over Unicode chars, using O(min(m, n)) space.
pub fn levenshtein(s1: &str, s2: &str) -> usize {
    let mut a: Vec<char> = s1.chars().collect();
    let mut b: Vec<char> = s2.chars().collect();
    if a.len() < b.len() {
        std::mem::swap(&mut a, &mut b);
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// A scored fuzzy match against an indexed file.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub path:  String,
    pub name:  String,
    pub score: f64,
}

/// Score candidates against the query, Jaro-Winkler primary with a Levenshtein fallback.
pub fn match_candidates(
    query: &str,
    candidates: &[FileRow],
    jw_threshold: f64,
    lev_max_dist: usize,
    limit: usize,
) -> Vec<FuzzyMatch> {
    let q = query.to_lowercase();
    let mut out: Vec<FuzzyMatch> = Vec::new();
    for candidate in candidates {
        let name = candidate.name.to_lowercase();
        let jw = jaro_winkler(&q, &name);
        let score = if jw >= jw_threshold {
            Some(jw)
        } else {
            let dist = levenshtein(&q, &name);
            if dist <= lev_max_dist {
                let denom = q.chars().count().max(name.chars().count()).max(1);
                Some(1.0 - dist as f64 / denom as f64)
            } else {
                None
            }
        };
        if let Some(score) = score {
            out.push(FuzzyMatch {
                path: candidate.path.clone(),
                name: candidate.name.clone(),
                score,
            });
        }
    }
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_row(name: &str) -> FileRow {
        FileRow {
            name: name.to_string(),
            path: format!("/{name}"),
            parent_path: "/".to_string(),
            file_type: "file".to_string(),
            size: None,
            last_modified: None,
            extension: None,
            is_hidden: 0,
        }
    }

    #[test]
    fn jaro_winkler_identical_returns_1() {
        assert!((jaro_winkler("report", "report") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaro_winkler_empty_both_returns_1() {
        assert!((jaro_winkler("", "") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaro_winkler_completely_different_below_0_5() {
        assert!(jaro_winkler("abc", "xyz") < 0.5);
    }

    #[test]
    fn jaro_winkler_martha_marhta_above_0_9() {
        assert!(jaro_winkler("martha", "marhta") > 0.9);
    }

    #[test]
    fn levenshtein_identical_is_0() {
        assert_eq!(levenshtein("kitten", "kitten"), 0);
    }

    #[test]
    fn levenshtein_empty_vs_abc_is_3() {
        assert_eq!(levenshtein("", "abc"), 3);
    }

    #[test]
    fn levenshtein_kitten_sitten_is_1() {
        assert_eq!(levenshtein("kitten", "sitten"), 1);
    }

    #[test]
    fn levenshtein_kitten_sitting_is_3() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
    }

    #[test]
    fn levenshtein_handles_unicode() {
        assert_eq!(levenshtein("cafe", "café"), 1);
    }

    #[test]
    fn match_candidates_sorted_by_score_desc() {
        let candidates = [file_row("report"), file_row("reprot"), file_row("xyz123")];
        let results = match_candidates("report", &candidates, 0.75, 2, 10);
        assert!(results.len() >= 2);
        assert_eq!(results[0].name, "report");
        for pair in results.windows(2) {
            assert!(pair[0].score >= pair[1].score);
        }
    }

    #[test]
    fn match_candidates_empty_input_returns_empty() {
        let candidates: [FileRow; 0] = [];
        let results = match_candidates("report", &candidates, 0.75, 2, 10);
        assert!(results.is_empty());
    }
}
