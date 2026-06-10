//! Local HTTP IPC surface for SynaptClip integration: the loopback server
//! (health, peers, clip send) and the outbound clip-delivery webhook.
pub mod server;
pub mod webhook;
