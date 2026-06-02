use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

use crate::network::discovery::SEARCH_PORT;
use crate::network::transfer::is_path_allowed;
use crate::network::{read_enc, session_handshake_server, write_enc, SessionError};
use crate::search::engine::{SearchEngine, SearchResult};
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
    let key = session_handshake_server(&mut stream, &identity, &db).await?;

    let (query, max_results) = match serde_json::from_slice::<SearchMsg>(&read_enc(&mut stream, &key).await?)? {
        SearchMsg::SearchRequest { query, max_results } => (query, max_results),
        _ => return Ok(()),
    };

    let results = engine.search(&query, max_results).await?;
    let shared_dirs = db.get_shared_dirs().await?;
    let filtered: Vec<SearchResult> = results
        .into_iter()
        .filter(|r| is_path_allowed(Path::new(&r.path), &shared_dirs))
        .collect();

    let response = SearchMsg::SearchResults { results: filtered };
    write_enc(&mut stream, &key, &serde_json::to_vec(&response)?).await?;
    Ok(())
}
