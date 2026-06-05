use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

use crate::network::discovery::SEARCH_PORT;
use crate::network::transfer::is_path_allowed;
use crate::network::{read_enc, session_handshake_server, write_enc, SessionError};
use crate::search::engine::{ResultType, SearchEngine, SearchResult};
use crate::storage::Db;
use crate::trust::LocalIdentity;

/// Errors raised by the remote search server.
#[derive(Debug, Error)]
pub enum SearchServerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("session: {0}")]
    Session(#[from] SessionError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("search: {0}")]
    Search(#[from] crate::search::engine::SearchError),
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
}

/// Encrypted search protocol messages exchanged after the session handshake.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SearchMsg {
    SearchRequest { query: String, max_results: usize },
    SearchResults { results: Vec<SearchResult> },
    LaunchRequest { source_path: String },
    LaunchResult { success: bool, error: Option<String> },
}

/// Run the remote search server, serving trusted peers on the search port.
pub async fn start_search_server(
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    search_engine: Arc<SearchEngine>,
) -> Result<(), SearchServerError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{SEARCH_PORT}")).await?;
    tracing::info!("search server listening on port {}", SEARCH_PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let identity = Arc::clone(&identity);
        let db = Arc::clone(&db);
        let engine = Arc::clone(&search_engine);
        tokio::spawn(async move {
            if let Err(e) = handle_search_conn(stream, identity, db, engine).await {
                tracing::warn!("search server error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_search_conn(
    mut stream: TcpStream,
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    engine: Arc<SearchEngine>,
) -> Result<(), SearchServerError> {
    // The session handshake only completes for trusted peers, so reaching this
    // point already satisfies the "requesting device must be trusted" check.
    let (key, _peer_id) = session_handshake_server(&mut stream, &identity, &db).await?;

    let msg = serde_json::from_slice::<SearchMsg>(&read_enc(&mut stream, &key).await?)?;
    match msg {
        SearchMsg::SearchRequest { query, max_results } => {
            let results = engine.search(&query, max_results).await?;
            let shared_dirs = db.get_shared_dirs().await?;
            let filtered: Vec<SearchResult> = results
                .into_iter()
                // File results are gated by the shared-dirs allow-list. App
                // results are launched (not transferred) and validated again by
                // remote_launch_app, so they are surfaced to trusted peers.
                .filter(|r| {
                    matches!(r.result_type, ResultType::App)
                        || is_path_allowed(Path::new(&r.path), &shared_dirs)
                })
                .map(|mut r| {
                    embed_app_icon(&mut r);
                    r
                })
                .collect();

            let response = SearchMsg::SearchResults { results: filtered };
            write_enc(&mut stream, &key, &serde_json::to_vec(&response)?).await?;
        }
        SearchMsg::LaunchRequest { source_path } => {
            let response = match handle_launch_request(&db, &source_path).await {
                Ok(()) => SearchMsg::LaunchResult { success: true, error: None },
                Err(error) => SearchMsg::LaunchResult { success: false, error: Some(error) },
            };
            write_enc(&mut stream, &key, &serde_json::to_vec(&response)?).await?;
        }
        _ => return Ok(()),
    }
    Ok(())
}

/// Inline an app result's icon as a base64 data URI so a remote peer can render
/// it without access to this device's filesystem. Files and unsupported or
/// oversized icons are left untouched (the peer shows a placeholder).
fn embed_app_icon(result: &mut SearchResult) {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    const MAX_ICON_BYTES: u64 = 256 * 1024;

    if !matches!(result.result_type, ResultType::App) {
        return;
    }
    let Some(path) = result.icon_path.clone() else {
        return;
    };
    let lower = path.to_lowercase();
    let mime = if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".svg") {
        "image/svg+xml"
    } else {
        result.icon_path = None;
        return;
    };
    match std::fs::metadata(&path) {
        Ok(meta) if meta.len() <= MAX_ICON_BYTES => match std::fs::read(&path) {
            Ok(bytes) => {
                result.icon_path = Some(format!("data:{};base64,{}", mime, BASE64.encode(&bytes)));
            }
            Err(_) => result.icon_path = None,
        },
        _ => result.icon_path = None,
    }
}

/// Resolve a remote launch request to the locally-indexed exec string. The
/// requester's source_path is only a lookup key; an unknown path is rejected
/// and the path itself is never executed.
async fn resolve_launch_exec(db: &Db, source_path: &str) -> Result<String, String> {
    db.app_exec_by_source_path(source_path)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "App not found in local index".to_string())
}

/// Validate and run a remote launch request, spawning only the exec that was
/// indexed locally for the requested application.
async fn handle_launch_request(db: &Db, source_path: &str) -> Result<(), String> {
    let exec = resolve_launch_exec(db, source_path).await?;
    crate::commands::launch_app(exec).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::AppRow;

    async fn db_with_app(source_path: &str, exec: &str) -> Db {
        let db = Db::open_in_memory().await.expect("in-memory db");
        db.upsert_app(&AppRow {
            id: 0,
            name: "Test App".to_string(),
            exec: exec.to_string(),
            icon_path: None,
            platform: "linux".to_string(),
            source_path: source_path.to_string(),
        })
        .await
        .expect("insert app");
        db
    }

    #[tokio::test]
    async fn launch_request_rejects_unindexed_source_path() {
        let db = db_with_app("/usr/share/applications/known.desktop", "known-bin").await;
        let err = resolve_launch_exec(&db, "/tmp/evil.desktop").await.unwrap_err();
        assert_eq!(err, "App not found in local index");
    }

    #[tokio::test]
    async fn launch_request_uses_locally_stored_exec_not_request_path() {
        // The request path is the lookup key; the returned exec must be the one
        // stored at index time, never the path supplied by the requester.
        let db = db_with_app("/usr/share/applications/known.desktop", "known-bin").await;
        let exec = resolve_launch_exec(&db, "/usr/share/applications/known.desktop")
            .await
            .unwrap();
        assert_eq!(exec, "known-bin");
    }
}
