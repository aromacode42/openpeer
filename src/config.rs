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
    /// Load configuration from a TOML file.
    pub fn from_file(path: &std::path::Path) -> io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let cfg: Config =
            toml::from_str(&text).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(cfg)
    }

    /// Apply configuration from environment variables, overwriting matching fields.
    /// Call this after `from_file` to implement: file < env < CLI precedence.
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

    struct EnvGuard(&'static str);
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var(self.0);
        }
    }

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
        let _guard = EnvGuard("OPENPEER_SIGNALING_PORT");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, 443);
    }

    #[test]
    fn test_apply_env_turn_port() {
        std::env::set_var("OPENPEER_TURN_PORT", "443");
        let _guard = EnvGuard("OPENPEER_TURN_PORT");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.turn_port, 443);
    }

    #[test]
    fn test_apply_env_log_filter() {
        std::env::set_var("OPENPEER_LOG", "debug");
        let _guard = EnvGuard("OPENPEER_LOG");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.log_filter, "debug");
    }

    #[test]
    fn test_apply_env_identity_dir() {
        std::env::set_var("OPENPEER_IDENTITY_DIR", "/tmp/openpeer-test");
        let _guard = EnvGuard("OPENPEER_IDENTITY_DIR");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(
            cfg.identity_dir,
            std::path::PathBuf::from("/tmp/openpeer-test")
        );
    }

    #[test]
    fn test_apply_env_invalid_port_ignores() {
        std::env::set_var("OPENPEER_SIGNALING_PORT", "not-a-number");
        let _guard = EnvGuard("OPENPEER_SIGNALING_PORT");
        let mut cfg = Config::default();
        let original = cfg.signaling_port;
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, original);
    }

    #[test]
    fn test_apply_env_empty_string_ignored() {
        std::env::set_var("OPENPEER_SIGNALING_PORT", "");
        let _guard = EnvGuard("OPENPEER_SIGNALING_PORT");
        let mut cfg = Config::default();
        let original = cfg.signaling_port;
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, original);
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
    fn test_signaling_addr_ipv4() {
        let cfg = Config::default();
        let addr: SocketAddr = "10.0.0.1:8080".parse().unwrap();
        let bound = cfg.signaling_addr(addr);
        assert_eq!(bound.port(), 38901);
        assert_eq!(bound.ip(), std::net::IpAddr::from([10, 0, 0, 1]));
    }

    #[test]
    fn test_signaling_addr_ipv6() {
        let cfg = Config::default();
        let addr: SocketAddr = "[::1]:8080".parse().unwrap();
        let bound = cfg.signaling_addr(addr);
        assert_eq!(bound.port(), 38901);
        assert!(bound.is_ipv6());
    }

    #[test]
    fn test_default_identity_dir_contains_openpeer() {
        let cfg = Config::default();
        assert!(cfg.identity_dir.to_string_lossy().contains(".openpeer"));
    }

    #[test]
    fn test_clone_preserves_values() {
        let cfg = Config::default();
        let cloned = cfg.clone();
        assert_eq!(cloned.signaling_port, cfg.signaling_port);
        assert_eq!(cloned.turn_port, cfg.turn_port);
        assert_eq!(cloned.log_filter, cfg.log_filter);
        assert_eq!(cloned.identity_dir, cfg.identity_dir);
    }

    #[test]
    fn test_debug_format_does_not_panic() {
        let cfg = Config::default();
        let s = format!("{:?}", cfg);
        assert!(s.contains("Config"));
        assert!(s.contains("38901"));
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

    #[test]
    fn test_from_file_invalid_toml() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let path = tmp_dir.path().join("bad.toml");
        std::fs::write(
            &path,
            "signaling_port = \"not_a_number\"\nturn_port = 38902\nlog_filter = \"info\"\nidentity_dir = \"/tmp\"\n",
        )
        .unwrap();
        let result = Config::from_file(&path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[test]
    fn test_apply_env_all_vars_simultaneously() {
        std::env::set_var("OPENPEER_SIGNALING_PORT", "11111");
        std::env::set_var("OPENPEER_TURN_PORT", "22222");
        std::env::set_var("OPENPEER_LOG", "trace");
        std::env::set_var("OPENPEER_IDENTITY_DIR", "/all/env");
        let _g1 = EnvGuard("OPENPEER_SIGNALING_PORT");
        let _g2 = EnvGuard("OPENPEER_TURN_PORT");
        let _g3 = EnvGuard("OPENPEER_LOG");
        let _g4 = EnvGuard("OPENPEER_IDENTITY_DIR");
        let mut cfg = Config::default();
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, 11111);
        assert_eq!(cfg.turn_port, 22222);
        assert_eq!(cfg.log_filter, "trace");
        assert_eq!(cfg.identity_dir, std::path::PathBuf::from("/all/env"));
    }

    #[test]
    fn test_config_clone_idempotent() {
        let cfg = Config::default();
        let c1 = cfg.clone();
        let c2 = c1.clone();
        assert_eq!(cfg.signaling_port, c2.signaling_port);
        assert_eq!(cfg.turn_port, c2.turn_port);
        assert_eq!(cfg.log_filter, c2.log_filter);
        assert_eq!(cfg.identity_dir, c2.identity_dir);
    }

    #[test]
    fn test_from_file_all_fields_loaded() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let path = tmp_dir.path().join("c.toml");
        std::fs::write(
            &path,
            "signaling_port = 12345\nturn_port = 54321\nlog_filter = \"warn\"\nidentity_dir = \"/var/opt/openpeer\"\n",
        )
        .unwrap();
        let cfg = Config::from_file(&path).unwrap();
        assert_eq!(cfg.signaling_port, 12345);
        assert_eq!(cfg.turn_port, 54321);
        assert_eq!(cfg.log_filter, "warn");
        assert_eq!(cfg.identity_dir, std::path::PathBuf::from("/var/opt/openpeer"));
    }

    #[test]
    fn test_from_file_then_env_overrides() {
        let tmp_dir = tempfile::TempDir::new().unwrap();
        let path = tmp_dir.path().join("openpeer.toml");
        std::fs::write(
            &path,
            "signaling_port = 10000\nturn_port = 38902\nlog_filter = \"info\"\nidentity_dir = \"/tmp\"\n",
        )
        .unwrap();
        std::env::set_var("OPENPEER_SIGNALING_PORT", "20000");
        let _guard = EnvGuard("OPENPEER_SIGNALING_PORT");
        let mut cfg = Config::from_file(&path).unwrap();
        cfg.apply_env();
        assert_eq!(cfg.signaling_port, 20000, "env should override file value");
    }
}
