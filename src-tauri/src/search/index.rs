use crate::storage::{Db, FileRow};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::{QueryParser, RegexQuery};
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};
use thiserror::Error;

/// Arena the tantivy writer keeps for the whole life of the process.
///
/// The writer is only used for the startup rebuild and for single-file updates,
/// so it spends nearly all of its time idle holding this budget. This is
/// tantivy's own documented floor: it divides the budget across indexing
/// threads and rejects anything under 15 MB per thread, so 15 MB buys one
/// indexing thread and is the smallest arena it will accept. A rebuild flushes
/// more segments than a larger arena would, which the merge policy then folds
/// back together.
const WRITER_HEAP_BYTES: usize = 15_000_000;

/// Errors raised by the full-text index layer.
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("open dir: {0}")]
    OpenDir(#[from] tantivy::directory::error::OpenDirectoryError),
    #[error("tantivy: {0}")]
    Tantivy(#[from] tantivy::TantivyError),
    #[error("query: {0}")]
    Query(#[from] tantivy::query::QueryParserError),
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
}

/// Schema and field handles for the file full-text index.
pub struct FileSchema {
    pub schema:    Schema,
    pub name:      Field,
    pub name_raw:  Field,
    pub path:      Field,
    pub extension: Field,
}

impl FileSchema {
    /// Build the file index schema.
    pub fn build() -> Self {
        let mut builder = Schema::builder();
        // `name` uses the TEXT analyser for tokenised term queries; `name_raw`
        // stores the whole lowercased name as a single STRING term so regex /
        // wildcard substring queries can match across token boundaries.
        let name = builder.add_text_field("name", TEXT | STORED);
        let name_raw = builder.add_text_field("name_raw", STRING | STORED);
        let path = builder.add_text_field("path", STORED);
        let extension = builder.add_text_field("extension", STRING | STORED);
        let schema = builder.build();
        Self { schema, name, name_raw, path, extension }
    }
}

/// Full-text index over indexed file names backed by tantivy.
pub struct FileIndex {
    index:  Index,
    reader: IndexReader,
    schema: FileSchema,
    writer: Arc<RwLock<IndexWriter>>,
}

/// Replace tantivy query-syntax characters with spaces so user input is treated literally.
fn escape_query(input: &str) -> String {
    const SPECIAL: &str = "+^`:{}[]\"\\()~";
    input
        .chars()
        .map(|c| if SPECIAL.contains(c) { ' ' } else { c })
        .collect()
}

/// Escape regex metacharacters so user input is matched literally by a RegexQuery.
fn escape_regex(input: &str) -> String {
    const META: &str = r".+*?()|[]{}^$\";
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        if META.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

impl FileIndex {
    /// Open or create the index at the given directory.
    ///
    /// If an index already exists with a different schema (e.g. after adding
    /// the `name_raw` field), the on-disk index is deleted and recreated so the
    /// new schema takes effect. The caller is expected to rebuild it from the
    /// database afterwards via [`FileIndex::rebuild_from_db`].
    pub fn open(index_dir: PathBuf) -> Result<Self, IndexError> {
        std::fs::create_dir_all(&index_dir)?;
        let schema = FileSchema::build();
        let index = match Self::open_or_create(&index_dir, &schema) {
            Ok(index) => index,
            Err(err) => {
                tracing::warn!(
                    "file index schema mismatch ({}); rebuilding index at {}",
                    err,
                    index_dir.display()
                );
                std::fs::remove_dir_all(&index_dir)?;
                std::fs::create_dir_all(&index_dir)?;
                Self::open_or_create(&index_dir, &schema)?
            }
        };
        let writer: IndexWriter = index.writer(WRITER_HEAP_BYTES)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self { index, reader, schema, writer: Arc::new(RwLock::new(writer)) })
    }

    /// Open or create the tantivy index in `index_dir`, validating the schema matches.
    fn open_or_create(index_dir: &PathBuf, schema: &FileSchema) -> Result<Index, IndexError> {
        let directory = MmapDirectory::open(index_dir)?;
        let index = Index::open_or_create(directory, schema.schema.clone())?;
        Ok(index)
    }

    /// Drop and rebuild the entire index from the database files table.
    pub async fn rebuild_from_db(&self, db: &Db) -> Result<u64, IndexError> {
        let files = db.get_all_files_for_index().await?;
        let mut writer = self.writer.write().unwrap_or_else(|p| p.into_inner());
        writer.delete_all_documents()?;
        let mut count = 0u64;
        for f in &files {
            writer.add_document(self.make_doc(f))?;
            count += 1;
        }
        writer.commit()?;
        self.reader.reload()?;
        Ok(count)
    }

    /// Add a single file to the index and make it immediately searchable.
    pub fn add_file(&self, f: &FileRow) -> Result<(), IndexError> {
        let mut writer = self.writer.write().unwrap_or_else(|p| p.into_inner());
        writer.add_document(self.make_doc(f))?;
        writer.commit()?;
        self.reader.reload()?;
        Ok(())
    }

    /// Search file names, returning matching stored paths.
    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<String>, IndexError> {
        let cleaned = escape_query(query_str);
        if cleaned.trim().is_empty() {
            return Ok(Vec::new());
        }
        let searcher = self.reader.searcher();
        let parser = QueryParser::for_index(&self.index, vec![self.schema.name]);
        let query = parser.parse_query(cleaned.trim())?;
        let top = searcher.search(&query, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(top.len());
        for (_score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            if let Some(path) = doc.get_first(self.schema.path).and_then(|v| v.as_str()) {
                out.push(path.to_string());
            }
        }
        Ok(out)
    }

    /// Search using a contains query (`*query*`) to find substrings in the middle of
    /// filenames. More expensive than [`FileIndex::search`] (it walks the term
    /// dictionary via a regex automaton), so only call it when the standard search
    /// returns few results.
    pub fn search_wildcard(&self, query_str: &str, limit: usize) -> Result<Vec<String>, IndexError> {
        let trimmed = query_str.trim().to_lowercase();
        if trimmed.is_empty() {
            return Ok(Vec::new());
        }
        // The `name_raw` field stores each name as a single lowercased term, and the
        // tantivy-fst regex is implicitly anchored to the whole term, so wrap the
        // (escaped) query in `.*` on each side to match it as a substring.
        let pattern = format!(".*{}.*", escape_regex(&trimmed));
        let query = RegexQuery::from_pattern(&pattern, self.schema.name_raw)?;
        let searcher = self.reader.searcher();
        let top = searcher.search(&query, &TopDocs::with_limit(limit))?;
        let mut out = Vec::with_capacity(top.len());
        for (_score, addr) in top {
            let doc: TantivyDocument = searcher.doc(addr)?;
            if let Some(path) = doc.get_first(self.schema.path).and_then(|v| v.as_str()) {
                out.push(path.to_string());
            }
        }
        Ok(out)
    }

    fn make_doc(&self, f: &FileRow) -> TantivyDocument {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.schema.name, &f.name);
        doc.add_text(self.schema.name_raw, f.name.to_lowercase());
        doc.add_text(self.schema.path, &f.path);
        if let Some(ext) = &f.extension {
            doc.add_text(self.schema.extension, ext);
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_index_dir() -> PathBuf {
        std::env::temp_dir().join(format!("synapt_index_{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn escape_query_strips_special_chars() {
        let cleaned = escape_query("a+b^c(d)e~f:g[h]");
        for c in "+^()~:[]".chars() {
            assert!(!cleaned.contains(c), "should not contain {c}");
        }
    }

    #[test]
    fn open_creates_directory_if_absent() {
        let dir = temp_index_dir();
        assert!(!dir.exists());
        let _index = FileIndex::open(dir.clone()).unwrap();
        assert!(dir.exists());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_on_empty_index_returns_empty() {
        let dir = temp_index_dir();
        let index = FileIndex::open(dir.clone()).unwrap();
        let results = index.search("report", 10).unwrap();
        assert!(results.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    fn file_row(name: &str) -> FileRow {
        FileRow {
            name: name.to_string(),
            path: format!("/tmp/{name}"),
            parent_path: "/tmp".to_string(),
            file_type: "file".to_string(),
            size: None,
            last_modified: None,
            extension: None,
            is_hidden: 0,
        }
    }

    #[test]
    fn escape_regex_escapes_metacharacters() {
        let escaped = escape_regex("a.b*c(d)e");
        assert_eq!(escaped, r"a\.b\*c\(d\)e");
    }

    #[test]
    fn search_wildcard_finds_substring_in_middle_of_name() {
        let dir = temp_index_dir();
        let index = FileIndex::open(dir.clone()).unwrap();
        index.add_file(&file_row("somethingtestfile.json")).unwrap();
        let results = index.search_wildcard("test", 10).unwrap();
        assert!(
            results.iter().any(|p| p.ends_with("somethingtestfile.json")),
            "wildcard search should find 'test' inside 'somethingtestfile.json', got {results:?}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_wildcard_empty_query_returns_empty() {
        let dir = temp_index_dir();
        let index = FileIndex::open(dir.clone()).unwrap();
        index.add_file(&file_row("somethingtestfile.json")).unwrap();
        assert!(index.search_wildcard("", 10).unwrap().is_empty());
        assert!(index.search_wildcard("   ", 10).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn search_wildcard_does_not_panic_on_regex_chars() {
        let dir = temp_index_dir();
        let index = FileIndex::open(dir.clone()).unwrap();
        index.add_file(&file_row("a(b)c.txt")).unwrap();
        // Special regex characters must be escaped, not interpreted, and must not panic.
        let results = index.search_wildcard("(b)", 10).unwrap();
        assert!(
            results.iter().any(|p| p.ends_with("a(b)c.txt")),
            "escaped regex chars should match literally, got {results:?}"
        );
        // A lone metacharacter that does not appear literally matches nothing.
        assert!(index.search_wildcard("+", 10).unwrap().is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }
}
