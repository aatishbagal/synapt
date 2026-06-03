use std::collections::HashMap;

/// A single node in the prefix Trie.
struct TrieNode {
    children:  HashMap<char, TrieNode>,
    is_end:    bool,
    frequency: u32,
    value:     Option<String>,
}

impl TrieNode {
    fn new() -> Self {
        Self { children: HashMap::new(), is_end: false, frequency: 0, value: None }
    }
}

/// Outcome of a recursive remove, threaded back up the call stack.
struct RemoveResult {
    existed:       bool,
    fully_removed: bool,
}

/// Frequency-weighted prefix Trie for autocompletion.
pub struct Trie {
    root: TrieNode,
    size: usize,
}

impl Trie {
    /// Create an empty Trie.
    pub fn new() -> Self {
        Self { root: TrieNode::new(), size: 0 }
    }

    /// Insert a key/value pair, incrementing frequency if the key already exists.
    pub fn insert(&mut self, key: &str, value: String) {
        let mut node = &mut self.root;
        for c in key.chars() {
            node = node.children.entry(c).or_insert_with(TrieNode::new);
        }
        if node.is_end {
            node.frequency += 1;
        } else {
            node.is_end = true;
            node.frequency = 1;
            node.value = Some(value);
            self.size += 1;
        }
    }

    /// Remove one occurrence of a key, pruning dead nodes when frequency reaches zero.
    pub fn remove(&mut self, key: &str) -> bool {
        let chars: Vec<char> = key.chars().collect();
        let result = Self::remove_rec(&mut self.root, &chars, 0);
        if result.fully_removed {
            self.size -= 1;
        }
        result.existed
    }

    fn remove_rec(node: &mut TrieNode, chars: &[char], i: usize) -> RemoveResult {
        if i == chars.len() {
            if !node.is_end {
                return RemoveResult { existed: false, fully_removed: false };
            }
            node.frequency -= 1;
            if node.frequency == 0 {
                node.is_end = false;
                node.value = None;
                return RemoveResult { existed: true, fully_removed: true };
            }
            return RemoveResult { existed: true, fully_removed: false };
        }
        let c = chars[i];
        let (result, child_dead) = match node.children.get_mut(&c) {
            None => return RemoveResult { existed: false, fully_removed: false },
            Some(child) => {
                let r = Self::remove_rec(child, chars, i + 1);
                let dead = child.children.is_empty() && !child.is_end;
                (r, dead)
            }
        };
        if result.existed && child_dead {
            node.children.remove(&c);
        }
        result
    }

    /// Get the value stored at an exact key, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        let mut node = &self.root;
        for c in key.chars() {
            node = node.children.get(&c)?;
        }
        if node.is_end {
            node.value.as_deref()
        } else {
            None
        }
    }

    /// Return values for all keys sharing the prefix, ordered by frequency descending.
    pub fn prefix_search(&self, prefix: &str, limit: usize) -> Vec<String> {
        let mut node = &self.root;
        for c in prefix.chars() {
            match node.children.get(&c) {
                Some(n) => node = n,
                None => return Vec::new(),
            }
        }
        let mut collected: Vec<(u32, String)> = Vec::new();
        Self::collect(node, &mut collected);
        collected.sort_by(|a, b| b.0.cmp(&a.0));
        collected.into_iter().take(limit).map(|(_, v)| v).collect()
    }

    fn collect(node: &TrieNode, out: &mut Vec<(u32, String)>) {
        if node.is_end {
            if let Some(v) = &node.value {
                out.push((node.frequency, v.clone()));
            }
        }
        for child in node.children.values() {
            Self::collect(child, out);
        }
    }

    /// Number of unique keys stored.
    pub fn len(&self) -> usize {
        self.size
    }

    /// Whether the Trie holds no keys.
    pub fn is_empty(&self) -> bool {
        self.size == 0
    }
}

impl Default for Trie {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_exact_get() {
        let mut t = Trie::new();
        t.insert("abc", "value-abc".to_string());
        assert_eq!(t.get("abc"), Some("value-abc"));
        assert_eq!(t.get("ab"), None);
        assert_eq!(t.get("abcd"), None);
    }

    #[test]
    fn prefix_search_frequency_ordered() {
        let mut t = Trie::new();
        for _ in 0..3 {
            t.insert("report.pdf", "report.pdf".to_string());
        }
        t.insert("readme.md", "readme.md".to_string());
        let results = t.prefix_search("re", 10);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0], "report.pdf");
        assert_eq!(results[1], "readme.md");
    }

    #[test]
    fn prefix_search_empty_prefix_returns_all() {
        let mut t = Trie::new();
        t.insert("alpha", "alpha".to_string());
        t.insert("beta", "beta".to_string());
        let results = t.prefix_search("", 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn prefix_search_no_match_returns_empty() {
        let mut t = Trie::new();
        t.insert("alpha", "alpha".to_string());
        assert!(t.prefix_search("zzz", 10).is_empty());
    }

    #[test]
    fn remove_decrements_frequency() {
        let mut t = Trie::new();
        t.insert("a", "a".to_string());
        t.insert("a", "a".to_string());
        assert!(t.remove("a"));
        assert_eq!(t.get("a"), Some("a"));
        assert!(t.remove("a"));
        assert_eq!(t.get("a"), None);
    }

    #[test]
    fn remove_nonexistent_returns_false() {
        let mut t = Trie::new();
        t.insert("alpha", "alpha".to_string());
        assert!(!t.remove("beta"));
    }

    #[test]
    fn insert_same_key_twice_increments_frequency() {
        let mut t = Trie::new();
        t.insert("k", "k".to_string());
        t.insert("k", "k".to_string());
        t.insert("m", "m".to_string());
        assert_eq!(t.len(), 2);
        let results = t.prefix_search("", 10);
        assert_eq!(results[0], "k");
    }

    #[test]
    fn len_tracks_unique_keys() {
        let mut t = Trie::new();
        t.insert("a", "a".to_string());
        t.insert("b", "b".to_string());
        t.insert("a", "a".to_string());
        assert_eq!(t.len(), 2);
    }
}
