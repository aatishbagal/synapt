use axum::{routing::get, Json, Router};
use serde::Serialize;
use std::net::SocketAddr;

/// Loopback port the IPC server binds. SynaptClip connects here in v0.5.
const IPC_PORT: u16 = 57321;

#[derive(Serialize)]
struct HealthResponse {
    api_version: &'static str,
    synapt_version: &'static str,
    status: &'static str,
}

/// Start the loopback IPC server. Binding failure is logged and tolerated so the
/// rest of Synapt continues to run normally.
pub async fn start() {
    let app = Router::new().route("/v1/health", get(health_handler));
    // /v1/peers and /v1/clips/send are added in v0.5.

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

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("IPC server error: {}", e);
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        api_version: "1",
        synapt_version: env!("CARGO_PKG_VERSION"),
        status: "ok",
    })
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
}
