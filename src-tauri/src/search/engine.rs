use crate::search::bloom::BloomFilter;
use crate::search::fuzzy;
use crate::search::index::FileIndex;
use crate::search::trie::Trie;
use crate::storage::{Db, FileRow};
use lru::LruCache;
use std::collections::HashSet;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use thiserror::Error;

const LRU_CAPACITY: usize = 200;
const JW_THRESHOLD: f64 = 0.75;
const LEV_MAX_DIST: usize = 2;
const MIN_RESULTS_BEFORE_FUZZY: usize = 3;

/// A single search hit returned to the UI.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SearchResult {
    pub name:   String,
    pub path:   String,
    /// Command or path used to launch the result (applications only).
    #[serde(default)]
    pub exec:   Option<String>,
    /// Whether this hit is a file or an installed application.
    #[serde(default)]
    pub result_type: ResultType,
    pub source: ResultSource,
    pub score:  f64,
}

/// Whether a result is a file or an installed application.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub enum ResultType {
    #[default]
    File,
    App,
}

/// Where a result came from.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ResultSource {
    LocalFile,
    LocalApp,
    Remote { device_name: String },
}

/// Errors raised by the search engine.
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
    #[error("index: {0}")]
    Index(#[from] crate::search::index::IndexError),
}

/// Orchestrates the Bloom -> Trie -> tantivy -> fuzzy pipeline with an LRU session cache.
pub struct SearchEngine {
    pub trie:  Arc<Mutex<Trie>>,
    pub bloom: Arc<Mutex<BloomFilter>>,
    cache:     Arc<Mutex<LruCache<String, Vec<SearchResult>>>>,
    index:     Arc<FileIndex>,
    db:        Arc<Db>,
}

fn build_indexes(files: &[FileRow]) -> (Trie, BloomFilter) {
    let mut trie = Trie::new();
    let mut bloom = BloomFilter::new(files.len().max(1000), 0.01);
    for f in files {
        let name_lc = f.name.to_lowercase();
        trie.insert(&name_lc, f.path.clone());
        bloom.insert(&name_lc);
    }
    (trie, bloom)
}

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// Build a local file [`SearchResult`] for a path at the given score.
fn file_result(path: String, score: f64) -> SearchResult {
    SearchResult {
        name: file_name_of(&path),
        exec: None,
        result_type: ResultType::File,
        source: ResultSource::LocalFile,
        score,
        path,
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl SearchEngine {
    /// Build the engine, loading the Trie and Bloom filter from the persisted file index.
    pub async fn init(db: Arc<Db>, index: Arc<FileIndex>) -> Result<Self, SearchError> {
        let files = db.get_all_files_for_index().await?;
        let (trie, bloom) = build_indexes(&files);
        let capacity = NonZeroUsize::new(LRU_CAPACITY).unwrap_or(NonZeroUsize::MIN);
        Ok(Self {
            trie:  Arc::new(Mutex::new(trie)),
            bloom: Arc::new(Mutex::new(bloom)),
            cache: Arc::new(Mutex::new(LruCache::new(capacity))),
            index,
            db,
        })
    }

    /// Rebuild the Trie and Bloom filter from the database and clear the cache.
    pub async fn rebuild(&self) -> Result<(), SearchError> {
        let files = self.db.get_all_files_for_index().await?;
        let (trie, bloom) = build_indexes(&files);
        *lock(&self.trie) = trie;
        *lock(&self.bloom) = bloom;
        self.invalidate_cache();
        Ok(())
    }

    /// Run the full local search pipeline.
    pub async fn search(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let query_trimmed = query.trim();
        if query_trimmed.is_empty() {
            return Ok(Vec::new());
        }

        // Star syntax: an explicit power-user contains search. A leading and/or
        // trailing '*' routes straight to the wildcard stage, skipping the trie,
        // standard tantivy, and fuzzy stages.
        if query_trimmed.starts_with('*') || query_trimmed.ends_with('*') {
            let stripped = query_trimmed.trim_matches('*').to_lowercase();
            if stripped.is_empty() {
                return Ok(Vec::new());
            }
            let results = self
                .index
                .search_wildcard(&stripped, max_results)?
                .into_iter()
                .map(|path| file_result(path, 0.65))
                .collect();
            return Ok(results);
        }

        let q = query_trimmed.to_lowercase();

        if let Some(hit) = lock(&self.cache).get(&q) {
            return Ok(hit.clone());
        }

        // Note: the Bloom filter is keyed on whole file names, but queries are
        // prefixes (trie), tokens (tantivy), or substrings (wildcard/fuzzy). A
        // name-keyed membership test produces false negatives for every one of
        // those paths, so it must not gate the search. The trie already resolves
        // absent prefixes in O(prefix length); the bloom is left as diagnostic infra.
        let mut results: Vec<SearchResult> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        // 1. Trie prefix search.
        for path in lock(&self.trie).prefix_search(&q, max_results) {
            if seen.insert(path.clone()) {
                results.push(file_result(path, 1.0));
            }
        }

        // 2. tantivy standard full-text (tokenised terms).
        if results.len() < max_results {
            for path in self.index.search(&q, max_results)? {
                if seen.insert(path.clone()) {
                    results.push(file_result(path, 0.8));
                }
            }
        }

        // 3. tantivy wildcard substring stage — catches matches in the middle of a
        // single token, e.g. 'test' in 'somethingtestfile.json'. Only run when the
        // cheaper stages came up short, since the regex automaton is more expensive.
        if results.len() < MIN_RESULTS_BEFORE_FUZZY {
            for path in self.index.search_wildcard(&q, max_results)? {
                if seen.insert(path.clone()) {
                    results.push(file_result(path, 0.65));
                }
            }
        }

        // 4. Application search — apps are surfaced above files. Always run so a
        // matching app appears regardless of how many files matched.
        let app_results = self.db.search_apps_by_name(&q).await.unwrap_or_default();
        for app in app_results {
            let key = format!("app:{}", app.source_path);
            if seen.insert(key) {
                results.push(SearchResult {
                    name: app.name,
                    path: app.source_path,
                    exec: Some(app.exec),
                    result_type: ResultType::App,
                    source: ResultSource::LocalApp,
                    score: 1.0,
                });
            }
        }

        // 5. Fuzzy fallback.
        if results.len() < MIN_RESULTS_BEFORE_FUZZY {
            let candidates = self.db.search_files_by_name(&q, 500).await?;
            for m in fuzzy::match_candidates(&q, &candidates, JW_THRESHOLD, LEV_MAX_DIST, max_results)
            {
                if seen.insert(m.path.clone()) {
                    results.push(SearchResult {
                        name: m.name,
                        path: m.path,
                        exec: None,
                        result_type: ResultType::File,
                        source: ResultSource::LocalFile,
                        score: m.score * 0.6,
                    });
                }
            }
        }

        // Apps sort above files; within a type, higher score first.
        results.sort_by(|a, b| match (&a.result_type, &b.result_type) {
            (ResultType::App, ResultType::File) => std::cmp::Ordering::Less,
            (ResultType::File, ResultType::App) => std::cmp::Ordering::Greater,
            _ => b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal),
        });

        results.truncate(max_results);
        lock(&self.cache).put(q, results.clone());
        Ok(results)
    }

    /// Add a single file to the in-memory Trie and Bloom filter.
    pub fn add_file(&self, name: &str, path: &str) {
        let name_lc = name.to_lowercase();
        lock(&self.trie).insert(&name_lc, path.to_string());
        lock(&self.bloom).insert(&name_lc);
    }

    /// Remove one occurrence of a file name from the Trie.
    pub fn remove_file(&self, name: &str) {
        lock(&self.trie).remove(&name.to_lowercase());
    }

    /// Clear the session search cache.
    pub fn invalidate_cache(&self) {
        lock(&self.cache).clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn empty_engine() -> SearchEngine {
        let db = Arc::new(Db::open_in_memory().await.unwrap());
        let dir = std::env::temp_dir().join(format!("synapt_engine_{}", uuid::Uuid::new_v4()));
        let index = Arc::new(FileIndex::open(dir).unwrap());
        SearchEngine::init(db, index).await.unwrap()
    }

    /// Build an engine whose index and database contain the given file names.
    async fn engine_with_files(names: &[&str]) -> SearchEngine {
        use crate::storage::FileRow;
        let db = Arc::new(Db::open_in_memory().await.unwrap());
        for name in names {
            db.upsert_file(&FileRow {
                name: name.to_string(),
                path: format!("/tmp/{name}"),
                parent_path: "/tmp".to_string(),
                file_type: "file".to_string(),
                size: None,
                last_modified: None,
                extension: None,
                is_hidden: 0,
            })
            .await
            .unwrap();
        }
        let dir = std::env::temp_dir().join(format!("synapt_engine_{}", uuid::Uuid::new_v4()));
        let index = Arc::new(FileIndex::open(dir).unwrap());
        index.rebuild_from_db(&db).await.unwrap();
        SearchEngine::init(db, index).await.unwrap()
    }

    #[tokio::test]
    async fn search_empty_engine_returns_empty() {
        let engine = empty_engine().await;
        let results = engine.search("anything", 50).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn star_syntax_routes_to_wildcard_search() {
        // 'somethingtestfile.json' has 'test' in the middle of a single token, so
        // only the wildcard stage can match it. '*test*' must find it directly.
        let engine = engine_with_files(&["somethingtestfile.json"]).await;
        let results = engine.search("*test*", 50).await.unwrap();
        assert!(
            results.iter().any(|r| r.name == "somethingtestfile.json"),
            "star syntax should route to wildcard search, got {results:?}"
        );
    }

    #[tokio::test]
    async fn star_syntax_empty_after_stripping_returns_empty() {
        let engine = engine_with_files(&["somethingtestfile.json"]).await;
        assert!(engine.search("**", 50).await.unwrap().is_empty());
        assert!(engine.search("*", 50).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn standard_query_finds_middle_substring_via_pipeline() {
        // A bare query (no stars) must still surface middle-of-name substrings
        // through the wildcard stage of the normal pipeline.
        let engine = engine_with_files(&["project_test_data.json"]).await;
        let results = engine.search("test", 50).await.unwrap();
        assert!(
            results.iter().any(|r| r.name == "project_test_data.json"),
            "standard pipeline should find middle substring, got {results:?}"
        );
    }

    #[tokio::test]
    async fn standard_query_prefix_match_still_works() {
        let engine = engine_with_files(&["report_final.pdf"]).await;
        let results = engine.search("report", 50).await.unwrap();
        assert!(results.iter().any(|r| r.name == "report_final.pdf"));
    }

    #[tokio::test]
    async fn app_results_sort_above_file_results() {
        use crate::storage::AppRow;
        // A file and an app both match 'code'; the app must come first.
        let engine = engine_with_files(&["code_notes.txt"]).await;
        engine
            .db
            .upsert_app(&AppRow {
                id: 0,
                name: "VS Code".to_string(),
                exec: "code".to_string(),
                icon_path: None,
                platform: "linux".to_string(),
                source_path: "/usr/share/applications/code.desktop".to_string(),
            })
            .await
            .unwrap();

        let results = engine.search("code", 50).await.unwrap();
        assert!(results.len() >= 2, "expected app and file, got {results:?}");
        assert!(matches!(results[0].result_type, ResultType::App));
        assert_eq!(results[0].name, "VS Code");
        assert!(results.iter().any(|r| r.name == "code_notes.txt"));
    }

    #[tokio::test]
    async fn invalidate_cache_clears_all_entries() {
        let engine = empty_engine().await;
        let _ = engine.search("foo bar", 50).await.unwrap();
        assert_eq!(lock(&engine.cache).len(), 1);
        engine.invalidate_cache();
        assert_eq!(lock(&engine.cache).len(), 0);
    }

    // Regression test for the v0.3.1 file-access bug: nothing was indexed
    // because no directory was ever added to indexed_dirs. This exercises the
    // whole pipeline (add dir -> scan -> rebuild -> search) and asserts a query
    // returns the file.
    #[tokio::test]
    async fn pipeline_indexes_added_dir_and_search_finds_file() {
        use crate::search::indexer;

        let db = Arc::new(Db::open_in_memory().await.unwrap());
        let work = std::env::temp_dir().join(format!("synapt_pipe_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&work).unwrap();
        std::fs::write(work.join("quarterly_report.txt"), b"data").unwrap();

        db.add_indexed_dir(&work.to_string_lossy()).await.unwrap();
        let scanned = indexer::run_full_scan_no_progress(&db, false).await.unwrap();
        assert_eq!(scanned, 1, "scan should index exactly one file");

        let idx_dir = std::env::temp_dir().join(format!("synapt_pipe_idx_{}", uuid::Uuid::new_v4()));
        let index = Arc::new(FileIndex::open(idx_dir).unwrap());
        index.rebuild_from_db(&db).await.unwrap();
        let engine = SearchEngine::init(Arc::clone(&db), index).await.unwrap();

        let results = engine.search("quarterly", 50).await.unwrap();
        assert!(
            results.iter().any(|r| r.name == "quarterly_report.txt"),
            "search should find the indexed file, got {:?}",
            results
        );

        std::fs::remove_dir_all(&work).ok();
    }
}
