use std::sync::MutexGuard;

/// Lock a std mutex, recovering the guard if a holder thread panicked.
fn lock<T>(m: &std::sync::Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Local device identity exposed to the UI (no private key material).
#[derive(serde::Serialize)]
pub struct LocalDeviceInfo {
    pub device_id: String,
    pub device_name: String,
    pub pubkey_b64: String,
    pub fingerprint: String,
}

/// Get the local device identity and key fingerprint.
#[tauri::command]
pub async fn get_local_device(
    state: tauri::State<'_, crate::AppState>,
) -> Result<LocalDeviceInfo, String> {
    let fingerprint =
        crate::trust::fingerprint(&state.identity.pubkey_b64).map_err(|e| e.to_string())?;
    Ok(LocalDeviceInfo {
        device_id: state.identity.device_id.to_string(),
        device_name: state.identity.device_name.clone(),
        pubkey_b64: state.identity.pubkey_b64.clone(),
        fingerprint,
    })
}

/// List peers currently visible on the LAN.
#[tauri::command]
pub async fn get_peers(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<synapt_core::Peer>, String> {
    Ok(crate::network::list_peers(&state.peer_map))
}

/// List all paired (trusted) peers.
#[tauri::command]
pub async fn get_trusted_peers(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<synapt_core::TrustedPeer>, String> {
    crate::trust::list_trusted_peers(&state.db).await.map_err(|e| e.to_string())
}

/// Begin pairing with a discovered peer; returns the verification code to display.
#[tauri::command]
pub async fn begin_pairing_cmd(
    device_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<String, String> {
    let (ip, port) = {
        let map = lock(&state.peer_map);
        let entry = map.get(&device_id).ok_or_else(|| "peer not found".to_string())?;
        (entry.peer.ip, entry.peer.pairing_port)
    };
    let pending = crate::network::peer::begin_pairing(ip, port, &state.identity)
        .await
        .map_err(|e| e.to_string())?;
    let code = pending.verify_code.clone();
    *state.pending_pair.lock().await = Some(pending);
    Ok(code)
}

/// Confirm a pending outbound pairing after the user verified the code.
#[tauri::command]
pub async fn confirm_pairing_cmd(
    state: tauri::State<'_, crate::AppState>,
) -> Result<synapt_core::TrustedPeer, String> {
    let pending = state
        .pending_pair
        .lock()
        .await
        .take()
        .ok_or_else(|| "no pending pairing".to_string())?;
    let peer = crate::network::peer::confirm_pairing(pending, &state.identity, &state.db)
        .await
        .map_err(|e| e.to_string())?;
    lock(&state.trusted_ids).insert(peer.device_id.to_string());
    Ok(peer)
}

/// Accept an incoming pair request awaiting a decision.
#[tauri::command]
pub async fn accept_pair_cmd(state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    let tx = state
        .pair_tx
        .lock()
        .await
        .take()
        .ok_or_else(|| "no pending pair request".to_string())?;
    tx.send(true).map_err(|_| "failed to deliver decision".to_string())
}

/// Reject an incoming pair request awaiting a decision.
#[tauri::command]
pub async fn reject_pair_cmd(state: tauri::State<'_, crate::AppState>) -> Result<(), String> {
    let tx = state
        .pair_tx
        .lock()
        .await
        .take()
        .ok_or_else(|| "no pending pair request".to_string())?;
    tx.send(false).map_err(|_| "failed to deliver decision".to_string())
}

/// Revoke a trusted peer by device id.
#[tauri::command]
pub async fn revoke_peer_cmd(
    device_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    crate::trust::revoke_peer(&state.db, &device_id).await.map_err(|e| e.to_string())?;
    lock(&state.trusted_ids).remove(&device_id);
    Ok(())
}

/// Get the configured shared directories.
#[tauri::command]
pub async fn get_shared_dirs(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<String>, String> {
    state.db.get_shared_dirs().await.map_err(|e| e.to_string())
}

/// Add a shared directory after validating it exists.
#[tauri::command]
pub async fn add_shared_dir(
    path: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    if !std::path::Path::new(&path).is_dir() {
        return Err("path is not a directory".to_string());
    }
    state.db.add_shared_dir(&path).await.map_err(|e| e.to_string())
}

/// Remove a shared directory.
#[tauri::command]
pub async fn remove_shared_dir(
    path: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    state.db.remove_shared_dir(&path).await.map_err(|e| e.to_string())
}

/// Get a setting value by key.
#[tauri::command]
pub async fn get_setting(
    key: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Option<String>, String> {
    state.db.get_setting(&key).await.map_err(|e| e.to_string())
}

/// Set a setting value.
#[tauri::command]
pub async fn set_setting(
    key: String,
    value: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    state.db.set_setting(&key, &value).await.map_err(|e| e.to_string())
}

/// Request a single file from a trusted peer; returns the local download path.
#[tauri::command]
pub async fn request_file_cmd(
    device_id: String,
    remote_path: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<String, String> {
    let peers = crate::trust::list_trusted_peers(&state.db).await.map_err(|e| e.to_string())?;
    let peer = peers
        .iter()
        .find(|p| p.device_id.to_string() == device_id)
        .ok_or_else(|| "peer not trusted".to_string())?;
    let peer_name = peer.device_name.clone();

    let download_dir = match state.db.get_setting("download_dir").await.map_err(|e| e.to_string())? {
        Some(d) if !d.is_empty() => std::path::PathBuf::from(d),
        _ => dirs::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Synapt"),
    };

    let ip = {
        let map = lock(&state.peer_map);
        map.get(&device_id)
            .map(|e| e.peer.ip)
            .ok_or_else(|| "peer not online".to_string())?
    };

    let path = crate::network::transfer::request_transfer(
        &device_id,
        ip,
        remote_path,
        0,
        &state.identity,
        &state.db,
        &download_dir,
        &peer_name,
        &app,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

/// Get the transfer history, most recent first.
#[tauri::command]
pub async fn get_transfer_history(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::storage::TransferHistoryRow>, String> {
    state.db.get_transfer_history().await.map_err(|e| e.to_string())
}

/// Rescan the indexed directories, prune deleted files, and rebuild the index.
#[tauri::command]
pub async fn trigger_reindex(state: tauri::State<'_, crate::AppState>) -> Result<u64, String> {
    let include_hidden = state
        .db
        .get_setting("include_hidden")
        .await
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(false);
    let total = crate::search::indexer::run_full_scan(&state.db, include_hidden)
        .await
        .map_err(|e| e.to_string())?;
    crate::search::indexer::prune_deleted(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    state
        .file_index
        .rebuild_from_db(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    state.search_engine.rebuild().await.map_err(|e| e.to_string())?;
    Ok(total)
}

/// Evaluate an inline arithmetic expression.
#[tauri::command]
pub fn evaluate_expr(input: String) -> Result<f64, String> {
    crate::search::calc::evaluate(&input).map_err(|e| e.to_string())
}

/// Run a local search and return ranked results.
#[tauri::command]
pub async fn search_local(
    query: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::search::engine::SearchResult>, String> {
    let max_results = state
        .db
        .get_setting("max_results")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);
    state
        .search_engine
        .search(&query, max_results)
        .await
        .map_err(|e| e.to_string())
}

/// Search a trusted peer's shared files over the encrypted session channel.
#[tauri::command]
pub async fn search_remote(
    device_id: String,
    query: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::search::engine::SearchResult>, String> {
    use crate::network::search_server::SearchMsg;
    use crate::search::engine::{ResultSource, SearchResult};

    let peers = crate::trust::list_trusted_peers(&state.db).await.map_err(|e| e.to_string())?;
    let (device_name, peer_pubkey) = peers
        .iter()
        .find(|p| p.device_id.to_string() == device_id)
        .map(|p| (p.device_name.clone(), p.pubkey_b64.clone()))
        .ok_or_else(|| "peer not trusted".to_string())?;

    let ip = {
        let map = lock(&state.peer_map);
        map.get(&device_id)
            .map(|e| e.peer.ip)
            .ok_or_else(|| "peer not online".to_string())?
    };

    let max_results = state
        .db
        .get_setting("max_results")
        .await
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(50);

    let mut stream = tokio::net::TcpStream::connect((ip, crate::network::SEARCH_PORT))
        .await
        .map_err(|e| e.to_string())?;
    let local_id = state.identity.device_id.to_string();
    let key = crate::network::session_handshake_client(&mut stream, &local_id, &state.identity, &peer_pubkey)
        .await
        .map_err(|e| e.to_string())?;

    let request = SearchMsg::SearchRequest { query, max_results };
    crate::network::write_enc(&mut stream, &key, &serde_json::to_vec(&request).map_err(|e| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?;

    let response: SearchMsg = serde_json::from_slice(
        &crate::network::read_enc(&mut stream, &key).await.map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;

    let results = match response {
        SearchMsg::SearchResults { results } => results,
        _ => return Err("unexpected response from peer".to_string()),
    };

    Ok(results
        .into_iter()
        .map(|r| SearchResult {
            name: r.name,
            path: r.path,
            source: ResultSource::Remote { device_name: device_name.clone() },
            score: r.score,
        })
        .collect())
}
