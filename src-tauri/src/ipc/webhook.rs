//! Outbound webhook: Synapt notifies the local SynaptClip instance when a clip
//! arrives from a remote peer. This is the only direction in which Synapt calls
//! out to SynaptClip (loopback port 57322).

use reqwest::Client;
use std::path::Path;
use std::time::Duration;

/// SynaptClip's loopback listener port. Synapt only ever POSTs here, never binds it.
const SYNAPTCLIP_PORT: u16 = 57322;

/// Webhook calls give up after this long; SynaptClip may not be installed.
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(2);

/// Deliver a received clip to the local SynaptClip instance. Returns silently
/// regardless of outcome: SynaptClip may not be running, which is normal.
pub async fn deliver_clip(sender_peer_id: &str, sender_name: &str, content: &str) {
    let client = match Client::builder().timeout(WEBHOOK_TIMEOUT).build() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("webhook: failed to build client: {}", e);
            return;
        }
    };

    let payload = serde_json::json!({
        "sender_peer_id": sender_peer_id,
        "sender_name":    sender_name,
        "content":        content,
        "content_type":   "text",
        "received_at":    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
    });
    // reqwest is built without its json feature, so serialize and send the bytes.
    let body = match serde_json::to_vec(&payload) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("webhook: failed to serialize clip: {}", e);
            return;
        }
    };

    let url = format!("http://127.0.0.1:{}/v1/clips/receive", SYNAPTCLIP_PORT);
    match client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::debug!("webhook: clip delivered to SynaptClip");
        }
        Ok(resp) => {
            tracing::warn!("webhook: SynaptClip returned status {}", resp.status());
        }
        Err(e) => {
            // SynaptClip not reachable is the normal case; log quietly and discard.
            tracing::debug!("webhook: SynaptClip not reachable ({})", e);
        }
    }
}

/// Heuristic: a received file is a SynaptClip clip when it is named `<uuid>.txt`,
/// matching the convention used by the clips/send handler. A non-clip txt file with
/// a UUID name (unlikely in practice) would also match; acceptable for v1.
pub fn is_synaptclip_clip(path: &Path) -> bool {
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    path.extension().and_then(|e| e.to_str()) == Some("txt")
        && uuid::Uuid::parse_str(stem).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_clip_true_for_uuid_txt() {
        let p = PathBuf::from("/tmp/synapt-clips/550e8400-e29b-41d4-a716-446655440000.txt");
        assert!(is_synaptclip_clip(&p));
    }

    #[test]
    fn is_clip_false_for_report_txt() {
        assert!(!is_synaptclip_clip(&PathBuf::from("/tmp/report.txt")));
    }

    #[test]
    fn is_clip_false_for_uuid_without_extension() {
        let p = PathBuf::from("/tmp/550e8400-e29b-41d4-a716-446655440000");
        assert!(!is_synaptclip_clip(&p));
    }

    #[tokio::test]
    async fn deliver_clip_no_panic_when_unreachable() {
        // Nothing listens on 57322 in tests; the call must return quietly.
        deliver_clip("peer-id", "peer-name", "hello").await;
    }
}
