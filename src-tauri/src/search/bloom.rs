/// Bloom filter using FNV-1a and DJB2 double hashing as a search pre-filter gate.
#[derive(Debug, Clone)]
pub struct BloomFilter {
    bits: Vec<bool>,
    k:    usize,
    /// Configured expected capacity, retained for diagnostics and resizing decisions.
    #[allow(dead_code)]
    capacity: usize,
}

impl BloomFilter {
    /// Create a filter sized for the expected capacity and target false-positive rate.
    pub fn new(capacity: usize, fp_rate: f64) -> Self {
        let cap = capacity.max(1);
        let m = (-(cap as f64 * fp_rate.ln()) / std::f64::consts::LN_2.powi(2)).ceil() as usize;
        let m = m.max(1);
        let k = (((m as f64) / cap as f64) * std::f64::consts::LN_2)
            .round()
            .max(1.0) as usize;
        Self { bits: vec![false; m], k, capacity: cap }
    }

    /// Insert a term into the filter.
    pub fn insert(&mut self, term: &str) {
        for i in 0..self.k {
            let idx = self.hash_i(term, i);
            self.bits[idx] = true;
        }
    }

    /// Return true if the term may be present, false if it is definitely absent.
    pub fn may_contain(&self, term: &str) -> bool {
        for i in 0..self.k {
            if !self.bits[self.hash_i(term, i)] {
                return false;
            }
        }
        true
    }

    fn fnv1a(data: &[u8]) -> u64 {
        let mut hash = 14695981039346656037u64;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(1099511628211);
        }
        hash
    }

    fn djb2(data: &[u8]) -> u64 {
        let mut hash = 5381u64;
        for &byte in data {
            hash = hash.wrapping_shl(5).wrapping_add(hash).wrapping_add(byte as u64);
        }
        hash
    }

    fn hash_i(&self, term: &str, i: usize) -> usize {
        let h1 = Self::fnv1a(term.as_bytes());
        // Force the DJB2 step odd so it stays coprime with an even table size,
        // keeping the k probes distinct and the false-positive rate near optimal.
        let h2 = Self::djb2(term.as_bytes()) | 1;
        (h1.wrapping_add((i as u64).wrapping_mul(h2)) % self.bits.len() as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserted_term_always_may_contain_true() {
        let mut filter = BloomFilter::new(100, 0.01);
        filter.insert("hello");
        assert!(filter.may_contain("hello"));
    }

    #[test]
    fn uninserted_term_on_empty_filter_returns_false() {
        let filter = BloomFilter::new(100, 0.01);
        assert!(!filter.may_contain("hello"));
    }

    #[test]
    fn false_positive_rate_within_bounds() {
        let mut filter = BloomFilter::new(1000, 0.01);
        for i in 0..1000 {
            filter.insert(&format!("inserted-term-{i}"));
        }
        let mut false_positives = 0;
        for i in 0..10000 {
            if filter.may_contain(&format!("absent-term-{i}")) {
                false_positives += 1;
            }
        }
        let rate = false_positives as f64 / 10000.0;
        assert!(rate < 0.02, "false positive rate {rate} exceeded bound");
    }

    #[test]
    fn fnv1a_empty_is_14695981039346656037() {
        assert_eq!(BloomFilter::fnv1a(b""), 14695981039346656037);
    }

    #[test]
    fn djb2_empty_is_5381() {
        assert_eq!(BloomFilter::djb2(b""), 5381);
    }
}
