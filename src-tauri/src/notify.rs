//! Best-effort system notifications for transfer, pairing, and presence events.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::storage::Db;

/// Whether notifications are enabled. Defaults to true when the setting is unset.
pub async fn enabled(db: &Db) -> bool {
    db.get_setting("notifications_enabled")
        .await
        .ok()
        .flatten()
        .as_deref()
        != Some("false")
}

/// Send a system notification. Silently ignores errors — notifications are best-effort.
pub fn send(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

/// Notify that a file transfer completed successfully.
pub fn transfer_complete(app: &AppHandle, filename: &str, peer_name: &str) {
    send(app, "Transfer complete", &format!("{filename} received from {peer_name}"));
}

/// Notify that a new device was paired.
pub fn peer_paired(app: &AppHandle, peer_name: &str) {
    send(app, "Device paired", &format!("{peer_name} is now a trusted device"));
}

/// Notify that a trusted peer came online.
pub fn peer_online(app: &AppHandle, peer_name: &str) {
    send(app, "Device online", &format!("{peer_name} is now available"));
}
