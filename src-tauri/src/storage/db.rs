use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use thiserror::Error;

/// Errors raised by the database layer.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("path error: {0}")]
    Path(String),
    #[error("migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
}

/// Local device identity row stored under the fixed primary key 'self'.
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct LocalDeviceRow {
    pub device_id:   String,
    pub device_name: String,
    pub pubkey_b64:  String,
    pub privkey_b64: String,
}

/// A row in the trusted_peers table.
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct TrustedPeerRow {
    pub device_id:   String,
    pub device_name: String,
    pub pubkey_b64:  String,
    pub fingerprint: String,
    pub paired_at:   i64,
    pub last_seen:   Option<i64>,
}

/// A row in the transfer_history table (without the auto-increment id).
#[derive(Debug, sqlx::FromRow, Clone, serde::Serialize)]
pub struct TransferHistoryRow {
    pub peer_device_id: String,
    pub filename:       String,
    pub remote_path:    String,
    pub local_path:     String,
    pub size:           Option<i64>,
    pub bytes_received: i64,
    pub status:         String,
    pub started_at:     i64,
    pub completed_at:   Option<i64>,
    pub transfer_id:    Option<String>,
}

/// A row in the files index table.
#[derive(Debug, sqlx::FromRow, Clone)]
pub struct FileRow {
    pub name:          String,
    pub path:          String,
    pub parent_path:   String,
    pub file_type:     String,
    pub size:          Option<i64>,
    pub last_modified: Option<i64>,
    pub extension:     Option<String>,
    pub is_hidden:     i64,
}

/// A row in the applications table (a discovered installed application).
#[derive(Debug, sqlx::FromRow, Clone, serde::Serialize)]
pub struct AppRow {
    pub id:          i64,
    pub name:        String,
    pub exec:        String,
    pub icon_path:   Option<String>,
    pub platform:    String,
    pub source_path: String,
}

/// A row in the indexed_dirs table.
#[derive(Debug, sqlx::FromRow, Clone, serde::Serialize)]
pub struct IndexedDirRow {
    pub path:         String,
    pub file_count:   i64,
    pub last_indexed: Option<i64>,
}

/// Handle to the SQLite connection pool.
pub struct Db {
    pool: SqlitePool,
}

impl Db {
    /// Open or create the database at the platform data dir and run migrations.
    pub async fn open() -> Result<Self, DbError> {
        let mut path = dirs::data_dir().ok_or_else(|| DbError::Path("no data dir".into()))?;
        path.push("synapt");
        std::fs::create_dir_all(&path).map_err(|e| DbError::Path(e.to_string()))?;
        path.push("synapt.db");

        let url = format!("sqlite://{}?mode=rwc", path.display());
        let pool = SqlitePoolOptions::new().max_connections(4).connect(&url).await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    /// Get a setting value by key.
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let value = sqlx::query_scalar::<_, String>("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value)
    }

    /// Get every setting as a key/value map.
    pub async fn get_all_settings(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, DbError> {
        let rows = sqlx::query_as::<_, (String, String)>("SELECT key, value FROM settings")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().collect())
    }

    /// Set a setting value.
    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Load all trusted peers ordered by device_name.
    pub async fn get_trusted_peers(&self) -> Result<Vec<TrustedPeerRow>, DbError> {
        let rows = sqlx::query_as::<_, TrustedPeerRow>(
            "SELECT device_id, device_name, pubkey_b64, fingerprint, paired_at, last_seen \
             FROM trusted_peers ORDER BY device_name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Upsert a trusted peer (used after pairing).
    pub async fn upsert_trusted_peer(&self, peer: &TrustedPeerRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO trusted_peers \
               (device_id, device_name, pubkey_b64, fingerprint, paired_at, last_seen) \
             VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT(device_id) DO UPDATE SET \
               device_name = excluded.device_name, \
               pubkey_b64  = excluded.pubkey_b64, \
               fingerprint = excluded.fingerprint, \
               paired_at   = excluded.paired_at, \
               last_seen   = excluded.last_seen",
        )
        .bind(&peer.device_id)
        .bind(&peer.device_name)
        .bind(&peer.pubkey_b64)
        .bind(&peer.fingerprint)
        .bind(peer.paired_at)
        .bind(peer.last_seen)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Remove a trusted peer by device_id.
    pub async fn remove_trusted_peer(&self, device_id: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM trusted_peers WHERE device_id = ?")
            .bind(device_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Load the local device identity row.
    pub async fn get_local_device(&self) -> Result<Option<LocalDeviceRow>, DbError> {
        let row = sqlx::query_as::<_, LocalDeviceRow>(
            "SELECT device_id, device_name, pubkey_b64, privkey_b64 \
             FROM local_device WHERE id = 'self'",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Rename the local device identity row.
    pub async fn update_local_device_name(&self, name: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE local_device SET device_name = ? WHERE id = 'self'")
            .bind(name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert the local device identity (first run only).
    pub async fn insert_local_device(&self, row: &LocalDeviceRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT INTO local_device (id, device_id, device_name, pubkey_b64, privkey_b64) \
             VALUES ('self', ?, ?, ?, ?)",
        )
        .bind(&row.device_id)
        .bind(&row.device_name)
        .bind(&row.pubkey_b64)
        .bind(&row.privkey_b64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get shared directory paths.
    pub async fn get_shared_dirs(&self) -> Result<Vec<String>, DbError> {
        let paths = sqlx::query_scalar::<_, String>("SELECT path FROM shared_dirs ORDER BY path")
            .fetch_all(&self.pool)
            .await?;
        Ok(paths)
    }

    /// Add a shared directory.
    pub async fn add_shared_dir(&self, path: &str) -> Result<(), DbError> {
        sqlx::query("INSERT OR IGNORE INTO shared_dirs (path) VALUES (?)")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Remove a shared directory.
    pub async fn remove_shared_dir(&self, path: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM shared_dirs WHERE path = ?")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Insert a transfer history record, return the new row id.
    pub async fn insert_transfer(&self, t: &TransferHistoryRow) -> Result<i64, DbError> {
        let id = sqlx::query(
            "INSERT INTO transfer_history \
               (peer_device_id, filename, remote_path, local_path, size, \
                bytes_received, status, started_at, completed_at, transfer_id) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&t.peer_device_id)
        .bind(&t.filename)
        .bind(&t.remote_path)
        .bind(&t.local_path)
        .bind(t.size)
        .bind(t.bytes_received)
        .bind(&t.status)
        .bind(t.started_at)
        .bind(t.completed_at)
        .bind(&t.transfer_id)
        .execute(&self.pool)
        .await?
        .last_insert_rowid();
        Ok(id)
    }

    /// Update transfer status by id.
    pub async fn update_transfer_status(
        &self,
        id: i64,
        bytes_received: u64,
        status: &str,
        completed_at: Option<i64>,
    ) -> Result<(), DbError> {
        sqlx::query(
            "UPDATE transfer_history \
             SET bytes_received = ?, status = ?, completed_at = ? WHERE id = ?",
        )
        .bind(bytes_received as i64)
        .bind(status)
        .bind(completed_at)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Get all transfer history rows ordered by started_at DESC.
    pub async fn get_transfer_history(&self) -> Result<Vec<TransferHistoryRow>, DbError> {
        let rows = sqlx::query_as::<_, TransferHistoryRow>(
            "SELECT peer_device_id, filename, remote_path, local_path, size, \
                    bytes_received, status, started_at, completed_at, transfer_id \
             FROM transfer_history ORDER BY started_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Set only the status column for a transfer row, leaving progress untouched.
    pub async fn set_transfer_status(&self, id: i64, status: &str) -> Result<(), DbError> {
        sqlx::query("UPDATE transfer_history SET status = ? WHERE id = ?")
            .bind(status)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Read the current bytes_received for a transfer row.
    pub async fn get_transfer_bytes(&self, id: i64) -> Result<Option<i64>, DbError> {
        let bytes = sqlx::query_scalar::<_, i64>(
            "SELECT bytes_received FROM transfer_history WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(bytes)
    }

    /// Find the most recent partial transfer for a peer and remote path, if any.
    /// Returns (row id, transfer_id, bytes_received).
    pub async fn find_partial_transfer(
        &self,
        peer_device_id: &str,
        remote_path: &str,
    ) -> Result<Option<(i64, Option<String>, i64)>, DbError> {
        let row = sqlx::query_as::<_, (i64, Option<String>, i64)>(
            "SELECT id, transfer_id, bytes_received FROM transfer_history \
             WHERE peer_device_id = ? AND remote_path = ? AND status = 'partial' \
             ORDER BY started_at DESC LIMIT 1",
        )
        .bind(peer_device_id)
        .bind(remote_path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert or replace a single file row in the index.
    pub async fn upsert_file(&self, row: &FileRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT OR REPLACE INTO files \
               (name, path, parent_path, type, size, last_modified, extension, is_hidden) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&row.name)
        .bind(&row.path)
        .bind(&row.parent_path)
        .bind(&row.file_type)
        .bind(row.size)
        .bind(row.last_modified)
        .bind(&row.extension)
        .bind(row.is_hidden)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete an indexed file by its absolute path.
    pub async fn delete_file_by_path(&self, path: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM files WHERE path = ?")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Return every indexed file path.
    pub async fn get_all_file_paths(&self) -> Result<Vec<String>, DbError> {
        let paths = sqlx::query_scalar::<_, String>("SELECT path FROM files")
            .fetch_all(&self.pool)
            .await?;
        Ok(paths)
    }

    /// Return all configured indexed directories.
    pub async fn get_indexed_dirs(&self) -> Result<Vec<IndexedDirRow>, DbError> {
        let rows = sqlx::query_as::<_, IndexedDirRow>(
            "SELECT path, file_count, last_indexed FROM indexed_dirs ORDER BY path",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Return indexed directories with their file counts and last-scan times.
    pub async fn get_indexed_dir_stats(&self) -> Result<Vec<IndexedDirRow>, DbError> {
        let rows = sqlx::query_as::<_, IndexedDirRow>(
            "SELECT path, file_count, last_indexed FROM indexed_dirs ORDER BY path ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Add a directory to the search index set. No-op if already present.
    pub async fn add_indexed_dir(&self, path: &str) -> Result<(), DbError> {
        sqlx::query("INSERT OR IGNORE INTO indexed_dirs (path) VALUES (?)")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Remove a directory from the search index set.
    pub async fn remove_indexed_dir(&self, path: &str) -> Result<(), DbError> {
        sqlx::query("DELETE FROM indexed_dirs WHERE path = ?")
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Count the total number of indexed files.
    pub async fn count_files(&self) -> Result<i64, DbError> {
        let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM files")
            .fetch_one(&self.pool)
            .await?;
        Ok(count)
    }

    /// Most recent last_indexed timestamp across all indexed directories.
    pub async fn get_last_scan(&self) -> Result<Option<i64>, DbError> {
        let ts = sqlx::query_scalar::<_, Option<i64>>("SELECT MAX(last_indexed) FROM indexed_dirs")
            .fetch_one(&self.pool)
            .await?;
        Ok(ts)
    }

    /// Update the cached file count and last-indexed time for a directory.
    pub async fn update_indexed_dir_count(&self, path: &str, count: i64) -> Result<(), DbError> {
        sqlx::query("UPDATE indexed_dirs SET file_count = ?, last_indexed = unixepoch() WHERE path = ?")
            .bind(count)
            .bind(path)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Load every indexed file ordered by name, for rebuilding the full-text index.
    pub async fn get_all_files_for_index(&self) -> Result<Vec<FileRow>, DbError> {
        let rows = sqlx::query_as::<_, FileRow>(
            "SELECT name, path, parent_path, type AS file_type, size, last_modified, \
                    extension, is_hidden \
             FROM files ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Substring search over indexed file names.
    pub async fn search_files_by_name(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<FileRow>, DbError> {
        let rows = sqlx::query_as::<_, FileRow>(
            "SELECT name, path, parent_path, type AS file_type, size, last_modified, \
                    extension, is_hidden \
             FROM files WHERE name LIKE ? LIMIT ?",
        )
        .bind(format!("%{}%", query))
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Insert or replace a discovered application, keyed on its unique source path.
    pub async fn upsert_app(&self, row: &AppRow) -> Result<(), DbError> {
        sqlx::query(
            "INSERT OR REPLACE INTO applications \
               (name, exec, icon_path, platform, source_path) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&row.name)
        .bind(&row.exec)
        .bind(&row.icon_path)
        .bind(&row.platform)
        .bind(&row.source_path)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Return every indexed application ordered by name.
    pub async fn get_all_apps(&self) -> Result<Vec<AppRow>, DbError> {
        let rows = sqlx::query_as::<_, AppRow>(
            "SELECT id, name, exec, icon_path, platform, source_path \
             FROM applications ORDER BY name ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Substring search over indexed application names, capped at 20 results.
    pub async fn search_apps_by_name(&self, query: &str) -> Result<Vec<AppRow>, DbError> {
        let rows = sqlx::query_as::<_, AppRow>(
            "SELECT id, name, exec, icon_path, platform, source_path \
             FROM applications WHERE name LIKE ? ORDER BY name ASC LIMIT 20",
        )
        .bind(format!("%{}%", query))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Remove all indexed applications (called before a fresh app scan).
    pub async fn clear_apps(&self) -> Result<(), DbError> {
        sqlx::query("DELETE FROM applications").execute(&self.pool).await?;
        Ok(())
    }

    /// Open an in-memory database with migrations applied, for tests only.
    #[cfg(test)]
    pub async fn open_in_memory() -> Result<Self, DbError> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }
}
