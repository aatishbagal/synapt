#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod network;
mod trust;
mod storage;
mod platform;

use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::network::PeerMap;
use crate::trust::LocalIdentity;
use crate::storage::Db;

/// All live runtime state shared across Tauri commands.
pub struct AppState {
    pub db:           Arc<Db>,
    pub identity:     Arc<LocalIdentity>,
    pub peer_map:     PeerMap,
    pub trusted_ids:  Arc<std::sync::Mutex<HashSet<String>>>,
    /// Channel sender for accepting/rejecting incoming pair requests.
    pub pair_tx:      Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>>,
    /// Pending outbound pairing (initiator side, waiting for user confirmation).
    pub pending_pair: Arc<Mutex<Option<crate::network::peer::PendingPairing>>>,
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

    let peer_map = network::start_discovery(
        identity.device_id,
        identity.device_name.clone(),
        Arc::clone(&trusted_ids),
    )?;

    let pair_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<bool>>>> = Arc::new(Mutex::new(None));
    let pending_pair: Arc<Mutex<Option<network::peer::PendingPairing>>> = Arc::new(Mutex::new(None));

    let state = AppState {
        db:           Arc::clone(&db),
        identity:     Arc::clone(&identity),
        peer_map,
        trusted_ids:  Arc::clone(&trusted_ids),
        pair_tx:      Arc::clone(&pair_tx),
        pending_pair: Arc::clone(&pending_pair),
    };

    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            platform::setup_current();

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
            tokio::spawn(async move {
                if let Err(e) = network::transfer::start_transfer_server(id3, db3, handle2).await {
                    tracing::error!("transfer server error: {}", e);
                }
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
            commands::request_file_cmd,
            commands::get_transfer_history,
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}
