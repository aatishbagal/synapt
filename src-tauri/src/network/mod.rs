pub mod discovery;
pub mod crypto;
pub mod transfer;
pub mod peer;

// Public module surface; some items are consumed via direct paths elsewhere.
#[allow(unused_imports)]
pub use discovery::{PeerMap, PeerEntry, list_peers, start as start_discovery,
                    PAIRING_PORT, SEARCH_PORT, TRANSFER_PORT};
