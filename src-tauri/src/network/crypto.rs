use x25519_dalek::{EphemeralSecret, StaticSecret, PublicKey};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit};
use chacha20poly1305::aead::OsRng as AeadOsRng;
use hkdf::Hkdf;
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rand::rngs::OsRng;
use thiserror::Error;

/// Errors raised by the cryptographic layer.
#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("encrypt failed")]
    Encrypt,
    #[error("decrypt failed - message may be tampered")]
    Decrypt,
    #[error("key decode error: {0}")]
    KeyDecode(String),
    #[error("key derivation failed")]
    Hkdf,
    #[error("frame too large: {0} bytes")]
    FrameTooLarge(usize),
}

pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;

/// Generate an ephemeral X25519 keypair. Returns (secret, public_key_bytes).
pub fn generate_ephemeral() -> (EphemeralSecret, [u8; 32]) {
    let secret = EphemeralSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    (secret, public.to_bytes())
}

/// DH with an ephemeral secret and a received public key. Returns shared secret.
pub fn ephemeral_dh(secret: EphemeralSecret, their_pubkey: &[u8; 32]) -> [u8; 32] {
    secret.diffie_hellman(&PublicKey::from(*their_pubkey)).to_bytes()
}

/// DH with a long-term static private key and a received public key.
pub fn static_dh(privkey: &[u8; 32], their_pubkey: &[u8; 32]) -> [u8; 32] {
    StaticSecret::from(*privkey)
        .diffie_hellman(&PublicKey::from(*their_pubkey))
        .to_bytes()
}

/// Derive the 8-digit pairing verification code.
/// HKDF-SHA256 with salt = b"synapt-pair-verify", expand 3 bytes, encode as 8-digit decimal.
pub fn pairing_code(shared_secret: &[u8; 32]) -> String {
    let hk = Hkdf::<Sha256>::new(Some(b"synapt-pair-verify"), shared_secret);
    let mut okm = [0u8; 3];
    // A 3-byte HKDF-SHA256 expand is far below the 255*HashLen limit, so it cannot fail.
    let _ = hk.expand(b"", &mut okm);
    let n = u32::from_be_bytes([0, okm[0], okm[1], okm[2]]) % 100_000_000;
    format!("{:08}", n)
}

/// Derive a 32-byte session key from a long-term DH secret and a session nonce.
/// HKDF-SHA256: salt = nonce, info = b"synapt-session-key".
pub fn session_key(shared_secret: &[u8; 32], nonce: &[u8; 32]) -> Result<[u8; 32], CryptoError> {
    let hk = Hkdf::<Sha256>::new(Some(nonce.as_ref()), shared_secret);
    let mut key = [0u8; 32];
    hk.expand(b"synapt-session-key", &mut key).map_err(|_| CryptoError::Hkdf)?;
    Ok(key)
}

/// Encrypt plaintext. Returns [12-byte nonce || ciphertext+tag].
pub fn encrypt(key_bytes: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes));
    let nonce  = ChaCha20Poly1305::generate_nonce(&mut AeadOsRng);
    let ct     = cipher.encrypt(&nonce, plaintext).map_err(|_| CryptoError::Encrypt)?;
    let mut out = nonce.to_vec();
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt payload produced by encrypt(). Expects [12-byte nonce || ciphertext+tag].
pub fn decrypt(key_bytes: &[u8; 32], data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < 12 {
        return Err(CryptoError::Decrypt);
    }
    let (nonce_bytes, ct) = data.split_at(12);
    let cipher = ChaCha20Poly1305::new(Key::from_slice(key_bytes));
    cipher.decrypt(Nonce::from_slice(nonce_bytes), ct).map_err(|_| CryptoError::Decrypt)
}

/// Encode bytes as a 4-byte LE length prefix + payload.
pub fn write_frame(payload: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(CryptoError::FrameTooLarge(payload.len()));
    }
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Read the declared payload length from a 4-byte LE prefix buffer.
pub fn read_frame_len(buf: &[u8; 4]) -> usize {
    u32::from_le_bytes(*buf) as usize
}

/// Encode bytes as a standard base64 string.
pub fn to_b64(b: &[u8]) -> String {
    BASE64.encode(b)
}

/// Decode a standard base64 string into bytes.
pub fn from_b64(s: &str) -> Result<Vec<u8>, CryptoError> {
    BASE64.decode(s).map_err(|e| CryptoError::KeyDecode(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pairing_code_is_8_digits() {
        let code = pairing_code(&[0u8; 32]);
        assert_eq!(code.len(), 8);
        assert!(code.parse::<u32>().is_ok());
    }
    #[test]
    fn pairing_code_is_deterministic() {
        assert_eq!(pairing_code(&[1u8; 32]), pairing_code(&[1u8; 32]));
    }
    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = [7u8; 32];
        let ct = encrypt(&key, b"hello synapt").unwrap();
        let pt = decrypt(&key, &ct).unwrap();
        assert_eq!(pt, b"hello synapt");
    }
    #[test]
    fn decrypt_tampered_returns_err() {
        let key = [7u8; 32];
        let mut ct = encrypt(&key, b"data").unwrap();
        *ct.last_mut().unwrap() ^= 0xff;
        assert!(decrypt(&key, &ct).is_err());
    }
    #[test]
    fn frame_roundtrip() {
        let payload = vec![1u8; 100];
        let frame = write_frame(&payload).unwrap();
        let len = read_frame_len(&frame[..4].try_into().unwrap());
        assert_eq!(len, 100);
        assert_eq!(&frame[4..], payload.as_slice());
    }
    #[test]
    fn frame_too_large_returns_err() {
        let big = vec![0u8; MAX_FRAME_BYTES + 1];
        assert!(matches!(write_frame(&big), Err(CryptoError::FrameTooLarge(_))));
    }
}
