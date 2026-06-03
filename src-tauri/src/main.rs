#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod network;
mod trust;
mod storage;
mod platform;
mod search;
mod share;
mod notify;

use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::OnceLock;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::Mutex;
use tokio::sync::RwLock;
use crate::network::PeerMap;
use crate::network::TransferQueue;
use crate::trust::LocalIdentity;
use crate::storage::Db;
use crate::search::index::FileIndex;
use crate::search::engine::SearchEngine;

/// All live runtime state shared across Tauri commands.
pub struct AppState {
    pub db:           Arc<Db>,
    /// Local identity, behind a lock so the device name can be changed at runtime.
    pub identity:     Arc<RwLock<LocalIdentity>>,
    pub peer_map:     PeerMap,
    pub trusted_ids:  Arc<std::sync::Mutex<HashSet<String>>>,
    /// Device name shared with the discovery thread so renames take effect live.
    pub discovery_name: Arc<std::sync::Mutex<String>>,
    /// Set to force the discovery thread to emit a presence packet immediately.
    pub rebroadcast:  Arc<AtomicBool>,
    /// Channel sender for accepting/rejecting incoming pair requests.
    pub pair_tx:      Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    /// Pending outbound pairing (initiator side, waiting for user confirmation).
    pub pending_pair: Arc<Mutex<Option<crate::network::peer::PendingPairing>>>,
    /// Full-text file index.
    pub file_index:   Arc<FileIndex>,
    /// Local search engine (Bloom -> Trie -> tantivy -> fuzzy with LRU cache).
    pub search_engine: Arc<SearchEngine>,
    /// True once the initial scan and full-text index rebuild have completed.
    pub index_ready: Arc<AtomicBool>,
    /// In-memory view of active, queued, and recently completed transfers.
    pub transfer_queue: Arc<TransferQueue>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db = Arc::new(Db::open().await?);
    let identity = Arc::new(trust::init_identity(&db).await?);
    tracing::info!("identity: {} / {}", identity.device_id, identity.device_name);

    let trusted_rows = db.get_trusted_peers().await?;
    let trusted_ids = Arc::new(std::sync::Mutex::new(
        trusted_rows.iter().map(|r| r.device_id.clone()).collect::<HashSet<_>>(),
    ));

    let discovery_name = Arc::new(std::sync::Mutex::new(identity.device_name.clone()));
    let rebroadcast = Arc::new(AtomicBool::new(false));
    // The AppHandle does not exist until setup() runs, so discovery receives a
    // deferred handle it reads once available (to notify on trusted peers coming online).
    let discovery_app_handle: Arc<OnceLock<AppHandle>> = Arc::new(OnceLock::new());
    let peer_map = network::start_discovery(
        identity.device_id,
        Arc::clone(&discovery_name),
        Arc::clone(&trusted_ids),
        Arc::clone(&rebroadcast),
        Arc::clone(&db),
        Arc::clone(&discovery_app_handle),
    )?;

    let pair_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>> = Arc::new(Mutex::new(None));
    let pending_pair: Arc<Mutex<Option<network::peer::PendingPairing>>> = Arc::new(Mutex::new(None));

    let mut index_path = dirs::data_dir().ok_or("no data dir")?;
    index_path.push("synapt");
    index_path.push("tantivy_index");
    let file_index = Arc::new(FileIndex::open(index_path)?);
    let search_engine =
        Arc::new(SearchEngine::init(Arc::clone(&db), Arc::clone(&file_index)).await?);

    let transfer_queue = Arc::new(TransferQueue::new(100));
    let index_ready = Arc::new(AtomicBool::new(false));

    let hotkey = db
        .get_setting("hotkey")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "ctrl+space".to_string());

    // The background network servers hold an immutable Arc<LocalIdentity>; the
    // commands layer holds a separate lockable copy whose device name can change.
    let identity_lock = Arc::new(RwLock::new((*identity).clone()));

    let state = AppState {
        db:            Arc::clone(&db),
        identity:      Arc::clone(&identity_lock),
        peer_map,
        trusted_ids:   Arc::clone(&trusted_ids),
        discovery_name: Arc::clone(&discovery_name),
        rebroadcast:   Arc::clone(&rebroadcast),
        pair_tx:       Arc::clone(&pair_tx),
        pending_pair:  Arc::clone(&pending_pair),
        file_index:    Arc::clone(&file_index),
        search_engine: Arc::clone(&search_engine),
        index_ready:   Arc::clone(&index_ready),
        transfer_queue: Arc::clone(&transfer_queue),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(state)
        .setup(move |app| {
            platform::setup_current();

            // Hand the discovery thread a live AppHandle for presence notifications.
            let _ = discovery_app_handle.set(app.handle().clone());

            // macOS: hide the Dock icon (tray-only app).
            #[cfg(target_os = "macos")]
            platform::macos::setup(app);

            // Shared system tray: become the host (owns the single icon) or attach
            // as a client to an already-running Synapt/SynaptClip host.
            share::start(app);

            // Global hotkey to toggle the overlay.
            // Linux X11 uses XGrabKey; Wayland uses the inhibitor protocol and may
            // require the user to grant permission. macOS uses CGEventTap and will
            // prompt for Accessibility permission on first registration. Windows uses
            // RegisterHotKey with no extra setup.
            {
                use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
                if let Err(e) = app.global_shortcut().on_shortcut(
                    hotkey.as_str(),
                    |app, _shortcut, event| {
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
                    },
                ) {
                    tracing::error!("failed to register global shortcut '{}': {}", hotkey, e);
                }
            }

            // Pairing responder server.
            let handle = app.handle().clone();
            let db2 = Arc::clone(&db);
            let id2 = Arc::clone(&identity);
            let tx2 = Arc::clone(&pair_tx);
            tokio::spawn(async move {
                if let Err(e) = network::peer::start_pairing_server(id2, db2, handle, tx2).await {
                    tracing::error!("pairing server error: {}", e);
                }
            });

            // Encrypted file transfer server.
            let handle2 = app.handle().clone();
            let db3 = Arc::clone(&db);
            let id3 = Arc::clone(&identity);
            let tq3 = Arc::clone(&transfer_queue);
            tokio::spawn(async move {
                if let Err(e) = network::transfer::start_transfer_server(id3, db3, handle2, tq3).await {
                    tracing::error!("transfer server error: {}", e);
                }
            });

            // Encrypted remote search server.
            let db_s = Arc::clone(&db);
            let id_s = Arc::clone(&identity);
            let se_s = Arc::clone(&search_engine);
            tokio::spawn(async move {
                if let Err(e) = network::start_search_server(id_s, db_s, se_s).await {
                    tracing::error!("search server error: {}", e);
                }
            });

            // Installed-application scan, independent of the file index.
            let db_for_apps = Arc::clone(&db);
            tokio::spawn(async move {
                if let Err(e) = search::app_indexer::run_app_scan(&db_for_apps).await {
                    tracing::warn!("app scan failed: {}", e);
                }
            });

            // Initial file system scan and full-text index build.
            let db4 = Arc::clone(&db);
            let fi = Arc::clone(&file_index);
            let se = Arc::clone(&search_engine);
            let ready = Arc::clone(&index_ready);
            let handle3 = app.handle().clone();
            tokio::spawn(async move {
                let dir_count = db4.get_indexed_dirs().await.map(|d| d.len()).unwrap_or(0);
                tracing::info!("indexed_dirs: {} directories configured", dir_count);
                if dir_count == 0 {
                    // No directories to scan: tell the frontend so it can prompt
                    // the user to add some in Settings.
                    let _ = handle3.emit("no-indexed-dirs", ());
                    ready.store(true, std::sync::atomic::Ordering::Relaxed);
                    return;
                }

                let include_hidden = db4
                    .get_setting("include_hidden")
                    .await
                    .ok()
                    .flatten()
                    .map(|v| v == "true")
                    .unwrap_or(false);
                match search::indexer::run_full_scan(&db4, include_hidden).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!("index scan error: {}", e);
                        return;
                    }
                }
                if let Err(e) = search::indexer::prune_deleted(&db4).await {
                    tracing::error!("index prune error: {}", e);
                }
                match fi.rebuild_from_db(&db4).await {
                    Ok(n) => tracing::info!("tantivy: index rebuilt with {} documents", n),
                    Err(e) => tracing::error!("full-text index rebuild error: {}", e),
                }
                if let Err(e) = se.rebuild().await {
                    tracing::error!("search engine rebuild error: {}", e);
                }
                ready.store(true, std::sync::atomic::Ordering::Relaxed);
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_local_device,
            commands::get_peers,
            commands::get_trusted_peers,
            commands::begin_pairing_cmd,
            commands::confirm_pairing_cmd,
            commands::accept_pair_cmd,
            commands::reject_pair_cmd,
            commands::revoke_peer_cmd,
            commands::get_shared_dirs,
            commands::add_shared_dir,
            commands::remove_shared_dir,
            commands::get_setting,
            commands::set_setting,
            commands::get_all_settings,
            commands::get_indexed_dir_stats,
            commands::set_device_name,
            commands::get_local_identity,
            commands::open_dir_picker,
            commands::set_autostart,
            commands::get_autostart,
            commands::set_hotkey,
            commands::request_file_cmd,
            commands::request_files_cmd,
            commands::send_files_cmd,
            commands::get_transfer_queue,
            commands::get_transfer_history,
            commands::trigger_reindex,
            commands::get_indexed_dirs,
            commands::add_indexed_dir,
            commands::remove_indexed_dir,
            commands::get_index_status,
            commands::search_local,
            commands::search_remote,
            commands::evaluate_expr,
            commands::open_file_path,
            commands::launch_app,
            commands::trigger_app_scan,
            commands::hide_window,
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}
