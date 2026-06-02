use x25519_dalek::{StaticSecret, PublicKey};
use sha2::{Sha256, Digest};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::rngs::OsRng;
use uuid::Uuid;
use crate::storage::{Db, DbError, LocalDeviceRow, TrustedPeerRow};
use synapt_core::TrustedPeer;
use thiserror::Error;

/// Errors raised by the trust store.
#[derive(Debug, Error)]
pub enum TrustError {
    #[error("database error: {0}")]
    Db(#[from] DbError),
    #[error("key decode error: {0}")]
    KeyDecode(String),
}

/// Local device identity kept in memory for the lifetime of the process.
#[derive(Debug, Clone)]
pub struct LocalIdentity {
    pub device_id:     Uuid,
    pub device_name:   String,
    pub pubkey_b64:    String,
    /// Raw private key bytes - never leave this process.
    pub privkey_bytes: [u8; 32],
}

/// Load identity from DB or generate on first run.
pub async fn init_identity(db: &Db) -> Result<LocalIdentity, TrustError> {
    if let Some(row) = db.get_local_device().await? {
        let privkey_bytes: [u8; 32] = BASE64.decode(&row.privkey_b64)
            .map_err(|e| TrustError::KeyDecode(e.to_string()))?
            .try_into()
            .map_err(|_| TrustError::KeyDecode("private key wrong length".into()))?;
        return Ok(LocalIdentity {
            device_id:    row.device_id.parse().map_err(|e: uuid::Error| TrustError::KeyDecode(e.to_string()))?,
            device_name:  row.device_name,
            pubkey_b64:   row.pubkey_b64,
            privkey_bytes,
        });
    }
    generate_and_store(db).await
}

async fn generate_and_store(db: &Db) -> Result<LocalIdentity, TrustError> {
    let secret        = StaticSecret::random_from_rng(OsRng);
    let public        = PublicKey::from(&secret);
    let privkey_bytes = secret.to_bytes();
    let pubkey_b64    = BASE64.encode(public.to_bytes());
    let privkey_b64   = BASE64.encode(privkey_bytes);
    let device_id     = Uuid::new_v4();
    let device_name   = hostname::get()
        .ok().and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| format!("device-{}", &device_id.to_string()[..8]));

    db.insert_local_device(&LocalDeviceRow {
        device_id:   device_id.to_string(),
        device_name: device_name.clone(),
        pubkey_b64:  pubkey_b64.clone(),
        privkey_b64,
    }).await?;

    Ok(LocalIdentity { device_id, device_name, pubkey_b64, privkey_bytes })
}

/// Derive a hex SHA-256 fingerprint from a base64-encoded public key.
pub fn fingerprint(pubkey_b64: &str) -> Result<String, TrustError> {
    let bytes = BASE64.decode(pubkey_b64).map_err(|e| TrustError::KeyDecode(e.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

/// Load all trusted peers from the database.
pub async fn list_trusted_peers(db: &Db) -> Result<Vec<TrustedPeer>, TrustError> {
    let rows = db.get_trusted_peers().await?;
    rows.into_iter().map(|r| -> Result<TrustedPeer, TrustError> {
        Ok(TrustedPeer {
            device_id:   r.device_id.parse().map_err(|e: uuid::Error| TrustError::KeyDecode(e.to_string()))?,
            device_name: r.device_name,
            pubkey_b64:  r.pubkey_b64,
            fingerprint: r.fingerprint,
            paired_at:   r.paired_at,
            last_seen:   r.last_seen,
        })
    }).collect()
}

/// Revoke a trusted peer.
pub async fn revoke_peer(db: &Db, device_id: &str) -> Result<(), TrustError> {
    db.remove_trusted_peer(device_id).await.map_err(TrustError::Db)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fingerprint_valid_key_returns_64_char_hex() {
        let key = BASE64.encode([0u8; 32]);
        let fp = fingerprint(&key).unwrap();
        assert_eq!(fp.len(), 64);
        assert!(fp.chars().all(|c| c.is_ascii_hexdigit()));
    }
    #[test]
    fn fingerprint_invalid_base64_returns_err() {
        assert!(fingerprint("not-valid-base64!!!").is_err());
    }
}
