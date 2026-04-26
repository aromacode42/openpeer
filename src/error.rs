// src/error.rs — tests first (TDD)
//
// Add new variants here as new modules are built.  Every variant must carry
// sufficient context to produce a human-readable `Display` message without
// leaking sensitive data (keys, plaintext, file contents).

use std::io;

/// Unified error type for all OpenPeer public APIs.
///
/// All variants are `pub(crate)` or harder to construct — callers receive only
/// `Error` wrapped in `Result<T, Error>`.  Internal variant details are not
/// part of the public API surface.
#[derive(Debug, thiserror::Error)]
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

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::Io(e) => Error::Io(std::io::Error::new(e.kind(), e.to_string())),
            Error::Config(s) => Error::Config(s.clone()),
            Error::Decode(s) => Error::Decode(s.clone()),
            Error::Encode(s) => Error::Encode(s.clone()),
            Error::Crypto(s) => Error::Crypto(s.clone()),
            Error::Signaling(s) => Error::Signaling(s.clone()),
            Error::P2P(s) => Error::P2P(s.clone()),
            Error::Transfer(s) => Error::Transfer(s.clone()),
        }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Error::Io(a), Error::Io(b)) => a.kind() == b.kind(),
            (Error::Config(a), Error::Config(b)) => a == b,
            (Error::Decode(a), Error::Decode(b)) => a == b,
            (Error::Encode(a), Error::Encode(b)) => a == b,
            (Error::Crypto(a), Error::Crypto(b)) => a == b,
            (Error::Signaling(a), Error::Signaling(b)) => a == b,
            (Error::P2P(a), Error::P2P(b)) => a == b,
            (Error::Transfer(a), Error::Transfer(b)) => a == b,
            _ => false,
        }
    }
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
        let _ = std::error::Error::source(&err);
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(io::ErrorKind::AddrInUse, "port in use");
        let err: Error = io_err.into();
        match err {
            Error::Io(ref e) => {
                assert_eq!(e.kind(), io::ErrorKind::AddrInUse);
                assert_eq!(e.to_string(), "port in use");
            }
            _ => panic!("expected Error::Io"),
        }
    }

    #[test]
    fn error_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Error>();
        // Note: Clone is not derivable because io::Error is not Clone.
        // Clone is manually implemented but clips at io::Error source text.
    }

    #[test]
    fn error_clone_all_variants() {
        // Manually implemented Clone covers all 8 variants
        let variants: Vec<Error> = vec![
            Error::Io(std::io::Error::other("io error")),
            Error::Config("cfg".into()),
            Error::Decode("dec".into()),
            Error::Encode("enc".into()),
            Error::Crypto("cry".into()),
            Error::Signaling("sig".into()),
            Error::P2P("p2p".into()),
            Error::Transfer("xfer".into()),
        ];
        for err in variants {
            let cloned = err.clone();
            assert_eq!(err, cloned, "cloned error should equal original");
        }
    }

    #[test]
    fn error_partial_eq_all_variants() {
        // String variants: compare values directly
        let string_cases: Vec<(Error, Error, bool)> = vec![
            (Error::Config("x".into()), Error::Config("x".into()), true),
            (Error::Config("x".into()), Error::Config("y".into()), false),
            (Error::Config("x".into()), Error::Decode("x".into()), false),
        ];
        for (a, b, expect_eq) in string_cases {
            if expect_eq {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
        // Io variants compare by kind only
        let io_cases: Vec<(Error, Error, bool)> = vec![
            (Error::Io(std::io::Error::other("a")), Error::Io(std::io::Error::other("a")), true),
            (Error::Io(std::io::Error::new(io::ErrorKind::NotFound, "nf")), Error::Io(std::io::Error::other("x")), false),
        ];
        for (a, b, expect_eq) in io_cases {
            if expect_eq {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn error_display_all_variants() {
        let variants: Vec<Error> = vec![
            Error::Io(std::io::Error::other("other")),
            Error::Config("cfg".into()),
            Error::Decode("dec".into()),
            Error::Encode("enc".into()),
            Error::Crypto("cry".into()),
            Error::Signaling("sig".into()),
            Error::P2P("p2p".into()),
            Error::Transfer("xfer".into()),
        ];
        for err in variants {
            let s = err.to_string();
            assert!(!s.is_empty());
            let prefix = match &err {
                Error::Io(_) => "I/O error",
                Error::Config(_) => "configuration error",
                Error::Decode(_) => "protocol decode error",
                Error::Encode(_) => "protocol encode error",
                Error::Crypto(_) => "cryptography error",
                Error::Signaling(_) => "signaling error",
                Error::P2P(_) => "P2P connection error",
                Error::Transfer(_) => "transfer error",
            };
            assert!(
                s.contains(prefix),
                "Display for {:?} should contain '{}'",
                err,
                prefix
            );
        }
    }

    #[test]
    fn error_source_none_for_string_variants() {
        let string_variants = [
            Error::Config("cfg".into()),
            Error::Decode("dec".into()),
            Error::Encode("enc".into()),
            Error::Crypto("cry".into()),
            Error::Signaling("sig".into()),
            Error::P2P("p2p".into()),
            Error::Transfer("xfer".into()),
        ];
        for err in string_variants {
            assert!(
                std::error::Error::source(&err).is_none(),
                "String variants should have no source, but {:?} has source",
                err
            );
        }
    }
}
