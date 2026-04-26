//! Configuration sources: TOML file < env vars < CLI flags.
//!
//! Priority (last wins): file → `OPENPEER_*` env vars → struct field values set at runtime.
//!
//! ## Env vars
//! - `OPENPEER_SIGNALING_PORT` — signaling TCP port (default 38901)
//! - `OPENPEER_TURN_PORT`      — TURN UDP port (default 38902)
//! - `OPENPEER_LOG`            — `RUST_LOG` filter (default `info`)
//! - `OPENPEER_IDENTITY_DIR`   — directory for identity key (default `~/.openpeer`)

use serde::Deserialize;
use std::io;
use std::net::SocketAddr;

/// Top-level configuration for both client and server binaries.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub signaling_port: u16,
    pub turn_port: u16,
    pub log_filter: String,
    pub identity_dir: std::path::PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            signaling_port: 38901,
            turn_port: 38902,
            log_filter: "info".into(),
            identity_dir: {
                let mut p = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                p.push(".openpeer");
                p
            },
        }
    }
}

impl Config {
    /// Load configuration from a TOML file, then apply environment overrides.
    pub fn from_file(path: &std::path::Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut cfg: Config =
            toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        cfg.apply_env();
        Ok(cfg)
    }

    /// Apply configuration from environment variables, overwriting matching fields.
    pub fn apply_env(&mut self) {
        if let Ok(v) = std::env::var("OPENPEER_SIGNALING_PORT") {
            self.signaling_port = v.parse().unwrap_or(self.signaling_port);
        }
        if let Ok(v) = std::env::var("OPENPEER_TURN_PORT") {
            self.turn_port = v.parse().unwrap_or(self.turn_port);
        }
        if let Ok(v) = std::env::var("OPENPEER_LOG") {
            self.log_filter = v;
        }
        if let Ok(v) = std::env::var("OPENPEER_IDENTITY_DIR") {
            self.identity_dir = std::path::PathBuf::from(v);
        }
    }

    /// Return the signaling address using the given bind IP with the configured port.
    pub fn signaling_addr(&self, bind: SocketAddr) -> SocketAddr {
        (bind.ip(), self.signaling_port).into()
    }
}

/// Wrapper around `dirs::home_dir()` — factored out so tests can mock it.
pub fn home_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_signaling_port() {
        let cfg = Config::default();
        assert_eq!(cfg.signaling_port, 38901);
        assert_eq!(cfg.turn_port, 38902);
        assert_eq!(cfg.log_filter, "info");
    }

    #[test]
    fn test_apply_env_signaling_port() {
        std::env::set_var("OPENPEER_SIGNALING_PORT", "443");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, 443);
        std::env::remove_var("OPENPEER_SIGNALING_PORT");
    }

    #[test]
    fn test_apply_env_turn_port() {
        std::env::set_var("OPENPEER_TURN_PORT", "443");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.turn_port, 443);
        std::env::remove_var("OPENPEER_TURN_PORT");
    }

    #[test]
    fn test_apply_env_log_filter() {
        std::env::set_var("OPENPEER_LOG", "debug");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.log_filter, "debug");
        std::env::remove_var("OPENPEER_LOG");
    }

    #[test]
    fn test_apply_env_identity_dir() {
        std::env::set_var("OPENPEER_IDENTITY_DIR", "/tmp/openpeer-test");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(
            cfg.identity_dir,
            std::path::PathBuf::from("/tmp/openpeer-test")
        );
        std::env::remove_var("OPENPEER_IDENTITY_DIR");
    }

    #[test]
    fn test_apply_env_invalid_port_ignores() {
        std::env::set_var("OPENPEER_SIGNALING_PORT", "not-a-number");
        let mut cfg = Config::default();
        let original = cfg.signaling_port;
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, original);
        std::env::remove_var("OPENPEER_SIGNALING_PORT");
    }

    #[test]
    fn test_apply_env() {
        let mut cfg = Config::default();
        cfg.apply_env();
        let _ = cfg;
    }

    #[test]
    fn test_signaling_addr() {
        let cfg = Config::default();
        let addr: SocketAddr = "1.2.3.4:0".parse().unwrap();
        let bound = cfg.signaling_addr(addr);
        assert_eq!(bound.port(), 38901);
        assert_eq!(bound.ip(), std::net::IpAddr::from([1, 2, 3, 4]));
    }

    #[test]
    fn test_from_file_not_found() {
        let result = Config::from_file(std::path::Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
    }

    #[test]
    fn test_from_file_valid_toml() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let path = tmp_dir.path().join("openpeer.toml");
        std::fs::write(
            &path,
            "signaling_port = 9999\nturn_port = 9998\nlog_filter = \"trace\"\nidentity_dir = \"/custom/path\"\n",
        )
        .unwrap();
        let cfg = Config::from_file(&path).unwrap();
        assert_eq!(cfg.signaling_port, 9999);
        assert_eq!(cfg.turn_port, 9998);
        assert_eq!(cfg.log_filter, "trace");
        assert_eq!(cfg.identity_dir, std::path::PathBuf::from("/custom/path"));
    }
}
