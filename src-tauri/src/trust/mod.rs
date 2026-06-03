pub mod store;
pub use store::{LocalIdentity, TrustError, init_identity, fingerprint, list_trusted_peers, revoke_peer};
