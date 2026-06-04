use std::fmt::Write as _;
use std::io::SeekFrom;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;
use crate::network::discovery::TRANSFER_PORT;
use crate::network::queue::{QueueEntry, QueueStatus, TransferQueue};
use crate::network::{
    read_enc, session_handshake_client, session_handshake_server, write_enc, SessionError,
};
use crate::storage::{Db, DbError, TransferHistoryRow};
use crate::trust::LocalIdentity;

/// Size of each streamed file chunk in bytes; the final chunk may be smaller.
pub const CHUNK_SIZE: usize = 65536;

/// Exponential backoff delays (seconds) between transfer retries; never exceeds 30s.
pub const RETRY_DELAYS_SECS: &[u64] = &[1, 2, 4, 8, 16, 30];

/// Errors raised by the file transfer subsystem.
#[derive(Debug, Error)]
pub enum TransferError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("access denied")]
    AccessDenied,
    #[error("invalid resume offset")]
    InvalidOffset,
    #[error("checksum mismatch")]
    Checksum,
    #[error("peer not trusted")]
    NotTrusted,
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("session error: {0}")]
    Session(#[from] SessionError),
    #[error("protocol error: {0}")]
    Protocol(String),
}

/// Encrypted control messages exchanged after the session key is established.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum TransferMsg {
    TransferRequest { path: String, resume_offset: u64 },
    BatchTransferRequest { paths: Vec<String> },
    TransferStart { filename: String, size: u64, resume_offset: u64 },
    TransferComplete { checksum: String },
    FileSkipped { path: String, reason: String },
    BatchComplete { total_files: u64, skipped: u64 },
    PushStart { filename: String, size: u64, checksum: String },
    PushAccept,
    Error { code: String },
}

/// Lowercase hex encoding of a byte slice.
fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Compute the SHA-256 of an entire file as a lowercase hex string.
async fn sha256_of_file(path: &Path) -> Result<String, TransferError> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(&hasher.finalize()))
}

/// Returns true if the canonicalised requested path is under any of shared_dirs.
pub fn is_path_allowed(requested: &Path, shared_dirs: &[String]) -> bool {
    let canonical = match std::fs::canonicalize(requested) {
        Ok(p) => p,
        Err(_) => return false,
    };
    for dir in shared_dirs {
        if let Ok(canonical_dir) = std::fs::canonicalize(dir) {
            if canonical.starts_with(&canonical_dir) {
                return true;
            }
        }
    }
    false
}

/// Run the encrypted file transfer server, accepting inbound transfer connections.
pub async fn start_transfer_server(
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    app: AppHandle,
    queue: Arc<TransferQueue>,
) -> Result<(), TransferError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{TRANSFER_PORT}")).await?;
    tracing::info!("transfer server listening on port {}", TRANSFER_PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let identity = Arc::clone(&identity);
        let db = Arc::clone(&db);
        let app = app.clone();
        let queue = Arc::clone(&queue);
        tokio::spawn(async move {
            if let Err(e) = handle_server_conn(stream, addr, identity, db, app, queue).await {
                tracing::warn!("transfer server error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_server_conn(
    mut stream: TcpStream,
    addr: SocketAddr,
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    app: AppHandle,
    queue: Arc<TransferQueue>,
) -> Result<(), TransferError> {
    let (key, peer_device_id) = session_handshake_server(&mut stream, &identity, &db).await?;

    match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
        TransferMsg::TransferRequest { path, resume_offset } => {
            serve_one_file(&mut stream, &key, &path, resume_offset, &db, addr).await
        }
        TransferMsg::BatchTransferRequest { paths } => {
            serve_batch(&mut stream, &key, paths, &db, addr).await
        }
        TransferMsg::PushStart { filename, size, checksum } => {
            recv_pushed_file(
                &mut stream, &key, &peer_device_id, filename, size, checksum, &db, &app, &queue,
            )
            .await
        }
        other => Err(TransferError::Protocol(format!("unexpected {other:?}"))),
    }
}

/// Receive a file pushed by a trusted peer, writing it under the download dir.
#[allow(clippy::too_many_arguments)]
async fn recv_pushed_file(
    stream: &mut TcpStream,
    key: &[u8; 32],
    peer_device_id: &str,
    filename: String,
    size: u64,
    checksum: String,
    db: &Db,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<(), TransferError> {
    let safe_name = Path::new(&filename)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());

    let download_dir = match db.get_setting("download_dir").await? {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => dirs::download_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Synapt"),
    };

    let peer_name = db
        .get_trusted_peers()
        .await?
        .into_iter()
        .find(|p| p.device_id == peer_device_id)
        .map(|p| p.device_name)
        .unwrap_or_else(|| peer_device_id.to_string());

    let dir = download_dir.join(&peer_name);
    tokio::fs::create_dir_all(&dir).await?;
    let local_path = dir.join(&safe_name);

    let transfer_id = Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now().timestamp();
    let hist_id = db
        .insert_transfer(&TransferHistoryRow {
            peer_device_id: peer_device_id.to_string(),
            filename: safe_name.clone(),
            remote_path: filename.clone(),
            local_path: local_path.to_string_lossy().to_string(),
            size: Some(size as i64),
            bytes_received: 0,
            status: "in_progress".into(),
            started_at,
            completed_at: None,
            transfer_id: Some(transfer_id.clone()),
        })
        .await?;

    queue.push(QueueEntry {
        transfer_id: transfer_id.clone(),
        filename: safe_name.clone(),
        remote_path: filename.clone(),
        peer_name: peer_name.clone(),
        status: QueueStatus::InProgress,
        bytes_received: 0,
        total: size,
        started_at,
    });

    write_enc(stream, key, &serde_json::to_vec(&TransferMsg::PushAccept)?).await?;

    let mut file = tokio::fs::File::create(&local_path).await?;
    let mut bytes_received = 0u64;
    while bytes_received < size {
        let chunk = read_enc(stream, key).await?;
        if chunk.is_empty() {
            break;
        }
        file.write_all(&chunk).await?;
        bytes_received += chunk.len() as u64;
        db.update_transfer_status(hist_id, bytes_received, "in_progress", None).await?;
        queue.update(&transfer_id, QueueStatus::InProgress, bytes_received);
        let _ = app.emit(
            "transfer-progress",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": safe_name,
                "bytes_received": bytes_received,
                "total": size,
                "peer_name": peer_name,
            }),
        );
    }
    file.flush().await?;

    let actual = sha256_of_file(&local_path).await?;
    let now = chrono::Utc::now().timestamp();
    if actual == checksum {
        db.update_transfer_status(hist_id, bytes_received, "complete", Some(now)).await?;
        queue.update(&transfer_id, QueueStatus::Complete, bytes_received);
        let _ = app.emit(
            "transfer-complete",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": safe_name,
                "local_path": local_path.to_string_lossy(),
                "peer_name": peer_name,
            }),
        );
        if crate::notify::enabled(db).await {
            crate::notify::transfer_complete(app, &safe_name, &peer_name);
        }
        Ok(())
    } else {
        tracing::warn!("pushed file checksum mismatch for {}", safe_name);
        let _ = tokio::fs::remove_file(&local_path).await;
        db.update_transfer_status(hist_id, 0, "failed", Some(now)).await?;
        queue.update(
            &transfer_id,
            QueueStatus::Failed { reason: "checksum_mismatch".into() },
            0,
        );
        let _ = app.emit(
            "transfer-failed",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": safe_name,
                "reason": "checksum_mismatch",
            }),
        );
        Err(TransferError::Checksum)
    }
}

/// Classify each requested path as allowed/denied against the shared dirs,
/// preserving request order.
fn classify_batch(paths: &[String], shared_dirs: &[String]) -> Vec<(String, bool)> {
    paths
        .iter()
        .map(|p| (p.clone(), is_path_allowed(Path::new(p), shared_dirs)))
        .collect()
}

/// Serve multiple files sequentially over one session, skipping denied paths.
async fn serve_batch(
    stream: &mut TcpStream,
    key: &[u8; 32],
    paths: Vec<String>,
    db: &Db,
    addr: SocketAddr,
) -> Result<(), TransferError> {
    let total_files = paths.len() as u64;
    let shared_dirs = db.get_shared_dirs().await?;
    let mut skipped = 0u64;

    for (path, allowed) in classify_batch(&paths, &shared_dirs) {
        if !allowed {
            skipped += 1;
            tracing::warn!("batch transfer: access denied for a path requested from {}", addr);
            write_enc(
                stream,
                key,
                &serde_json::to_vec(&TransferMsg::FileSkipped {
                    path,
                    reason: "access_denied".into(),
                })?,
            )
            .await?;
            continue;
        }
        serve_one_file(stream, key, &path, 0, db, addr).await?;
    }

    write_enc(
        stream,
        key,
        &serde_json::to_vec(&TransferMsg::BatchComplete { total_files, skipped })?,
    )
    .await?;
    Ok(())
}

/// Validate, then stream a single requested file over an established session.
async fn serve_one_file(
    stream: &mut TcpStream,
    key: &[u8; 32],
    path: &str,
    resume_offset: u64,
    db: &Db,
    addr: SocketAddr,
) -> Result<(), TransferError> {
    let shared_dirs = db.get_shared_dirs().await?;
    if !is_path_allowed(Path::new(path), &shared_dirs) {
        tracing::warn!("transfer: access denied for a path requested from {}", addr);
        write_enc(stream, key, &serde_json::to_vec(&TransferMsg::Error { code: "access_denied".into() })?).await?;
        return Ok(());
    }

    let p = Path::new(path);
    let mut file = match tokio::fs::File::open(p).await {
        Ok(f) => f,
        Err(_) => {
            write_enc(stream, key, &serde_json::to_vec(&TransferMsg::Error { code: "access_denied".into() })?).await?;
            return Ok(());
        }
    };
    let size = file.metadata().await?.len();

    if resume_offset > size {
        write_enc(stream, key, &serde_json::to_vec(&TransferMsg::Error { code: "invalid_offset".into() })?).await?;
        return Ok(());
    }

    let checksum = sha256_of_file(p).await?;
    file.seek(SeekFrom::Start(resume_offset)).await?;

    let filename = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());

    write_enc(
        stream,
        key,
        &serde_json::to_vec(&TransferMsg::TransferStart { filename, size, resume_offset })?,
    )
    .await?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        write_enc(stream, key, &buf[..n]).await?;
    }

    write_enc(stream, key, &serde_json::to_vec(&TransferMsg::TransferComplete { checksum })?).await?;
    Ok(())
}

/// Request a file from a trusted peer, streaming it to the download directory.
///
/// On the first attempt pass `hist_id = None`; the history row is created once the
/// transfer starts and the assigned id is written back through the mutable handle so
/// subsequent retries reuse the same row.
#[allow(clippy::too_many_arguments)]
pub async fn request_transfer(
    transfer_id: &str,
    hist_id: &mut Option<i64>,
    peer_device_id: &str,
    peer_ip: IpAddr,
    remote_path: String,
    resume_offset: u64,
    identity: &LocalIdentity,
    db: &Db,
    download_dir: &Path,
    peer_name: &str,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<PathBuf, TransferError> {
    let started_at = chrono::Utc::now().timestamp();
    queue.push(QueueEntry {
        transfer_id: transfer_id.to_string(),
        filename: Path::new(&remote_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into()),
        remote_path: remote_path.clone(),
        peer_name: peer_name.to_string(),
        status: QueueStatus::Queued,
        bytes_received: resume_offset,
        total: 0,
        started_at,
    });

    let mut stream = TcpStream::connect((peer_ip, TRANSFER_PORT)).await?;

    let peer_pubkey = {
        let peers = db.get_trusted_peers().await?;
        let peer = peers
            .iter()
            .find(|p| p.device_id == peer_device_id)
            .ok_or(TransferError::NotTrusted)?;
        peer.pubkey_b64.clone()
    };
    let local_id = identity.device_id.to_string();
    let key = session_handshake_client(&mut stream, &local_id, identity, &peer_pubkey).await?;

    write_enc(
        &mut stream,
        &key,
        &serde_json::to_vec(&TransferMsg::TransferRequest { path: remote_path.clone(), resume_offset })?,
    )
    .await?;

    let (filename, size, server_offset) =
        match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
            TransferMsg::TransferStart { filename, size, resume_offset } => (filename, size, resume_offset),
            TransferMsg::Error { code } => {
                return Err(match code.as_str() {
                    "access_denied" => TransferError::AccessDenied,
                    "invalid_offset" => TransferError::InvalidOffset,
                    other => TransferError::Protocol(other.to_string()),
                });
            }
            other => return Err(TransferError::Protocol(format!("unexpected {other:?}"))),
        };

    let dir = download_dir.join(peer_name);
    tokio::fs::create_dir_all(&dir).await?;
    let local_path = dir.join(&filename);

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(server_offset == 0)
        .open(&local_path)
        .await?;
    if server_offset > 0 {
        file.seek(SeekFrom::Start(server_offset)).await?;
    }

    let id = match *hist_id {
        Some(id) => {
            db.set_transfer_status(id, "in_progress").await?;
            id
        }
        None => {
            let new_id = db
                .insert_transfer(&TransferHistoryRow {
                    peer_device_id: peer_device_id.to_string(),
                    filename: filename.clone(),
                    remote_path: remote_path.clone(),
                    local_path: local_path.to_string_lossy().to_string(),
                    size: Some(size as i64),
                    bytes_received: server_offset as i64,
                    status: "in_progress".into(),
                    started_at: chrono::Utc::now().timestamp(),
                    completed_at: None,
                    transfer_id: Some(transfer_id.to_string()),
                })
                .await?;
            *hist_id = Some(new_id);
            new_id
        }
    };

    queue.push(QueueEntry {
        transfer_id: transfer_id.to_string(),
        filename: filename.clone(),
        remote_path: remote_path.clone(),
        peer_name: peer_name.to_string(),
        status: QueueStatus::InProgress,
        bytes_received: server_offset,
        total: size,
        started_at,
    });

    let mut bytes_received = server_offset;
    while bytes_received < size {
        let chunk = read_enc(&mut stream, &key).await?;
        if chunk.is_empty() {
            break;
        }
        file.write_all(&chunk).await?;
        bytes_received += chunk.len() as u64;
        db.update_transfer_status(id, bytes_received, "in_progress", None).await?;
        queue.update(transfer_id, QueueStatus::InProgress, bytes_received);
        let _ = app.emit(
            "transfer-progress",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": filename,
                "bytes_received": bytes_received,
                "total": size,
                "peer_name": peer_name,
            }),
        );
    }
    file.flush().await?;

    let checksum = match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
        TransferMsg::TransferComplete { checksum } => checksum,
        other => return Err(TransferError::Protocol(format!("unexpected {other:?}"))),
    };

    let actual = sha256_of_file(&local_path).await?;
    let now = chrono::Utc::now().timestamp();
    if actual == checksum {
        db.update_transfer_status(id, bytes_received, "complete", Some(now)).await?;
        queue.update(transfer_id, QueueStatus::Complete, bytes_received);
        let _ = app.emit(
            "transfer-complete",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": filename,
                "local_path": local_path.to_string_lossy(),
                "peer_name": peer_name,
            }),
        );
        if crate::notify::enabled(db).await {
            crate::notify::transfer_complete(app, &filename, peer_name);
        }
        Ok(local_path)
    } else {
        tracing::warn!("transfer checksum mismatch for {}", filename);
        let _ = tokio::fs::remove_file(&local_path).await;
        db.update_transfer_status(id, 0, "failed", Some(now)).await?;
        queue.update(
            transfer_id,
            QueueStatus::Failed { reason: "checksum_mismatch".into() },
            0,
        );
        let _ = app.emit(
            "transfer-failed",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": filename,
                "reason": "checksum_mismatch",
            }),
        );
        Err(TransferError::Checksum)
    }
}

/// Request a file with exponential backoff retry, resuming from the bytes already
/// received on each attempt.
#[allow(clippy::too_many_arguments)]
pub async fn request_transfer_with_retry(
    peer_device_id: &str,
    peer_ip: IpAddr,
    remote_path: String,
    identity: &LocalIdentity,
    db: &Db,
    download_dir: &Path,
    peer_device_name: &str,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<PathBuf, TransferError> {
    let (transfer_id, mut hist_id, mut resume_offset) =
        match db.find_partial_transfer(peer_device_id, &remote_path).await? {
            Some((id, tid, bytes)) => (
                tid.unwrap_or_else(|| Uuid::new_v4().to_string()),
                Some(id),
                bytes.max(0) as u64,
            ),
            None => (Uuid::new_v4().to_string(), None, 0u64),
        };

    let mut attempt = 0usize;
    loop {
        match request_transfer(
            &transfer_id,
            &mut hist_id,
            peer_device_id,
            peer_ip,
            remote_path.clone(),
            resume_offset,
            identity,
            db,
            download_dir,
            peer_device_name,
            app,
            queue,
        )
        .await
        {
            Ok(path) => return Ok(path),
            Err(e) => {
                if attempt < RETRY_DELAYS_SECS.len() {
                    let delay = RETRY_DELAYS_SECS[attempt];
                    tracing::warn!(
                        "transfer attempt {} failed: {}; retrying in {}s",
                        attempt + 1,
                        e,
                        delay
                    );
                    if let Some(id) = hist_id {
                        let _ = db.set_transfer_status(id, "partial").await;
                    }
                    queue.update(&transfer_id, QueueStatus::Partial, resume_offset);
                    let _ = app.emit(
                        "transfer-retry",
                        serde_json::json!({
                            "transfer_id": transfer_id,
                            "attempt": attempt + 1,
                            "delay_secs": delay,
                            "reason": e.to_string(),
                        }),
                    );
                    tokio::time::sleep(Duration::from_secs(delay)).await;
                    if let Some(id) = hist_id {
                        if let Ok(Some(bytes)) = db.get_transfer_bytes(id).await {
                            resume_offset = bytes.max(0) as u64;
                        }
                    }
                    attempt += 1;
                } else {
                    let bytes = match hist_id {
                        Some(id) => {
                            let bytes = db.get_transfer_bytes(id).await.ok().flatten().unwrap_or(0).max(0) as u64;
                            let _ = db
                                .update_transfer_status(
                                    id,
                                    bytes,
                                    "failed",
                                    Some(chrono::Utc::now().timestamp()),
                                )
                                .await;
                            bytes
                        }
                        None => 0,
                    };
                    queue.update(
                        &transfer_id,
                        QueueStatus::Failed { reason: e.to_string() },
                        bytes,
                    );
                    return Err(e);
                }
            }
        }
    }
}

/// Request multiple files from a peer, each with independent retry. Files that
/// ultimately fail are skipped; returns the paths of the successful transfers.
#[allow(clippy::too_many_arguments)]
pub async fn request_batch_transfer_with_retry(
    peer_device_id: &str,
    peer_ip: IpAddr,
    remote_paths: Vec<String>,
    identity: &LocalIdentity,
    db: &Db,
    download_dir: &Path,
    peer_device_name: &str,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<Vec<PathBuf>, TransferError> {
    let total_files = remote_paths.len() as u64;
    let mut skipped = 0u64;
    let mut paths = Vec::new();

    for remote_path in remote_paths {
        match request_transfer_with_retry(
            peer_device_id,
            peer_ip,
            remote_path.clone(),
            identity,
            db,
            download_dir,
            peer_device_name,
            app,
            queue,
        )
        .await
        {
            Ok(path) => paths.push(path),
            Err(e) => {
                skipped += 1;
                tracing::warn!("batch transfer: file {} failed: {}", remote_path, e);
            }
        }
    }

    let _ = app.emit(
        "batch-complete",
        serde_json::json!({
            "total_files": total_files,
            "skipped": skipped,
            "peer_name": peer_device_name,
        }),
    );

    Ok(paths)
}

/// Push (send) local files to a trusted peer. Files that fail are logged and
/// skipped; the remaining files continue.
#[allow(clippy::too_many_arguments)]
pub async fn push_files(
    peer_device_id: &str,
    peer_ip: IpAddr,
    local_paths: Vec<String>,
    identity: &LocalIdentity,
    db: &Db,
    peer_name: &str,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<(), TransferError> {
    let peer_pubkey = {
        let peers = db.get_trusted_peers().await?;
        let peer = peers
            .iter()
            .find(|p| p.device_id == peer_device_id)
            .ok_or(TransferError::NotTrusted)?;
        peer.pubkey_b64.clone()
    };
    let local_id = identity.device_id.to_string();

    for local_path in local_paths {
        if let Err(e) = push_one_file(
            &local_path, peer_ip, &local_id, identity, &peer_pubkey, peer_name, app, queue,
        )
        .await
        {
            tracing::warn!("push of {} failed: {}", local_path, e);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn push_one_file(
    local_path: &str,
    peer_ip: IpAddr,
    local_id: &str,
    identity: &LocalIdentity,
    peer_pubkey: &str,
    peer_name: &str,
    app: &AppHandle,
    queue: &TransferQueue,
) -> Result<(), TransferError> {
    let p = Path::new(local_path);
    let meta = tokio::fs::metadata(p).await?;
    if !meta.is_file() {
        return Err(TransferError::Protocol("not a file".into()));
    }
    let size = meta.len();
    let filename = p
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let checksum = sha256_of_file(p).await?;

    let transfer_id = Uuid::new_v4().to_string();
    let started_at = chrono::Utc::now().timestamp();
    queue.push(QueueEntry {
        transfer_id: transfer_id.clone(),
        filename: filename.clone(),
        remote_path: local_path.to_string(),
        peer_name: peer_name.to_string(),
        status: QueueStatus::InProgress,
        bytes_received: 0,
        total: size,
        started_at,
    });

    let mut stream = TcpStream::connect((peer_ip, TRANSFER_PORT)).await?;
    let key = session_handshake_client(&mut stream, local_id, identity, peer_pubkey).await?;

    write_enc(
        &mut stream,
        &key,
        &serde_json::to_vec(&TransferMsg::PushStart {
            filename: filename.clone(),
            size,
            checksum,
        })?,
    )
    .await?;

    match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
        TransferMsg::PushAccept => {}
        TransferMsg::Error { code } => return Err(TransferError::Protocol(code)),
        other => return Err(TransferError::Protocol(format!("unexpected {other:?}"))),
    }

    let mut file = tokio::fs::File::open(p).await?;
    let mut buf = vec![0u8; CHUNK_SIZE];
    let mut sent = 0u64;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        write_enc(&mut stream, &key, &buf[..n]).await?;
        sent += n as u64;
        queue.update(&transfer_id, QueueStatus::InProgress, sent);
        let _ = app.emit(
            "transfer-progress",
            serde_json::json!({
                "transfer_id": transfer_id,
                "filename": filename,
                "bytes_received": sent,
                "total": size,
                "peer_name": peer_name,
            }),
        );
    }

    queue.update(&transfer_id, QueueStatus::Complete, sent);
    let _ = app.emit(
        "transfer-complete",
        serde_json::json!({
            "transfer_id": transfer_id,
            "filename": filename,
            "local_path": local_path,
            "peer_name": peer_name,
        }),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_base() -> PathBuf {
        std::env::temp_dir().join(format!("synapt-test-{}", uuid::Uuid::new_v4()))
    }

    #[test]
    fn chunk_size_is_65536() {
        assert_eq!(CHUNK_SIZE, 65536);
    }

    #[test]
    fn retry_delays_has_six_entries() {
        assert_eq!(RETRY_DELAYS_SECS.len(), 6);
    }

    #[test]
    fn retry_last_delay_is_30() {
        assert_eq!(*RETRY_DELAYS_SECS.last().unwrap(), 30);
    }

    #[test]
    fn retry_delays_strictly_increasing() {
        for w in RETRY_DELAYS_SECS.windows(2) {
            assert!(w[1] > w[0]);
        }
    }

    #[test]
    fn path_outside_shared_is_rejected() {
        let base = unique_base();
        let shared = base.join("shared");
        let outside = base.join("outside");
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let f = outside.join("secret.txt");
        fs::write(&f, b"x").unwrap();
        assert!(!is_path_allowed(&f, &[shared.to_string_lossy().to_string()]));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn path_inside_shared_is_allowed() {
        let base = unique_base();
        let shared = base.join("shared");
        fs::create_dir_all(&shared).unwrap();
        let f = shared.join("ok.txt");
        fs::write(&f, b"x").unwrap();
        assert!(is_path_allowed(&f, &[shared.to_string_lossy().to_string()]));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn path_that_does_not_exist_returns_false_not_panic() {
        let base = unique_base();
        let shared = base.join("shared");
        fs::create_dir_all(&shared).unwrap();
        let missing = shared.join("nope.txt");
        assert!(!is_path_allowed(&missing, &[shared.to_string_lossy().to_string()]));
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn batch_with_empty_paths_yields_no_transfers() {
        let allowed = classify_batch(&[], &["/tmp".to_string()]);
        assert!(allowed.is_empty());
    }

    #[test]
    fn batch_skips_denied_paths_and_keeps_allowed() {
        let base = unique_base();
        let shared = base.join("shared");
        let outside = base.join("outside");
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let ok = shared.join("ok.txt");
        let denied = outside.join("secret.txt");
        fs::write(&ok, b"x").unwrap();
        fs::write(&denied, b"x").unwrap();

        let dirs = vec![shared.to_string_lossy().to_string()];
        let classified = classify_batch(
            &[ok.to_string_lossy().to_string(), denied.to_string_lossy().to_string()],
            &dirs,
        );
        let allowed_count = classified.iter().filter(|(_, a)| *a).count();
        let denied_count = classified.iter().filter(|(_, a)| !*a).count();
        assert_eq!(allowed_count, 1);
        assert_eq!(denied_count, 1);
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn path_with_traversal_is_rejected() {
        let base = unique_base();
        let shared = base.join("shared");
        let outside = base.join("outside");
        fs::create_dir_all(&shared).unwrap();
        fs::create_dir_all(&outside).unwrap();
        let secret = outside.join("secret.txt");
        fs::write(&secret, b"x").unwrap();
        let traversal = shared.join("..").join("outside").join("secret.txt");
        assert!(!is_path_allowed(&traversal, &[shared.to_string_lossy().to_string()]));
        let _ = fs::remove_dir_all(&base);
    }
}
