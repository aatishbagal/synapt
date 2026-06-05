use crate::storage::{Db, FileRow};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use thiserror::Error;

/// Files larger than this are skipped by the indexer.
const MAX_FILE_SIZE_BYTES: u64 = 10 * 1024 * 1024;

/// Emit a progress event after every this many files scanned.
const EMIT_EVERY_N_FILES: u64 = 250;

/// Tauri event name carrying [`IndexProgress`] updates to the frontend.
pub const INDEX_PROGRESS_EVENT: &str = "index-progress";

/// Errors raised while scanning the file system or writing the index.
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
}

/// A point-in-time snapshot of indexing progress, sent to the frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct IndexProgress {
    pub phase:         IndexPhase,
    pub files_scanned: u64,
    pub current_dir:   String,
    pub total_dirs:    usize,
    pub dirs_done:     usize,
}

/// The current stage of an index refresh.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum IndexPhase {
    Starting,
    Scanning,
    BuildingIndex,
    Complete { total_files: u64 },
    Failed { reason: String },
}

/// Shared progress counters the recursive scan updates and emits from.
struct ScanState<'a> {
    app:           &'a AppHandle,
    total_dirs:    usize,
    files_scanned: AtomicU64,
    dirs_done:     AtomicUsize,
    current_dir:   Mutex<String>,
}

impl ScanState<'_> {
    /// Build a progress snapshot for the given phase from the live counters.
    fn snapshot(&self, phase: IndexPhase) -> IndexProgress {
        IndexProgress {
            phase,
            files_scanned: self.files_scanned.load(Ordering::Relaxed),
            current_dir: self.current_dir.lock().map(|g| g.clone()).unwrap_or_default(),
            total_dirs: self.total_dirs,
            dirs_done: self.dirs_done.load(Ordering::Relaxed),
        }
    }

    /// Emit a progress event for the given phase (best-effort).
    fn emit(&self, phase: IndexPhase) {
        let _ = self.app.emit(INDEX_PROGRESS_EVENT, self.snapshot(phase));
    }
}

/// Emit a progress event that carries no scan counters (terminal phases).
fn emit_bare(app: &AppHandle, phase: IndexPhase) {
    let _ = app.emit(
        INDEX_PROGRESS_EVENT,
        IndexProgress { phase, files_scanned: 0, current_dir: String::new(), total_dirs: 0, dirs_done: 0 },
    );
}

/// Emit the terminal Complete event and clear the indexing flag. Callers invoke
/// this after the follow-up full-text index rebuild has finished.
pub fn finish_ok(app: &AppHandle, is_indexing: &Arc<AtomicBool>, total_files: u64) {
    let _ = app.emit(
        INDEX_PROGRESS_EVENT,
        IndexProgress {
            phase: IndexPhase::Complete { total_files },
            files_scanned: total_files,
            current_dir: String::new(),
            total_dirs: 0,
            dirs_done: 0,
        },
    );
    is_indexing.store(false, Ordering::Relaxed);
}

/// Emit a Failed event and clear the indexing flag.
pub fn finish_err(app: &AppHandle, is_indexing: &Arc<AtomicBool>, reason: String) {
    emit_bare(app, IndexPhase::Failed { reason });
    is_indexing.store(false, Ordering::Relaxed);
}

/// Scan every configured indexed directory and populate the files table,
/// emitting `index-progress` events throughout. Raises `is_indexing` for the
/// scan; on success the flag is left set so the caller can keep it raised
/// through the follow-up full-text index rebuild before calling [`finish_ok`].
/// On failure the flag is cleared and a Failed event is emitted.
pub async fn run_full_scan(
    db: &Db,
    include_hidden: bool,
    app: &AppHandle,
    is_indexing: &Arc<AtomicBool>,
) -> Result<u64, IndexError> {
    is_indexing.store(true, Ordering::Relaxed);
    match scan_all(db, include_hidden, app).await {
        Ok(total) => Ok(total),
        Err(e) => {
            emit_bare(app, IndexPhase::Failed { reason: e.to_string() });
            is_indexing.store(false, Ordering::Relaxed);
            Err(e)
        }
    }
}

/// Scan every indexed directory without emitting progress events, for callers
/// (tests, headless paths) that have no [`AppHandle`].
#[cfg(test)]
pub async fn run_full_scan_no_progress(db: &Db, include_hidden: bool) -> Result<u64, IndexError> {
    let dirs = db.get_indexed_dirs().await?;
    let mut total = 0u64;
    for dir in dirs {
        let count = scan_dir(db, Path::new(&dir.path), include_hidden, None).await?;
        db.update_indexed_dir_count(&dir.path, count as i64).await?;
        total += count;
    }
    Ok(total)
}

/// Walk every indexed directory, emitting Starting, per-directory Scanning, and
/// a final BuildingIndex event. Returns the total number of files indexed.
async fn scan_all(db: &Db, include_hidden: bool, app: &AppHandle) -> Result<u64, IndexError> {
    let dirs = db.get_indexed_dirs().await?;
    tracing::info!("indexer: starting full scan over {} directories", dirs.len());
    let state = ScanState {
        app,
        total_dirs: dirs.len(),
        files_scanned: AtomicU64::new(0),
        dirs_done: AtomicUsize::new(0),
        current_dir: Mutex::new(String::new()),
    };
    state.emit(IndexPhase::Starting);
    let mut total = 0u64;
    for dir in dirs {
        if let Ok(mut cur) = state.current_dir.lock() {
            *cur = dir.path.clone();
        }
        state.emit(IndexPhase::Scanning);
        let count = scan_dir(db, Path::new(&dir.path), include_hidden, Some(&state)).await?;
        db.update_indexed_dir_count(&dir.path, count as i64).await?;
        total += count;
        state.dirs_done.fetch_add(1, Ordering::Relaxed);
    }
    state.emit(IndexPhase::BuildingIndex);
    tracing::info!("indexer: scan complete, {} files indexed", total);
    Ok(total)
}

/// Recursively scan a single directory, upserting each qualifying file.
fn scan_dir<'a>(
    db: &'a Db,
    dir: &'a Path,
    include_hidden: bool,
    progress: Option<&'a ScanState<'a>>,
) -> Pin<Box<dyn Future<Output = Result<u64, IndexError>> + Send + 'a>> {
    Box::pin(async move {
        let mut count = 0u64;
        // An unreadable directory (e.g. permission denied) must not abort the
        // entire scan. Log it and skip this branch.
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("indexer: cannot read dir {}: {}", dir.display(), e);
                return Ok(0);
            }
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(e) => {
                    tracing::warn!("indexer: error reading entry in {}: {}", dir.display(), e);
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().to_string();
            let is_hidden = name.starts_with('.');
            if is_hidden && !include_hidden {
                continue;
            }
            let file_type = match entry.file_type().await {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                // Index the directory itself (type = 'dir') so folder search can
                // find it, then recurse for the files inside. Directories are not
                // counted toward the file-count stat.
                let parent_path = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let dir_row = FileRow {
                    name,
                    path: path.to_string_lossy().to_string(),
                    parent_path,
                    file_type: "dir".to_string(),
                    size: None,
                    last_modified: None,
                    extension: None,
                    is_hidden: if is_hidden { 1 } else { 0 },
                };
                db.upsert_file(&dir_row).await?;
                count += scan_dir(db, &path, include_hidden, progress).await.unwrap_or(0);
            } else if file_type.is_file() {
                let meta = match entry.metadata().await {
                    Ok(m) => m,
                    Err(_) => continue,
                };
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
                if let Some(p) = progress {
                    let scanned = p.files_scanned.fetch_add(1, Ordering::Relaxed) + 1;
                    if scanned % EMIT_EVERY_N_FILES == 0 {
                        p.emit(IndexPhase::Scanning);
                    }
                }
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

    #[test]
    fn index_progress_scanning_serialises_with_type_tag() {
        let progress = IndexProgress {
            phase: IndexPhase::Scanning,
            files_scanned: 5,
            current_dir: "/tmp".to_string(),
            total_dirs: 2,
            dirs_done: 1,
        };
        let json = serde_json::to_value(&progress).unwrap();
        assert_eq!(json["phase"]["type"], "Scanning");
        assert_eq!(json["files_scanned"], 5);
    }

    #[test]
    fn index_phase_complete_serialises_total_files() {
        let json = serde_json::to_value(IndexPhase::Complete { total_files: 42 }).unwrap();
        assert_eq!(json["type"], "Complete");
        assert_eq!(json["total_files"], 42);
    }

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

        let count = scan_dir(&db, &dir, false, None).await.unwrap();
        assert_eq!(count, 3);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_indexes_subdirectories_for_folder_search() {
        // Subdirectories are indexed as type='dir' (and excluded from the file
        // count) so the folder search mode can find them.
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        let sub = dir.join("Reports");
        std::fs::create_dir_all(&sub).unwrap();
        write_file(&sub, "a.txt", 10);

        let count = scan_dir(&db, &dir, false, None).await.unwrap();
        assert_eq!(count, 1, "only the file is counted, not the directory");

        let dirs = db.search_dirs_by_name("report", 10).await.unwrap();
        assert_eq!(dirs.len(), 1);
        assert_eq!(dirs[0].name, "Reports");
        assert_eq!(dirs[0].file_type, "dir");
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

        let count = scan_dir(&db, &dir, false, None).await.unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_skips_hidden_when_disabled() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "visible.txt", 10);
        write_file(&dir, ".hidden", 10);

        let count = scan_dir(&db, &dir, false, None).await.unwrap();
        assert_eq!(count, 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn scan_dir_includes_hidden_when_enabled() {
        let db = Db::open_in_memory().await.unwrap();
        let dir = temp_dir();
        write_file(&dir, "visible.txt", 10);
        write_file(&dir, ".hidden", 10);

        let count = scan_dir(&db, &dir, true, None).await.unwrap();
        assert_eq!(count, 2);
        std::fs::remove_dir_all(&dir).ok();
    }
}
