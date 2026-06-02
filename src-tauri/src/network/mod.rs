pub mod discovery;
pub mod crypto;
pub mod transfer;
pub mod peer;
pub mod search_server;

use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::network::crypto::{
    decrypt, encrypt, from_b64, read_frame_len, session_key, static_dh, to_b64, write_frame,
    CryptoError, MAX_FRAME_BYTES,
};
use crate::storage::Db;
use crate::trust::LocalIdentity;

// Public module surface; some items are consumed via direct paths elsewhere.
#[allow(unused_imports)]
pub use discovery::{PeerMap, PeerEntry, list_peers, start as start_discovery,
                    PAIRING_PORT, SEARCH_PORT, TRANSFER_PORT};
pub use search_server::start_search_server;

/// Errors raised while establishing or using an encrypted session.
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("crypto: {0}")]
    Crypto(#[from] CryptoError),
    #[error("not trusted")]
    NotTrusted,
    #[error("db: {0}")]
    Db(#[from] crate::storage::DbError),
}

/// Plaintext handshake packet sent first by the initiator.
#[derive(Debug, Serialize, Deserialize)]
struct HandshakeInit {
    device_id: String,
}

/// Plaintext handshake reply from the responder.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerHello {
    Hello { nonce_b64: String },
    Rejected,
}

fn to_key32(v: Vec<u8>) -> Result<[u8; 32], SessionError> {
    v.try_into()
        .map_err(|_| SessionError::Crypto(CryptoError::KeyDecode("key/nonce wrong length".into())))
}

/// Write a length-prefixed plaintext frame.
pub(crate) async fn write_plain(stream: &mut TcpStream, bytes: &[u8]) -> Result<(), SessionError> {
    let frame = write_frame(bytes)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

/// Read a length-prefixed plaintext frame.
pub(crate) async fn read_plain(stream: &mut TcpStream) -> Result<Vec<u8>, SessionError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = read_frame_len(&len_buf);
    if len > MAX_FRAME_BYTES {
        return Err(SessionError::Crypto(CryptoError::FrameTooLarge(len)));
    }
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(buf)
}

/// Encrypt and write a length-prefixed frame using the session key.
pub(crate) async fn write_enc(
    stream: &mut TcpStream,
    key: &[u8; 32],
    plaintext: &[u8],
) -> Result<(), SessionError> {
    let ct = encrypt(key, plaintext)?;
    write_plain(stream, &ct).await
}

/// Read and decrypt a length-prefixed frame using the session key.
pub(crate) async fn read_enc(stream: &mut TcpStream, key: &[u8; 32]) -> Result<Vec<u8>, SessionError> {
    let ct = read_plain(stream).await?;
    Ok(decrypt(key, &ct)?)
}

/// Responder side of the session handshake. Returns the derived session key.
pub async fn session_handshake_server(
    stream: &mut TcpStream,
    identity: &LocalIdentity,
    db: &Db,
) -> Result<[u8; 32], SessionError> {
    let init: HandshakeInit = serde_json::from_slice(&read_plain(stream).await?)?;

    let peers = db.get_trusted_peers().await?;
    let peer = match peers.iter().find(|p| p.device_id == init.device_id) {
        Some(p) => p.clone(),
        None => {
            write_plain(stream, &serde_json::to_vec(&ServerHello::Rejected)?).await?;
            return Err(SessionError::NotTrusted);
        }
    };

    let their_pub = to_key32(from_b64(&peer.pubkey_b64)?)?;
    let mut nonce = [0u8; 32];
    OsRng.fill_bytes(&mut nonce);
    let shared = static_dh(&identity.privkey_bytes, &their_pub);
    let key = session_key(&shared, &nonce)?;

    write_plain(
        stream,
        &serde_json::to_vec(&ServerHello::Hello { nonce_b64: to_b64(&nonce) })?,
    )
    .await?;

    Ok(key)
}

/// Initiator side of the session handshake. Returns the derived session key.
pub async fn session_handshake_client(
    stream: &mut TcpStream,
    local_device_id: &str,
    identity: &LocalIdentity,
    peer_pubkey_b64: &str,
) -> Result<[u8; 32], SessionError> {
    write_plain(
        stream,
        &serde_json::to_vec(&HandshakeInit { device_id: local_device_id.to_string() })?,
    )
    .await?;

    let nonce = match serde_json::from_slice::<ServerHello>(&read_plain(stream).await?)? {
        ServerHello::Hello { nonce_b64 } => to_key32(from_b64(&nonce_b64)?)?,
        ServerHello::Rejected => return Err(SessionError::NotTrusted),
    };

    let their_pub = to_key32(from_b64(peer_pubkey_b64)?)?;
    let shared = static_dh(&identity.privkey_bytes, &their_pub);
    let key = session_key(&shared, &nonce)?;
    Ok(key)
}
