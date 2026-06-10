use axum::{
    extract::{ConnectInfo, State},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    routing::get,
    Json, Router,
};
use serde::Serialize;
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
    /// Device IDs of currently trusted peers. Consumed by the clip endpoints.
    #[allow(dead_code)]
    pub trusted_ids: Arc<Mutex<HashSet<String>>>,
    /// Persistent store. Consumed by the clip endpoints.
    #[allow(dead_code)]
    pub db: Arc<crate::storage::Db>,
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

/// Build the GET /v1/peers response body from a peer snapshot. Every peer in the
/// discovery map is currently reachable — stale entries are evicted by discovery,
/// so `online` is always true and `last_seen` reflects the snapshot time.
fn peers_response(peers: &[synapt_core::Peer]) -> serde_json::Value {
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let peer_list: Vec<serde_json::Value> = peers
        .iter()
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
    let peers = crate::network::list_peers(&state.peer_map);
    Json(peers_response(&peers))
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
        let body = peers_response(&[]);
        assert_eq!(body["api_version"], "1");
    }

    #[test]
    fn peers_handler_empty_map_returns_empty_array() {
        let body = peers_response(&[]);
        let arr = body["peers"].as_array().expect("peers must be an array");
        assert!(arr.is_empty());
    }
}
