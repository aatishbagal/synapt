use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

/// Loopback port the IPC server binds. SynaptClip connects here.
const IPC_PORT: u16 = 57321;

/// Shared state handed to IPC route handlers. Cheap to clone (all fields are Arc).
#[derive(Clone)]
pub struct IpcState {
    /// Live discovery peer map, snapshotted per request.
    pub peer_map: crate::network::PeerMap,
    /// Device IDs of currently trusted peers.
    pub trusted_ids: Arc<Mutex<HashSet<String>>>,
    /// Persistent store.
    pub db: Arc<crate::storage::Db>,
    /// Local identity used to authenticate outbound transfers.
    pub identity: Arc<crate::trust::LocalIdentity>,
    /// Shared transfer queue the clip transfer is enqueued on.
    pub transfer_queue: Arc<crate::network::TransferQueue>,
    /// App handle so the spawned transfer can emit progress events.
    pub app: tauri::AppHandle,
}

#[derive(Serialize)]
struct HealthResponse {
    api_version: &'static str,
    synapt_version: &'static str,
    status: &'static str,
}

/// Start the loopback IPC server. Binding failure is logged and tolerated so the
/// rest of Synapt continues to run normally.
pub async fn start(state: IpcState) {
    let app = Router::new()
        .route("/v1/health", get(health_handler))
        .route("/v1/peers", get(peers_handler))
        .route("/v1/clips/send", post(clips_send_handler))
        .layer(axum::middleware::from_fn(require_loopback))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], IPC_PORT));
    tracing::info!("IPC server listening on {}", addr);

    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!(
                "IPC server failed to bind on port {}: {} — SynaptClip integration disabled",
                IPC_PORT,
                e
            );
            return;
        }
    };

    // ConnectInfo is required by the loopback middleware to read the remote IP.
    let service = app.into_make_service_with_connect_info::<SocketAddr>();
    if let Err(e) = axum::serve(listener, service).await {
        tracing::error!("IPC server error: {}", e);
    }
}

/// Decide whether a remote address is permitted: loopback only, otherwise 403.
fn check_loopback(addr: &SocketAddr) -> Result<(), StatusCode> {
    if addr.ip().is_loopback() {
        Ok(())
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// Reject any request that does not originate from a loopback address.
pub async fn require_loopback(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    if let Err(status) = check_loopback(&addr) {
        tracing::warn!("ipc: rejected non-loopback request from {}", addr);
        return Err(status);
    }
    Ok(next.run(req).await)
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        api_version: "1",
        synapt_version: env!("CARGO_PKG_VERSION"),
        status: "ok",
    })
}

/// Build the GET /v1/peers response body from a peer snapshot, keeping only peers
/// that are both trusted and online.
///
/// Trusted means the device completed the pairing ceremony and is in the trust
/// store; online means discovery still holds a presence entry for it, since stale
/// entries are evicted on timeout. Untrusted peers are excluded because Synapt has
/// no session key for them, so any clip send SynaptClip attempted would fail.
/// `online` is therefore always true and `last_seen` reflects the snapshot time.
fn peers_response(
    peers: &[synapt_core::Peer],
    trusted_ids: &HashSet<String>,
) -> serde_json::Value {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let peer_list: Vec<serde_json::Value> = peers
        .iter()
        .filter(|p| trusted_ids.contains(&p.device_id.to_string()))
        .map(|p| {
            serde_json::json!({
                "id":        p.device_id.to_string(),
                "name":      p.device_name,
                "ip":        p.ip.to_string(),
                "port":      crate::network::TRANSFER_PORT,
                "online":    true,
                "last_seen": now,
            })
        })
        .collect();

    serde_json::json!({
        "api_version": "1",
        "peers": peer_list,
    })
}

async fn peers_handler(State(state): State<IpcState>) -> Json<serde_json::Value> {
    let trusted_ids = match state.trusted_ids.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let peers = crate::network::list_peers(&state.peer_map);
    Json(peers_response(&peers, &trusted_ids))
}

/// Body of POST /v1/clips/send. Fields default to empty so a body missing a field
/// is rejected by our own validation with the contract error shape, rather than by
/// the extractor with a non-conforming body.
#[derive(Deserialize)]
struct ClipSendRequest {
    #[serde(default)]
    peer_id: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    content_type: String,
}

/// Build a contract-shaped error response: { api_version, error, message }.
fn api_error(
    status: StatusCode,
    error: &str,
    message: &str,
) -> (StatusCode, Json<serde_json::Value>) {
    (
        status,
        Json(serde_json::json!({
            "api_version": "1",
            "error":       error,
            "message":     message,
        })),
    )
}

/// Validate a clip-send request and resolve its target to a trusted, online peer.
/// On rejection returns (status, error_code, message) for `api_error`.
fn validate_and_resolve(
    body: &ClipSendRequest,
    peer_map: &crate::network::PeerMap,
    trusted_ids: &Arc<Mutex<HashSet<String>>>,
) -> Result<synapt_core::Peer, (StatusCode, &'static str, &'static str)> {
    // Only text is supported in v1.
    if body.content_type != "text" {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "unsupported_content_type",
            "Only content_type 'text' is supported in v1",
        ));
    }

    // Required fields must be present and non-empty.
    if body.peer_id.is_empty() || body.content.is_empty() {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "missing_fields",
            "peer_id and content are required",
        ));
    }

    // Resolve the peer; an unparseable id cannot match any known peer.
    let peer_id_uuid = uuid::Uuid::parse_str(&body.peer_id).map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "peer_not_found",
            "Invalid peer_id format",
        )
    })?;
    let peer = crate::network::list_peers(peer_map)
        .into_iter()
        .find(|p| p.device_id == peer_id_uuid)
        .ok_or((
            StatusCode::NOT_FOUND,
            "peer_not_found",
            "No peer with that ID is currently online",
        ))?;

    // The transfer layer rejects untrusted peers, so refuse early with a clear error.
    let is_trusted = trusted_ids
        .lock()
        .map(|set| set.contains(&peer.device_id.to_string()))
        .unwrap_or(false);
    if !is_trusted {
        return Err((
            StatusCode::NOT_FOUND,
            "peer_not_found",
            "Peer is not trusted. Pair the device first.",
        ));
    }

    Ok(peer)
}

/// Resolve the configured download directory, mirroring the command layer.
fn download_dir_from(setting: Option<String>) -> std::path::PathBuf {
    match setting {
        Some(d) if !d.is_empty() => std::path::PathBuf::from(d),
        _ => dirs::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("Synapt"),
    }
}

/// POST /v1/clips/send — accept text from the local SynaptClip and deliver it to a
/// trusted peer over the existing encrypted transfer layer. Returns 202 immediately;
/// the transfer runs asynchronously.
async fn clips_send_handler(
    State(state): State<IpcState>,
    Json(body): Json<ClipSendRequest>,
) -> (StatusCode, Json<serde_json::Value>) {
    let peer = match validate_and_resolve(&body, &state.peer_map, &state.trusted_ids) {
        Ok(p) => p,
        Err((status, error, message)) => return api_error(status, error, message),
    };

    // Wrap the clip text as a temp file the transfer layer can send. The
    // <uuid>.txt naming under synapt-clips is what the receive-side webhook detects.
    let transfer_id = uuid::Uuid::new_v4();
    let tmp_dir = std::env::temp_dir().join("synapt-clips");
    let _ = std::fs::create_dir_all(&tmp_dir);
    let tmp_path = tmp_dir.join(format!("{}.txt", transfer_id));
    if let Err(e) = std::fs::write(&tmp_path, body.content.as_bytes()) {
        tracing::error!("ipc: failed to write clip temp file: {}", e);
        return api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "transfer_unavailable",
            "Could not prepare clip for transfer",
        );
    }

    let identity = Arc::clone(&state.identity);
    let db = Arc::clone(&state.db);
    let queue = Arc::clone(&state.transfer_queue);
    let app = state.app.clone();
    let path_str = tmp_path.to_string_lossy().to_string();
    let peer_name = peer.device_name.clone();
    let peer_ip = peer.ip;
    let peer_id = peer.device_id.to_string();

    tokio::spawn(async move {
        let download_dir = download_dir_from(
            db.get_setting("download_dir").await.ok().flatten(),
        );

        let result = crate::network::transfer::request_transfer_with_retry(
            &peer_id,
            peer_ip,
            path_str.clone(),
            &identity,
            &db,
            &download_dir,
            &peer_name,
            &app,
            &queue,
        )
        .await;

        // Remove the temp file whether the transfer succeeded or failed.
        let _ = std::fs::remove_file(&path_str);

        if let Err(e) = result {
            tracing::error!("ipc: clip transfer failed: {}", e);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({
            "api_version": "1",
            "transfer_id": transfer_id.to_string(),
            "status":      "queued",
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_handler_returns_version_one_and_ok() {
        let Json(resp) = health_handler().await;
        assert_eq!(resp.api_version, "1");
        assert_eq!(resp.status, "ok");
    }

    #[test]
    fn require_loopback_rejects_non_loopback() {
        let addr: SocketAddr = "192.168.1.1:1234".parse().unwrap();
        assert_eq!(check_loopback(&addr), Err(StatusCode::FORBIDDEN));
    }

    #[test]
    fn require_loopback_allows_loopback() {
        let addr: SocketAddr = "127.0.0.1:1234".parse().unwrap();
        assert_eq!(check_loopback(&addr), Ok(()));
    }

    #[test]
    fn peers_handler_returns_api_version_one() {
        let body = peers_response(&[], &HashSet::new());
        assert_eq!(body["api_version"], "1");
    }

    #[test]
    fn peers_handler_empty_map_returns_empty_array() {
        let body = peers_response(&[], &HashSet::new());
        let arr = body["peers"].as_array().expect("peers must be an array");
        assert!(arr.is_empty());
    }

    /// Build an online discovery peer with a fixed id for the trust-filter tests.
    fn online_peer(device_id: uuid::Uuid) -> synapt_core::Peer {
        synapt_core::Peer {
            device_id,
            device_name: "Test Device".into(),
            ip: "192.168.1.42".parse().expect("literal is a valid IP"),
            pairing_port: crate::network::PAIRING_PORT,
            status: synapt_core::PeerStatus::Discovered,
        }
    }

    #[test]
    fn peers_handler_omits_online_peer_that_is_not_trusted() {
        let peer = online_peer(uuid::Uuid::nil());
        let body = peers_response(&[peer], &HashSet::new());
        let arr = body["peers"].as_array().expect("peers must be an array");
        assert!(
            arr.is_empty(),
            "an online but unpaired peer must not be advertised to SynaptClip"
        );
    }

    #[test]
    fn peers_handler_returns_peer_that_is_both_online_and_trusted() {
        let id = uuid::Uuid::nil();
        let trusted: HashSet<String> = std::iter::once(id.to_string()).collect();
        let body = peers_response(&[online_peer(id)], &trusted);
        let arr = body["peers"].as_array().expect("peers must be an array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], id.to_string());
        assert_eq!(arr[0]["online"], true);
        assert_eq!(arr[0]["port"], crate::network::TRANSFER_PORT);
    }

    #[test]
    fn peers_handler_keeps_only_the_trusted_peer_of_a_mixed_set() {
        let trusted_id = uuid::Uuid::from_u128(1);
        let stranger_id = uuid::Uuid::from_u128(2);
        let trusted: HashSet<String> = std::iter::once(trusted_id.to_string()).collect();
        let body = peers_response(
            &[online_peer(trusted_id), online_peer(stranger_id)],
            &trusted,
        );
        let arr = body["peers"].as_array().expect("peers must be an array");
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["id"], trusted_id.to_string());
    }

    fn empty_state() -> (crate::network::PeerMap, Arc<Mutex<HashSet<String>>>) {
        (
            Arc::new(Mutex::new(std::collections::HashMap::new())),
            Arc::new(Mutex::new(HashSet::new())),
        )
    }

    #[test]
    fn clips_send_rejects_unsupported_content_type() {
        let (peers, trusted) = empty_state();
        let body = ClipSendRequest {
            peer_id: "some-id".into(),
            content: "data".into(),
            content_type: "image".into(),
        };
        let err = validate_and_resolve(&body, &peers, &trusted).unwrap_err();
        assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.1, "unsupported_content_type");
    }

    #[test]
    fn clips_send_rejects_empty_peer_id() {
        let (peers, trusted) = empty_state();
        let body = ClipSendRequest {
            peer_id: String::new(),
            content: "data".into(),
            content_type: "text".into(),
        };
        let err = validate_and_resolve(&body, &peers, &trusted).unwrap_err();
        assert_eq!(err.0, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(err.1, "missing_fields");
    }

    #[test]
    fn clips_send_rejects_peer_not_in_map() {
        let (peers, trusted) = empty_state();
        let body = ClipSendRequest {
            peer_id: "00000000-0000-0000-0000-000000000000".into(),
            content: "data".into(),
            content_type: "text".into(),
        };
        let err = validate_and_resolve(&body, &peers, &trusted).unwrap_err();
        assert_eq!(err.0, StatusCode::NOT_FOUND);
        assert_eq!(err.1, "peer_not_found");
    }

    #[test]
    fn api_error_has_contract_shape() {
        let (status, Json(v)) =
            api_error(StatusCode::NOT_FOUND, "peer_not_found", "nope");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(v["api_version"], "1");
        assert_eq!(v["error"], "peer_not_found");
        assert_eq!(v["message"], "nope");
    }
}
