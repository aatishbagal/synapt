use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use uuid::Uuid;
use synapt_core::{Peer, PeerStatus};
use thiserror::Error;

use crate::storage::Db;

pub const MULTICAST_GROUP: Ipv4Addr = Ipv4Addr::new(239, 255, 42, 99);
pub const MULTICAST_PORT:  u16 = 42099;
pub const PAIRING_PORT:    u16 = 42100;
/// Reserved for the v0.x file-search feature; not yet used.
#[allow(dead_code)]
pub const SEARCH_PORT:     u16 = 42101;
pub const TRANSFER_PORT:   u16 = 42102;

const BROADCAST_INTERVAL: Duration = Duration::from_secs(5);
const PEER_TIMEOUT:        Duration = Duration::from_secs(20);

/// Errors raised by peer discovery.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("socket error: {0}")]
    Socket(#[from] std::io::Error),
    #[error("serialise error: {0}")]
    Serialise(#[from] serde_json::Error),
}

/// Presence packet broadcast over UDP multicast.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresencePacket {
    pub r#type:       String,
    pub device_id:    String,
    pub device_name:  String,
    pub version:      String,
    pub pairing_port: u16,
}

/// Entry in the in-memory peer map.
#[derive(Debug, Clone)]
pub struct PeerEntry {
    pub peer:      Peer,
    pub last_seen: Instant,
}

/// Shared in-memory peer map - keyed by device_id string.
pub type PeerMap = Arc<Mutex<HashMap<String, PeerEntry>>>;

/// Lock a mutex, recovering the guard if a holder thread panicked.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Select the best local IPv4 interface for multicast.
/// Falls back to UNSPECIFIED if none found.
pub fn select_interface() -> Ipv4Addr {
    use local_ip_address::local_ip;
    match local_ip() {
        Ok(IpAddr::V4(ip)) => ip,
        _ => Ipv4Addr::UNSPECIFIED,
    }
}

/// Start the discovery background thread.
/// Returns the shared PeerMap for reading from Tauri commands.
///
/// `device_name` is shared so a rename in Settings is reflected in the next
/// broadcast; setting `rebroadcast` forces an immediate presence packet.
pub fn start(
    local_device_id: Uuid,
    device_name: Arc<Mutex<String>>,
    trusted_ids: Arc<Mutex<HashSet<String>>>,
    rebroadcast: Arc<AtomicBool>,
    db: Arc<Db>,
    notify_handle: Arc<OnceLock<AppHandle>>,
) -> Result<PeerMap, DiscoveryError> {
    let peer_map: PeerMap = Arc::new(Mutex::new(HashMap::new()));
    let pm = Arc::clone(&peer_map);
    let interface_ip = select_interface();

    std::thread::spawn(move || {
        if let Err(e) = run_loop(
            pm, interface_ip, local_device_id, device_name, trusted_ids, rebroadcast, db, notify_handle,
        ) {
            tracing::error!("discovery loop terminated: {}", e);
        }
    });

    Ok(peer_map)
}

#[allow(clippy::too_many_arguments)]
fn run_loop(
    pm: PeerMap,
    interface_ip: Ipv4Addr,
    local_device_id: Uuid,
    device_name: Arc<Mutex<String>>,
    trusted_ids: Arc<Mutex<HashSet<String>>>,
    rebroadcast: Arc<AtomicBool>,
    db: Arc<Db>,
    notify_handle: Arc<OnceLock<AppHandle>>,
) -> Result<(), DiscoveryError> {
    let socket = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), MULTICAST_PORT))?;
    socket.set_read_timeout(Some(Duration::from_millis(500)))?;
    socket.join_multicast_v4(&MULTICAST_GROUP, &interface_ip)?;
    socket.set_multicast_loop_v4(false)?;

    let dest = SocketAddr::new(IpAddr::V4(MULTICAST_GROUP), MULTICAST_PORT);
    let mut last_tx = Instant::now() - BROADCAST_INTERVAL;
    // Trusted peers seen as online on the previous iteration, so we can notify
    // exactly once each time one transitions from absent to present.
    let mut last_online_trusted: HashSet<String> = HashSet::new();

    loop {
        let forced = rebroadcast.swap(false, Ordering::Relaxed);
        if forced || last_tx.elapsed() >= BROADCAST_INTERVAL {
            let pkt = PresencePacket {
                r#type:       "presence".into(),
                device_id:    local_device_id.to_string(),
                device_name:  lock(&device_name).clone(),
                version:      env!("CARGO_PKG_VERSION").into(),
                pairing_port: PAIRING_PORT,
            };
            if let Ok(json) = serde_json::to_string(&pkt) {
                let _ = socket.send_to(json.as_bytes(), dest);
            }
            last_tx = Instant::now();
        }

        let mut buf = [0u8; 1024];
        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                if let Ok(pkt) = serde_json::from_slice::<PresencePacket>(&buf[..len]) {
                    if pkt.device_id == local_device_id.to_string() {
                        continue;
                    }
                    if pkt.r#type != "presence" {
                        continue;
                    }

                    let is_trusted = lock(&trusted_ids).contains(&pkt.device_id);
                    let status = if is_trusted { PeerStatus::Trusted } else { PeerStatus::Discovered };

                    lock(&pm).insert(pkt.device_id.clone(), PeerEntry {
                        peer: Peer {
                            device_id:    pkt.device_id.parse().unwrap_or_default(),
                            device_name:  pkt.device_name,
                            ip:           src.ip(),
                            pairing_port: pkt.pairing_port,
                            status,
                        },
                        last_seen: Instant::now(),
                    });
                }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock
                       || e.kind() == std::io::ErrorKind::TimedOut => {}
            Err(e) => tracing::warn!("discovery recv: {}", e),
        }

        lock(&pm).retain(|_, e| e.last_seen.elapsed() < PEER_TIMEOUT);

        // Detect trusted peers that just appeared and fire a presence notification.
        let newly_online: Vec<(String, String)> = {
            let map = lock(&pm);
            let online_trusted: HashSet<String> = map
                .iter()
                .filter(|(_, e)| e.peer.status == PeerStatus::Trusted)
                .map(|(id, _)| id.clone())
                .collect();
            let appeared = online_trusted
                .iter()
                .filter(|id| !last_online_trusted.contains(*id))
                .filter_map(|id| map.get(id).map(|e| (id.clone(), e.peer.device_name.clone())))
                .collect();
            last_online_trusted = online_trusted;
            appeared
        };
        for (_, name) in newly_online {
            if let Some(app) = notify_handle.get() {
                let app = app.clone();
                let db = Arc::clone(&db);
                tauri::async_runtime::spawn(async move {
                    if crate::notify::enabled(&db).await {
                        crate::notify::peer_online(&app, &name);
                    }
                });
            }
        }
    }
}

/// Snapshot the peer list as a Vec.
pub fn list_peers(peer_map: &PeerMap) -> Vec<Peer> {
    lock(peer_map).values().map(|e| e.peer.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presence_packet_roundtrip() {
        let pkt = PresencePacket {
            r#type: "presence".into(), device_id: "abc".into(),
            device_name: "test".into(), version: "0.1.0".into(), pairing_port: 42100,
        };
        let json = serde_json::to_string(&pkt).unwrap();
        let pkt2: PresencePacket = serde_json::from_str(&json).unwrap();
        assert_eq!(pkt2.device_id, "abc");
    }
    #[test]
    fn list_peers_empty_map() {
        let map: PeerMap = Arc::new(Mutex::new(std::collections::HashMap::new()));
        assert!(list_peers(&map).is_empty());
    }
    #[test]
    fn select_interface_does_not_panic() { let _ = select_interface(); }
}
