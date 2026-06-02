use std::io::SeekFrom;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tauri::{AppHandle, Emitter};
use crate::network::discovery::TRANSFER_PORT;
use crate::network::{
    read_enc, session_handshake_client, session_handshake_server, write_enc, SessionError,
};
use crate::storage::{Db, DbError, TransferHistoryRow};
use crate::trust::LocalIdentity;

const CHUNK_SIZE: usize = 65536;

/// Errors raised by the file transfer subsystem.
#[derive(Debug, Error)]
pub enum TransferError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("access denied")]
    AccessDenied,
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
    TransferStart { filename: String, size: u64 },
    TransferComplete,
    Error { code: String },
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
    _app: AppHandle,
) -> Result<(), TransferError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{TRANSFER_PORT}")).await?;
    tracing::info!("transfer server listening on port {}", TRANSFER_PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let identity = Arc::clone(&identity);
        let db = Arc::clone(&db);
        tokio::spawn(async move {
            if let Err(e) = handle_server_conn(stream, addr, identity, db).await {
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
) -> Result<(), TransferError> {
    let key = session_handshake_server(&mut stream, &identity, &db).await?;

    let (path, resume_offset) = match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
        TransferMsg::TransferRequest { path, resume_offset } => (path, resume_offset),
        other => return Err(TransferError::Protocol(format!("unexpected {other:?}"))),
    };

    let shared_dirs = db.get_shared_dirs().await?;
    if !is_path_allowed(Path::new(&path), &shared_dirs) {
        tracing::warn!("transfer: access denied for a path requested from {}", addr);
        write_enc(&mut stream, &key, &serde_json::to_vec(&TransferMsg::Error { code: "access_denied".into() })?).await?;
        return Ok(());
    }

    let mut file = tokio::fs::File::open(&path).await?;
    let size = file.metadata().await?.len();
    if resume_offset > 0 {
        file.seek(SeekFrom::Start(resume_offset)).await?;
    }
    let filename = Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());

    write_enc(&mut stream, &key, &serde_json::to_vec(&TransferMsg::TransferStart { filename, size })?).await?;

    let mut buf = vec![0u8; CHUNK_SIZE];
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        write_enc(&mut stream, &key, &buf[..n]).await?;
    }

    write_enc(&mut stream, &key, &serde_json::to_vec(&TransferMsg::TransferComplete)?).await?;
    Ok(())
}

/// Request a file from a trusted peer and stream it to the download directory.
#[allow(clippy::too_many_arguments)]
pub async fn request_transfer(
    peer_device_id: &str,
    peer_ip: IpAddr,
    remote_path: String,
    resume_offset: u64,
    identity: &LocalIdentity,
    db: &Db,
    download_dir: &Path,
    peer_name: &str,
    app: &AppHandle,
) -> Result<PathBuf, TransferError> {
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

    let (filename, size) = match serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)? {
        TransferMsg::TransferStart { filename, size } => (filename, size),
        TransferMsg::Error { code } => {
            return Err(if code == "access_denied" {
                TransferError::AccessDenied
            } else {
                TransferError::Protocol(code)
            });
        }
        other => return Err(TransferError::Protocol(format!("unexpected {other:?}"))),
    };

    let dir = download_dir.join(peer_name);
    tokio::fs::create_dir_all(&dir).await?;
    let local_path = dir.join(&filename);

    let started_at = chrono::Utc::now().timestamp();
    let hist_id = db
        .insert_transfer(&TransferHistoryRow {
            peer_device_id: peer_device_id.to_string(),
            filename: filename.clone(),
            remote_path: remote_path.clone(),
            local_path: local_path.to_string_lossy().to_string(),
            size: Some(size as i64),
            bytes_received: resume_offset as i64,
            status: "in_progress".into(),
            started_at,
            completed_at: None,
        })
        .await?;

    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(resume_offset == 0)
        .open(&local_path)
        .await?;
    if resume_offset > 0 {
        file.seek(SeekFrom::Start(resume_offset)).await?;
    }

    let mut bytes_received = resume_offset;
    while bytes_received < size {
        let chunk = read_enc(&mut stream, &key).await?;
        if chunk.is_empty() {
            break;
        }
        file.write_all(&chunk).await?;
        bytes_received += chunk.len() as u64;
        let _ = app.emit(
            "transfer-progress",
            serde_json::json!({ "filename": filename, "bytes_received": bytes_received, "total": size }),
        );
    }

    if let Ok(TransferMsg::TransferComplete) =
        serde_json::from_slice::<TransferMsg>(&read_enc(&mut stream, &key).await?)
    {
        tracing::info!("transfer complete: {}", filename);
    }

    file.flush().await?;
    db.update_transfer_status(hist_id, bytes_received, "complete", Some(chrono::Utc::now().timestamp()))
        .await?;
    let _ = app.emit(
        "transfer-complete",
        serde_json::json!({ "filename": filename, "local_path": local_path.to_string_lossy() }),
    );

    Ok(local_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn unique_base() -> PathBuf {
        std::env::temp_dir().join(format!("synapt-test-{}", uuid::Uuid::new_v4()))
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
