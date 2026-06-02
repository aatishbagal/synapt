use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, Mutex};
use tauri::{AppHandle, Emitter};

use crate::network::crypto::{
    ephemeral_dh, from_b64, generate_ephemeral, pairing_code, read_frame_len, to_b64, write_frame,
    CryptoError,
};
use crate::network::discovery::PAIRING_PORT;
use crate::storage::{Db, DbError, TrustedPeerRow};
use crate::trust::{fingerprint, LocalIdentity, TrustError};
use synapt_core::TrustedPeer;

const MAX_PAIR_MSG: usize = 65536;

/// Wire messages exchanged during the pairing ceremony.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PairMsg {
    PairRequest { device_id: String, device_name: String, pubkey_b64: String },
    PairAccept { device_id: String, device_name: String, pubkey_b64: String },
    PairRejected,
    PairConfirm { longterm_pubkey_b64: String },
    PairComplete { longterm_pubkey_b64: String },
}

/// Errors raised during the pairing ceremony.
#[derive(Debug, Error)]
pub enum PairingError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("pairing rejected by peer")]
    Rejected,
    #[error("pairing timed out")]
    Timeout,
    #[error("unexpected message: {0}")]
    UnexpectedMsg(String),
    #[error("db error: {0}")]
    Db(#[from] DbError),
    #[error("trust error: {0}")]
    Trust(#[from] TrustError),
    #[error("event emit error: {0}")]
    Emit(#[from] tauri::Error),
}

/// An in-progress outbound pairing awaiting user confirmation on the initiator side.
pub struct PendingPairing {
    pub stream:          TcpStream,
    pub shared_secret:   [u8; 32],
    pub peer_device_id:  String,
    pub peer_name:       String,
    pub peer_pubkey_b64: String,
    pub verify_code:     String,
}

fn to_key32(v: Vec<u8>) -> Result<[u8; 32], PairingError> {
    v.try_into()
        .map_err(|_| PairingError::UnexpectedMsg("public key wrong length".into()))
}

async fn send_msg(stream: &mut TcpStream, msg: &PairMsg) -> Result<(), PairingError> {
    let json = serde_json::to_vec(msg)?;
    let frame = write_frame(&json)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

async fn recv_msg(stream: &mut TcpStream) -> Result<PairMsg, PairingError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = read_frame_len(&len_buf);
    if len > MAX_PAIR_MSG {
        return Err(PairingError::UnexpectedMsg(format!("frame too large: {len}")));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(serde_json::from_slice(&buf)?)
}

/// Initiate pairing with a peer. Returns a PendingPairing awaiting user confirmation.
pub async fn begin_pairing(
    peer_ip: IpAddr,
    peer_pairing_port: u16,
    identity: &LocalIdentity,
) -> Result<PendingPairing, PairingError> {
    let connect = TcpStream::connect((peer_ip, peer_pairing_port));
    let mut stream = tokio::time::timeout(Duration::from_secs(10), connect)
        .await
        .map_err(|_| PairingError::Timeout)??;

    let (secret, my_pub) = generate_ephemeral();
    send_msg(
        &mut stream,
        &PairMsg::PairRequest {
            device_id: identity.device_id.to_string(),
            device_name: identity.device_name.clone(),
            pubkey_b64: to_b64(&my_pub),
        },
    )
    .await?;

    match recv_msg(&mut stream).await? {
        PairMsg::PairAccept { device_id, device_name, pubkey_b64 } => {
            let their_pub = to_key32(from_b64(&pubkey_b64)?)?;
            let shared_secret = ephemeral_dh(secret, &their_pub);
            let verify_code = pairing_code(&shared_secret);
            Ok(PendingPairing {
                stream,
                shared_secret,
                peer_device_id: device_id,
                peer_name: device_name,
                peer_pubkey_b64: pubkey_b64,
                verify_code,
            })
        }
        PairMsg::PairRejected => Err(PairingError::Rejected),
        other => Err(PairingError::UnexpectedMsg(format!("{other:?}"))),
    }
}

/// Complete pairing after the user has confirmed the verification code matches.
pub async fn confirm_pairing(
    mut pending: PendingPairing,
    identity: &LocalIdentity,
    db: &Db,
) -> Result<TrustedPeer, PairingError> {
    send_msg(
        &mut pending.stream,
        &PairMsg::PairConfirm { longterm_pubkey_b64: identity.pubkey_b64.clone() },
    )
    .await?;

    let their_longterm = match recv_msg(&mut pending.stream).await? {
        PairMsg::PairComplete { longterm_pubkey_b64 } => longterm_pubkey_b64,
        PairMsg::PairRejected => return Err(PairingError::Rejected),
        other => return Err(PairingError::UnexpectedMsg(format!("{other:?}"))),
    };

    let fp = fingerprint(&their_longterm)?;
    let paired_at = chrono::Utc::now().timestamp();

    db.upsert_trusted_peer(&TrustedPeerRow {
        device_id: pending.peer_device_id.clone(),
        device_name: pending.peer_name.clone(),
        pubkey_b64: their_longterm.clone(),
        fingerprint: fp.clone(),
        paired_at,
        last_seen: None,
    })
    .await?;

    Ok(TrustedPeer {
        device_id: pending
            .peer_device_id
            .parse()
            .map_err(|e: uuid::Error| PairingError::UnexpectedMsg(e.to_string()))?,
        device_name: pending.peer_name,
        pubkey_b64: their_longterm,
        fingerprint: fp,
        paired_at,
        last_seen: None,
    })
}

/// Run the pairing responder server, accepting inbound pairing connections.
/// For each connection a fresh oneshot channel is created; the sender is stored
/// in `pair_tx` for the accept/reject commands and the receiver is handed to the
/// responder task to await the user's decision.
pub async fn start_pairing_server(
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    app: AppHandle,
    pair_tx: Arc<Mutex<Option<oneshot::Sender<bool>>>>,
) -> Result<(), PairingError> {
    let listener = TcpListener::bind(format!("0.0.0.0:{PAIRING_PORT}")).await?;
    tracing::info!("pairing server listening on port {}", PAIRING_PORT);

    loop {
        let (stream, addr) = listener.accept().await?;
        let identity = Arc::clone(&identity);
        let db = Arc::clone(&db);
        let app = app.clone();

        let (decision_tx, decision_rx) = oneshot::channel();
        *pair_tx.lock().await = Some(decision_tx);

        tokio::spawn(async move {
            if let Err(e) = handle_responder(stream, addr, identity, db, app, decision_rx).await {
                tracing::warn!("pairing responder error from {}: {}", addr, e);
            }
        });
    }
}

async fn handle_responder(
    mut stream: TcpStream,
    addr: SocketAddr,
    identity: Arc<LocalIdentity>,
    db: Arc<Db>,
    app: AppHandle,
    decision_rx: oneshot::Receiver<bool>,
) -> Result<(), PairingError> {
    let (their_id, their_name, their_eph_b64) = match recv_msg(&mut stream).await? {
        PairMsg::PairRequest { device_id, device_name, pubkey_b64 } => {
            (device_id, device_name, pubkey_b64)
        }
        other => return Err(PairingError::UnexpectedMsg(format!("{other:?}"))),
    };

    let their_eph = to_key32(from_b64(&their_eph_b64)?)?;
    let (secret, my_pub) = generate_ephemeral();
    let shared_secret = ephemeral_dh(secret, &their_eph);
    let verify_code = pairing_code(&shared_secret);

    app.emit(
        "pair-request",
        serde_json::json!({
            "device_id": their_id,
            "device_name": their_name,
            "ip": addr.ip().to_string(),
            "verify_code": verify_code,
        }),
    )?;

    send_msg(
        &mut stream,
        &PairMsg::PairAccept {
            device_id: identity.device_id.to_string(),
            device_name: identity.device_name.clone(),
            pubkey_b64: to_b64(&my_pub),
        },
    )
    .await?;

    let accepted = decision_rx.await.unwrap_or(false);

    if !accepted {
        send_msg(&mut stream, &PairMsg::PairRejected).await?;
        return Ok(());
    }

    let their_longterm = match recv_msg(&mut stream).await? {
        PairMsg::PairConfirm { longterm_pubkey_b64 } => longterm_pubkey_b64,
        other => return Err(PairingError::UnexpectedMsg(format!("{other:?}"))),
    };

    let fp = fingerprint(&their_longterm)?;
    let paired_at = chrono::Utc::now().timestamp();

    db.upsert_trusted_peer(&TrustedPeerRow {
        device_id: their_id.clone(),
        device_name: their_name.clone(),
        pubkey_b64: their_longterm.clone(),
        fingerprint: fp.clone(),
        paired_at,
        last_seen: None,
    })
    .await?;

    send_msg(
        &mut stream,
        &PairMsg::PairComplete { longterm_pubkey_b64: identity.pubkey_b64.clone() },
    )
    .await?;

    let trusted = TrustedPeer {
        device_id: their_id
            .parse()
            .map_err(|e: uuid::Error| PairingError::UnexpectedMsg(e.to_string()))?,
        device_name: their_name,
        pubkey_b64: their_longterm,
        fingerprint: fp,
        paired_at,
        last_seen: None,
    };
    app.emit("pair-complete", &trusted)?;
    Ok(())
}
