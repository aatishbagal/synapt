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
                bytes_received, status, started_at, completed_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
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
                    bytes_received, status, started_at, completed_at \
             FROM transfer_history ORDER BY started_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
