use crate::storage::{Db, FileRow};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tantivy::collector::TopDocs;
use tantivy::directory::MmapDirectory;
use tantivy::query::QueryParser;
use tantivy::schema::{Field, Schema, Value, STORED, STRING, TEXT};
use tantivy::{Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument};
use thiserror::Error;

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
    pub path:      Field,
    pub extension: Field,
}

impl FileSchema {
    /// Build the file index schema.
    pub fn build() -> Self {
        let mut builder = Schema::builder();
        let name = builder.add_text_field("name", TEXT | STORED);
        let path = builder.add_text_field("path", STORED);
        let extension = builder.add_text_field("extension", STRING | STORED);
        let schema = builder.build();
        Self { schema, name, path, extension }
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

impl FileIndex {
    /// Open or create the index at the given directory.
    pub fn open(index_dir: PathBuf) -> Result<Self, IndexError> {
        std::fs::create_dir_all(&index_dir)?;
        let schema = FileSchema::build();
        let directory = MmapDirectory::open(&index_dir)?;
        let index = Index::open_or_create(directory, schema.schema.clone())?;
        let writer: IndexWriter = index.writer(50_000_000)?;
        let reader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::Manual)
            .try_into()?;
        Ok(Self { index, reader, schema, writer: Arc::new(RwLock::new(writer)) })
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

    fn make_doc(&self, f: &FileRow) -> TantivyDocument {
        let mut doc = TantivyDocument::default();
        doc.add_text(self.schema.name, &f.name);
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
}
