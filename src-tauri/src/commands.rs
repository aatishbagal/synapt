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
    let identity = state.identity.read().await;
    let fingerprint =
        crate::trust::fingerprint(&identity.pubkey_b64).map_err(|e| e.to_string())?;
    Ok(LocalDeviceInfo {
        device_id: identity.device_id.to_string(),
        device_name: identity.device_name.clone(),
        pubkey_b64: identity.pubkey_b64.clone(),
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

/// Resolve a discovered peer's address and pairing port by device id.
fn resolve_pairing_target(
    peer_map: &crate::network::PeerMap,
    device_id: &str,
) -> Result<(std::net::IpAddr, u16), String> {
    let map = lock(peer_map);
    let entry = map.get(device_id).ok_or_else(|| "peer not found".to_string())?;
    Ok((entry.peer.ip, entry.peer.pairing_port))
}

/// Begin pairing with a discovered peer; returns the verification code to display.
#[tauri::command]
pub async fn begin_pairing_cmd(
    device_id: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<String, String> {
    let (ip, port) = resolve_pairing_target(&state.peer_map, &device_id)?;
    let pending = {
        let identity = state.identity.read().await;
        crate::network::peer::begin_pairing(ip, port, &identity)
            .await
            .map_err(|e| e.to_string())?
    };
    let code = pending.verify_code.clone();
    *state.pending_pair.lock().await = Some(pending);
    Ok(code)
}

/// Confirm a pending outbound pairing after the user verified the code.
#[tauri::command]
pub async fn confirm_pairing_cmd(
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<synapt_core::TrustedPeer, String> {
    let pending = state
        .pending_pair
        .lock()
        .await
        .take()
        .ok_or_else(|| "no pending pairing".to_string())?;
    let peer = {
        let identity = state.identity.read().await;
        crate::network::peer::confirm_pairing(pending, &identity, &state.db)
            .await
            .map_err(|e| e.to_string())?
    };
    lock(&state.trusted_ids).insert(peer.device_id.to_string());
    if crate::notify::enabled(&state.db).await {
        crate::notify::peer_paired(&app, &peer.device_name);
    }
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

/// Get every setting as a key/value map (loaded once by the Settings page).
#[tauri::command]
pub async fn get_all_settings(
    state: tauri::State<'_, crate::AppState>,
) -> Result<std::collections::HashMap<String, String>, String> {
    state.db.get_all_settings().await.map_err(|e| e.to_string())
}

/// List indexed directories with their file counts and last-scan times.
#[tauri::command]
pub async fn get_indexed_dir_stats(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::storage::IndexedDirRow>, String> {
    state.db.get_indexed_dir_stats().await.map_err(|e| e.to_string())
}

/// Local identity fields exposed to the Settings page.
#[derive(serde::Serialize)]
pub struct LocalIdentityInfo {
    pub device_id:   String,
    pub device_name: String,
    pub fingerprint: String,
}

/// Get the local device id, name, and key fingerprint.
#[tauri::command]
pub async fn get_local_identity(
    state: tauri::State<'_, crate::AppState>,
) -> Result<LocalIdentityInfo, String> {
    let identity = state.identity.read().await;
    let fingerprint =
        crate::trust::fingerprint(&identity.pubkey_b64).map_err(|e| e.to_string())?;
    Ok(LocalIdentityInfo {
        device_id:   identity.device_id.to_string(),
        device_name: identity.device_name.clone(),
        fingerprint,
    })
}

/// Rename the local device, persist it, and re-announce presence immediately.
#[tauri::command]
pub async fn set_device_name(
    name: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > 64 {
        return Err("device name must be between 1 and 64 characters".to_string());
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err("device name must not contain control characters".to_string());
    }

    state.db.update_local_device_name(trimmed).await.map_err(|e| e.to_string())?;
    {
        let mut identity = state.identity.write().await;
        identity.device_name = trimmed.to_string();
    }
    *lock(&state.discovery_name) = trimmed.to_string();
    state.rebroadcast.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(())
}

/// Enable or disable launching Synapt automatically on login.
#[tauri::command]
pub async fn set_autostart(enabled: bool, _app: tauri::AppHandle) -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    if enabled {
        crate::platform::autostart::enable(&exe_path).map_err(|e| e.to_string())
    } else {
        crate::platform::autostart::disable().map_err(|e| e.to_string())
    }
}

/// Report whether Synapt is configured to launch on login.
#[tauri::command]
pub async fn get_autostart() -> Result<bool, String> {
    crate::platform::autostart::is_enabled().map_err(|e| e.to_string())
}

/// Probe the local IPC server's health endpoint; true when it responds 200.
#[tauri::command]
pub async fn get_ipc_status() -> Result<bool, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .map_err(|e| e.to_string())?;
    match client.get("http://127.0.0.1:57321/v1/health").send().await {
        Ok(resp) => Ok(resp.status().is_success()),
        Err(_) => Ok(false),
    }
}

/// Show a native folder picker. Returns the chosen path, or None if cancelled.
#[tauri::command]
pub async fn open_dir_picker(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    // Keep the overlay from auto-hiding while the picker (which steals focus) is open.
    state.suppress_hide.store(true, std::sync::atomic::Ordering::Relaxed);
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().pick_folder(move |picked| {
        let _ = tx.send(picked);
    });
    let result = rx.await.map_err(|e| e.to_string());
    state.suppress_hide.store(false, std::sync::atomic::Ordering::Relaxed);
    let picked = result?;
    Ok(picked.and_then(|p| p.into_path().ok()).map(|p| p.to_string_lossy().to_string()))
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

    let identity = state.identity.read().await;
    let path = crate::network::transfer::request_transfer_with_retry(
        &device_id,
        ip,
        remote_path,
        &identity,
        &state.db,
        &download_dir,
        &peer_name,
        &app,
        &state.transfer_queue,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(path.to_string_lossy().to_string())
}

/// Request multiple files from a trusted peer; returns the local download paths.
#[tauri::command]
pub async fn request_files_cmd(
    device_id: String,
    remote_paths: Vec<String>,
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<Vec<String>, String> {
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

    let identity = state.identity.read().await;
    let paths = crate::network::transfer::request_batch_transfer_with_retry(
        &device_id,
        ip,
        remote_paths,
        &identity,
        &state.db,
        &download_dir,
        &peer_name,
        &app,
        &state.transfer_queue,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(paths.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

/// Return the in-memory transfer queue, most recent first.
#[tauri::command]
pub async fn get_transfer_queue(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::network::QueueEntry>, String> {
    Ok(state.transfer_queue.list())
}

/// Re-register the global hotkey and persist it to settings.
#[tauri::command]
pub async fn set_hotkey(
    hotkey: String,
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri::Manager;
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

    app.global_shortcut().unregister_all().map_err(|e| e.to_string())?;
    app.global_shortcut()
        .on_shortcut(hotkey.as_str(), |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .map_err(|e| e.to_string())?;

    state.db.set_setting("hotkey", &hotkey).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Send (push) one or more local files to a trusted peer.
#[tauri::command]
pub async fn send_files_cmd(
    device_id: String,
    local_paths: Vec<String>,
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    for path in &local_paths {
        let meta = std::fs::metadata(path).map_err(|e| format!("{path}: {e}"))?;
        if !meta.is_file() {
            return Err(format!("{path} is not a file"));
        }
    }

    let peers = crate::trust::list_trusted_peers(&state.db).await.map_err(|e| e.to_string())?;
    let peer = peers
        .iter()
        .find(|p| p.device_id.to_string() == device_id)
        .ok_or_else(|| "peer not trusted".to_string())?;
    let peer_name = peer.device_name.clone();

    let ip = {
        let map = lock(&state.peer_map);
        map.get(&device_id)
            .map(|e| e.peer.ip)
            .ok_or_else(|| "peer not online".to_string())?
    };

    let identity = state.identity.read().await;
    crate::network::transfer::push_files(
        &device_id,
        ip,
        local_paths,
        &identity,
        &state.db,
        &peer_name,
        &app,
        &state.transfer_queue,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
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
pub async fn trigger_reindex(
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<u64, String> {
    let include_hidden = state
        .db
        .get_setting("include_hidden")
        .await
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(false);
    // run_full_scan emits Failed and clears the flag itself on scan error.
    let total = crate::search::indexer::run_full_scan(&state.db, include_hidden, &app, &state.is_indexing)
        .await
        .map_err(|e| e.to_string())?;
    let rebuilt = async {
        crate::search::indexer::prune_deleted(&state.db).await.map_err(|e| e.to_string())?;
        state.file_index.rebuild_from_db(&state.db).await.map_err(|e| e.to_string())?;
        state.search_engine.rebuild().await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    }
    .await;
    if let Err(e) = rebuilt {
        crate::search::indexer::finish_err(&app, &state.is_indexing, e.clone());
        return Err(e);
    }
    tracing::info!("tantivy: index rebuilt after reindex");
    state.index_ready.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::search::indexer::finish_ok(&app, &state.is_indexing, total);
    Ok(total)
}

/// List the directories currently configured for the search index.
#[tauri::command]
pub async fn get_indexed_dirs(
    state: tauri::State<'_, crate::AppState>,
) -> Result<Vec<crate::storage::IndexedDirRow>, String> {
    state.db.get_indexed_dirs().await.map_err(|e| e.to_string())
}

/// Add a directory to the search index and rescan it in the background.
#[tauri::command]
pub async fn add_indexed_dir(
    path: String,
    state: tauri::State<'_, crate::AppState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let p = std::path::Path::new(&path);
    if !p.is_dir() {
        return Err(format!("not a directory: {}", path));
    }
    state.db.add_indexed_dir(&path).await.map_err(|e| e.to_string())?;
    // Rescan in the foreground so the caller can refresh results immediately.
    let include_hidden = state
        .db
        .get_setting("include_hidden")
        .await
        .map_err(|e| e.to_string())?
        .map(|v| v == "true")
        .unwrap_or(false);
    let total = crate::search::indexer::run_full_scan(&state.db, include_hidden, &app, &state.is_indexing)
        .await
        .map_err(|e| e.to_string())?;
    let rebuilt = async {
        state.file_index.rebuild_from_db(&state.db).await.map_err(|e| e.to_string())?;
        state.search_engine.rebuild().await.map_err(|e| e.to_string())?;
        Ok::<(), String>(())
    }
    .await;
    if let Err(e) = rebuilt {
        crate::search::indexer::finish_err(&app, &state.is_indexing, e.clone());
        return Err(e);
    }
    state.index_ready.store(true, std::sync::atomic::Ordering::Relaxed);
    crate::search::indexer::finish_ok(&app, &state.is_indexing, total);
    Ok(())
}

/// Remove a directory from the search index. Indexed files are pruned on next scan.
#[tauri::command]
pub async fn remove_indexed_dir(
    path: String,
    state: tauri::State<'_, crate::AppState>,
) -> Result<(), String> {
    state.db.remove_indexed_dir(&path).await.map_err(|e| e.to_string())?;
    Ok(())
}

/// Current state of the file index, for surfacing in Settings and the overlay.
#[derive(serde::Serialize)]
pub struct IndexStatus {
    pub indexed_dirs_count: usize,
    pub file_count: i64,
    pub tantivy_ready: bool,
    pub last_scan: Option<i64>,
}

/// Report the current file index status.
#[tauri::command]
pub async fn get_index_status(
    state: tauri::State<'_, crate::AppState>,
) -> Result<IndexStatus, String> {
    let dirs = state.db.get_indexed_dirs().await.map_err(|e| e.to_string())?;
    let file_count = state.db.count_files().await.map_err(|e| e.to_string())?;
    let last_scan = state.db.get_last_scan().await.map_err(|e| e.to_string())?;
    Ok(IndexStatus {
        indexed_dirs_count: dirs.len(),
        file_count,
        tantivy_ready: state.index_ready.load(std::sync::atomic::Ordering::Relaxed),
        last_scan,
    })
}

/// Report whether a file-system scan / index rebuild is currently running.
#[tauri::command]
pub async fn get_is_indexing(state: tauri::State<'_, crate::AppState>) -> Result<bool, String> {
    Ok(state.is_indexing.load(std::sync::atomic::Ordering::Relaxed))
}

/// Evaluate an inline arithmetic expression.
#[tauri::command]
pub fn evaluate_expr(input: String) -> Result<f64, String> {
    crate::search::calc::evaluate(&input).map_err(|e| e.to_string())
}

/// Open a file or folder path with the platform's default handler.
#[tauri::command]
pub async fn open_file_path(path: String) -> Result<(), String> {
    use std::process::Command;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "windows")]
    Command::new("explorer").arg(&path).spawn().map_err(|e| e.to_string())?;
    #[cfg(target_os = "macos")]
    Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

/// Launch an installed application by its platform-specific exec string/path.
#[tauri::command]
pub async fn launch_app(exec: String) -> Result<(), String> {
    use std::process::Command;
    #[cfg(target_os = "linux")]
    {
        // `exec` has already had its .desktop field codes stripped at index time.
        let mut parts = exec.split_whitespace();
        let program = parts.next().ok_or_else(|| "empty exec".to_string())?;
        let args: Vec<&str> = parts.collect();
        Command::new(program).args(args).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // Use the shell to resolve .lnk shortcuts and launch executables alike.
        Command::new("cmd")
            .args(["/c", "start", "", &exec])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(&exec).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Read an application icon file and return it as a data URI the webview can
/// render. Supports PNG and SVG; rejects files larger than 512 KB.
#[tauri::command]
pub async fn get_app_icon(icon_path: String) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
    const MAX_ICON_BYTES: u64 = 512 * 1024;

    let lower = icon_path.to_lowercase();
    let mime = if lower.ends_with(".svg") {
        "image/svg+xml"
    } else if lower.ends_with(".png") {
        "image/png"
    } else {
        return Err("unsupported icon format".to_string());
    };

    let meta = std::fs::metadata(&icon_path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_ICON_BYTES {
        return Err("icon file too large".to_string());
    }
    let bytes = std::fs::read(&icon_path).map_err(|e| e.to_string())?;
    Ok(format!("data:{};base64,{}", mime, BASE64.encode(&bytes)))
}

/// Scan installed applications and repopulate the applications index.
#[tauri::command]
pub async fn trigger_app_scan(state: tauri::State<'_, crate::AppState>) -> Result<usize, String> {
    crate::search::app_indexer::run_app_scan(&state.db).await.map_err(|e| e.to_string())
}

/// Hide the main overlay window.
#[tauri::command]
pub async fn hide_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    app.get_webview_window("main")
        .ok_or_else(|| "no window".to_string())?
        .hide()
        .map_err(|e| e.to_string())
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
    use crate::search::engine::{ResultSource, ResultType, SearchResult};

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
    let identity = state.identity.read().await;
    let local_id = identity.device_id.to_string();
    let key = crate::network::session_handshake_client(&mut stream, &local_id, &identity, &peer_pubkey)
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
            exec: None,
            result_type: ResultType::File,
            source: ResultSource::Remote { device_name: device_name.clone() },
            score: r.score,
            icon_path: None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn resolve_pairing_target_errors_when_peer_not_found() {
        // A missing peer must yield a meaningful error, not a panic or serde error.
        let peer_map: crate::network::PeerMap = Arc::new(Mutex::new(HashMap::new()));
        let err = resolve_pairing_target(&peer_map, "missing-device").unwrap_err();
        assert_eq!(err, "peer not found");
    }
}
