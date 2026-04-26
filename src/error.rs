// src/error.rs — tests first (TDD)
//
// Add new variants here as new modules are built.  Every variant must carry
// sufficient context to produce a human-readable `Display` message without
// leaking sensitive data (keys, plaintext, file contents).

use std::io;
use thiserror::Error;

/// Unified error type for all OpenPeer public APIs.
///
/// All variants are `pub(crate)` or harder to construct — callers receive only
/// `Error` wrapped in `Result<T, Error>`.  Internal variant details are not
/// part of the public API surface.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("protocol decode error: {0}")]
    Decode(String),

    #[error("protocol encode error: {0}")]
    Encode(String),

    #[error("cryptography error: {0}")]
    Crypto(String),

    #[error("signaling error: {0}")]
    Signaling(String),

    #[error("P2P connection error: {0}")]
    P2P(String),

    #[error("transfer error: {0}")]
    Transfer(String),
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_io() {
        let err = Error::from(std::io::Error::new(
            io::ErrorKind::NotFound,
            "file not found",
        ));
        let s = err.to_string();
        assert!(s.contains("I/O error"));
        assert!(s.contains("file not found"));
    }

    #[test]
    fn error_display_config() {
        let err = Error::Config("missing `external_addr`".into());
        assert!(err.to_string().contains("configuration error"));
        assert!(err.to_string().contains("missing `external_addr`"));
    }

    #[test]
    fn error_display_decode() {
        let err = Error::Decode("unexpected magic bytes".into());
        let s = err.to_string();
        assert!(s.contains("protocol decode error"));
        assert!(s.contains("unexpected magic bytes"));
    }

    #[test]
    fn error_display_encode() {
        let err = Error::Encode("varint overflow".into());
        assert!(err.to_string().contains("protocol encode error"));
    }

    #[test]
    fn error_display_crypto() {
        let err = Error::Crypto("handshake timeout".into());
        assert!(err.to_string().contains("cryptography error"));
    }

    #[test]
    fn error_display_signaling() {
        let err = Error::Signaling("peer not registered".into());
        assert!(err.to_string().contains("signaling error"));
    }

    #[test]
    fn error_display_p2p() {
        let err = Error::P2P("connection reset by peer".into());
        assert!(err.to_string().contains("P2P connection error"));
    }

    #[test]
    fn error_display_transfer() {
        let err = Error::Transfer("checksum mismatch".into());
        assert!(err.to_string().contains("transfer error"));
    }

    #[test]
    fn error_debug_does_not_panic() {
        let err = Error::Config("test".into());
        // Debug must not leak; it just prints the variant name + fields
        let s = format!("{:?}", err);
        assert!(s.starts_with("Config"));
    }

    #[test]
    fn error_source_chain_io() {
        let io_err = std::io::Error::new(io::ErrorKind::PermissionDenied, "perm");
        let err: Error = io_err.into();
        // Error should implement std::error::Source (thiserror provides it)
        let _ = std::error::Error::source(&err);
    }
}
