use crate::storage::{Db, FileRow};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use thiserror::Error;

/// Files larger than this are skipped by the indexer.
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Errors raised while scanning the file system or writing the index.
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
}

/// Scan every configured indexed directory and populate the files table.
pub async fn run_full_scan(db: &Db, include_hidden: bool) -> Result<u64, IndexError> {
    let dirs = db.get_indexed_dirs().await?;
    let mut total = 0u64;
    for dir in dirs {
        let count = scan_dir(db, Path::new(&dir.path), include_hidden).await?;
        db.update_indexed_dir_count(&dir.path, count as i64).await?;
        total += count;
    }
    Ok(total)
}

/// Recursively scan a single directory, upserting each qualifying file.
fn scan_dir<'a>(
    db: &'a Db,
    dir: &'a Path,
    include_hidden: bool,
) -> Pin<Box<dyn Future<Output = Result<u64, IndexError>> + Send + 'a>> {
    Box::pin(async move {
        let mut count = 0u64;
        let mut entries = tokio::fs::read_dir(dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_hidden = name.starts_with('.');
            if is_hidden && !include_hidden {
                continue;
            }
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                count += scan_dir(db, &path, include_hidden).await?;
            } else if file_type.is_file() {
                let meta = entry.metadata().await?;
                if meta.len() > MAX_FILE_SIZE_BYTES {
                    continue;
                }
                let last_modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64);
                let extension = path.extension().map(|e| e.to_string_lossy().to_string());
                let parent_path = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let row = FileRow {
                    name,
                    path: path.to_string_lossy().to_string(),
                    parent_path,
                    file_type: "file".to_string(),
                    size: Some(meta.len() as i64),
                    last_modified,
                    extension,
                    is_hidden: if is_hidden { 1 } else { 0 },
                };
                db.upsert_file(&row).await?;
                count += 1;
            }
        }
        Ok(count)
    })
}

/// Remove index rows whose backing file no longer exists on disk.
pub async fn prune_deleted(db: &Db) -> Result<u64, IndexError> {
    let paths = db.get_all_file_paths().await?;
    let mut removed = 0u64;
    for path in paths {
        if !Path::new(&path).exists() {
            db.delete_file_by_path(&path).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("synapt_indexer_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write_file(dir: &Path, name: &str, bytes: usize) {
        let path = dir.join(name);
        std::fs::write(&path, vec![b'x'; bytes]).unwrap();
    }

    #[tokio::test]
    async fn scan_dir_counts_three_files() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "a.txt", 10);
        write_file(&dir, "b.txt", 10);
        write_file(&dir, "c.txt", 10);

        let count = scan_dir(&db, &dir, false).await.unwrap();
        assert_eq!(count, 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_skips_files_over_limit() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "small.txt", 10);
        let big = dir.join("big.bin");
        let f = std::fs::File::create(&big).unwrap();
        f.set_len(MAX_FILE_SIZE_BYTES + 1).unwrap();
        drop(f);

        let count = scan_dir(&db, &dir, false).await.unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_skips_hidden_when_disabled() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "visible.txt", 10);
        write_file(&dir, ".hidden", 10);

        let count = scan_dir(&db, &dir, false).await.unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_includes_hidden_when_enabled() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "visible.txt", 10);
        write_file(&dir, ".hidden", 10);

        let count = scan_dir(&db, &dir, true).await.unwrap();
        assert_eq!(count, 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
