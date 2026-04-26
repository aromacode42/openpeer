//! OpenPeer — P2P end-to-end encrypted file transfer.
//!
//! ## Module map
//!
//! | Module | Purpose |
//! |---|---|
//! | [`protocol`] | Wire format messages + binary codec (no IO) |
//! | [`crypto`] | Noise Protocol session state machines (no IO) |
//! | [`signaling`] | Server-side registry and peer matchmaking |
//! | [`p2p`] | NAT punching, Noise handshake, encrypted stream |
//! | [`transfer`] | Chunked file protocol on encrypted stream |
//! | [`cli`] | User interaction only; no business logic |
//!
//! ## Public API
//!
//! All public APIs return `Result<T, crate::Error>`.

pub mod cli;
pub mod config;
pub mod crypto;
pub mod p2p;
pub mod protocol;
pub mod signaling;
pub mod transfer;

mod error;
pub use error::Error;
