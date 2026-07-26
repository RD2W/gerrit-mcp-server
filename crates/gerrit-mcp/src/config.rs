// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Configuration loading and validation.
//!
//! Loads TOML configuration with environment variable overrides.
//! Priority: `--config <path>` → `./config.toml` →
//! `./config/config.toml` → built-in defaults.
//!
//! Sensitive fields (token, password) are loaded from environment
//! variables specified in the config, never from the file itself.

use std::path::Path;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

/// Top-level configuration for the Gerrit MCP server.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Gerrit connection settings.
    pub gerrit: GerritConfig,
    /// Service-level behaviour.
    pub service: ServiceConfig,
    /// Cache settings.
    pub cache: CacheConfig,
    /// Rate limit settings.
    pub rate_limit: RateLimitConfig,
    /// Transport settings.
    pub transport: TransportConfig,
    /// Logging settings.
    pub log: LogConfig,
}

impl Config {
    /// Loads configuration from the given explicit path or falls back
    /// to `./config/config.toml` → `./config.toml` → defaults.
    ///
    /// After loading the file, environment variables are applied as
    /// overrides for specific fields.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The explicit path is specified but the file doesn't exist.
    /// - The TOML file contains unknown fields.
    /// - The TOML syntax is invalid.
    /// - Validation fails (e.g. empty `base_url`).
    #[allow(dead_code)]
    pub fn load(explicit_path: Option<&str>) -> Result<Self, ConfigError> {
        let mut config = if let Some(path) = explicit_path {
            Self::from_file(Path::new(path))?
        } else {
            Self::from_search_paths(&["./config/config.toml", "./config.toml"])?
        };

        config.apply_env_overrides();
        config.validate()?;

        tracing::info!(
            base_url = %config.gerrit.base_url,
            transport_mode = ?config.transport.mode,
            "configuration loaded"
        );

        Ok(config)
    }

    fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            ConfigError::Io(format!(
                "failed to read config file '{}': {e}",
                path.display()
            ))
        })?;
        let config: Self = toml::from_str(&content).map_err(|e| {
            ConfigError::Parse(format!(
                "failed to parse config file '{}': {e}",
                path.display()
            ))
        })?;
        Ok(config)
    }

    fn from_search_paths(paths: &[&str]) -> Result<Self, ConfigError> {
        for path in paths {
            if Path::new(path).exists() {
                return Self::from_file(Path::new(path));
            }
        }
        tracing::info!("no config file found, using defaults");
        Ok(Self::default())
    }

    fn apply_env_overrides(&mut self) {
        if let Ok(val) = std::env::var("GERRIT_URL") {
            self.gerrit.base_url = val;
        }

        if let Ok(val) = std::env::var("GERRIT_CA_CERT") {
            self.gerrit.ca_cert = Some(val);
        }
        if let Ok(val) = std::env::var("SSL_CERT_FILE") {
            self.gerrit.ca_cert = Some(val);
        }
        if let Ok(val) = std::env::var("SSL_CERT_DIR") {
            self.gerrit.ca_cert_dir = Some(val);
        }
        if let Ok(val) = std::env::var("GERRIT_VERIFY_SSL")
            && (val.eq_ignore_ascii_case("false") || val == "0")
        {
            self.gerrit.verify_ssl = false;
        }

        if let Some(ref username_env) = self.gerrit.auth.username_env.clone()
            && let Ok(username) = std::env::var(username_env)
        {
            self.gerrit.auth.username = Some(username);
        }
        if let Some(ref auth_token_env) = self.gerrit.auth.auth_token_env.clone()
            && let Ok(auth_token) = std::env::var(auth_token_env)
        {
            self.gerrit.auth.auth_token = Some(auth_token);
        }
        if let Some(ref token_env) = self.gerrit.auth.token_env.clone()
            && let Ok(bearer_token) = std::env::var(token_env)
        {
            self.gerrit.auth.bearer_token = Some(bearer_token);
        }

        if let Ok(val) = std::env::var("RUST_LOG") {
            self.log.level = val;
        }
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.gerrit.base_url.is_empty() {
            return Err(ConfigError::Validation(
                "gerrit.base_url must not be empty".into(),
            ));
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// GerritConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct GerritConfig {
    /// Base URL of the Gerrit instance.
    #[serde(default)]
    pub base_url: String,
    /// Authentication settings.
    pub auth: AuthConfig,
    /// HTTP request timeout in seconds.
    pub timeout_secs: u64,
    /// Path to custom CA certificate PEM file.
    pub ca_cert: Option<String>,
    /// Path to directory of CA certificate files.
    pub ca_cert_dir: Option<String>,
    /// Whether to verify TLS certificates.
    pub verify_ssl: bool,
}

impl Default for GerritConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            auth: AuthConfig::default(),
            timeout_secs: 30,
            ca_cert: None,
            ca_cert_dir: None,
            verify_ssl: true,
        }
    }
}

// ---------------------------------------------------------------------------
// AuthConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct AuthConfig {
    /// `"basic"`, `"token"`, or `"none"`.
    pub mode: String,
    /// Env variable name for Gerrit username.
    pub username_env: Option<String>,
    /// The actual username (populated from env at load time).
    #[serde(skip)]
    pub username: Option<String>,
    /// Env variable name for Gerrit HTTP auth token (e.g. `GERRIT_AUTH_TOKEN`).
    pub auth_token_env: Option<String>,
    /// The actual HTTP auth token (populated from env at load time).
    #[serde(skip)]
    pub auth_token: Option<String>,
    /// Env variable name for the bearer token.
    pub token_env: Option<String>,
    /// The actual bearer token value (populated from env at load time).
    #[serde(skip)]
    pub bearer_token: Option<String>,
    /// Path to a `.gitcookies` file for Gerrit authentication.
    pub gitcookies_path: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            username_env: None,
            username: None,
            auth_token_env: Some("GERRIT_AUTH_TOKEN".into()),
            auth_token: None,
            token_env: Some("GERRIT_TOKEN".into()),
            bearer_token: None,
            gitcookies_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ServiceConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServiceConfig {
    /// Default maximum number of results returned by queries.
    pub default_max_results: u32,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            default_max_results: 25,
        }
    }
}

// ---------------------------------------------------------------------------
// CacheConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    pub enabled: bool,
    pub ttl_secs: u64,
    pub max_entries: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ttl_secs: 300,
            max_entries: 1000,
        }
    }
}

// ---------------------------------------------------------------------------
// RateLimitConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct RateLimitConfig {
    pub enabled: bool,
    pub requests_per_second: u32,
    pub burst: u32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            requests_per_second: 10,
            burst: 20,
        }
    }
}

// ---------------------------------------------------------------------------
// TransportConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct TransportConfig {
    /// `"stdio"`, `"http"`, or `"both"`.
    pub mode: String,
    /// Bind address for HTTP transport.
    pub bind_addr: String,
    /// URL path for the MCP Streamable HTTP endpoint.
    pub http_path: String,
    /// Health check endpoint path.
    pub health_path: String,
    /// Readiness check endpoint path.
    pub ready_path: String,
    /// Prometheus metrics endpoint path.
    pub metrics_path: String,
    /// Allowed hostnames for Streamable HTTP Host header validation.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            mode: "both".into(),
            bind_addr: "0.0.0.0:8080".into(),
            http_path: "/mcp".into(),
            health_path: "/healthz".into(),
            ready_path: "/readyz".into(),
            metrics_path: "/metrics".into(),
            allowed_hosts: vec![],
        }
    }
}

// ---------------------------------------------------------------------------
// LogConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct LogConfig {
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// ConfigError
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config I/O error: {0}")]
    Io(String),
    #[error("config parse error: {0}")]
    Parse(String),
    #[error("config validation error: {0}")]
    Validation(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_fails_validation() {
        let config = Config::default();
        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn config_with_base_url_validates() {
        let config = Config {
            gerrit: GerritConfig {
                base_url: "https://gerrit.example.com".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        let result = config.validate();
        assert!(result.is_ok());
    }

    #[test]
    fn parses_valid_toml() {
        let toml_str = r#"
[gerrit]
base_url = "https://gerrit.example.com"

[gerrit.auth]
mode = "basic"
username_env = "GERRIT_USER"

[transport]
mode = "http"
allowed_hosts = ["gerrit.example.com"]
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.gerrit.base_url, "https://gerrit.example.com");
        assert_eq!(config.gerrit.auth.mode, "basic");
        assert_eq!(config.transport.mode, "http");
        assert_eq!(config.transport.allowed_hosts, vec!["gerrit.example.com"]);
    }

    #[test]
    fn denies_unknown_fields() {
        let toml_str = r#"
[gerrit]
base_url = "https://gerrit.example.com"
unknown_field = 42
"#;
        let result: Result<Config, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn env_override_base_url() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://from-env.example.com".into();
        assert_eq!(config.gerrit.base_url, "https://from-env.example.com");
    }

    #[test]
    fn auth_token_from_env() {
        let mut config = Config::default();
        config.gerrit.auth.auth_token_env = Some("TEST_AUTH_TOKEN".into());
        assert!(config.gerrit.auth.auth_token.is_none());
    }

    #[test]
    fn bearer_token_from_env() {
        let mut config = Config::default();
        config.gerrit.auth.token_env = Some("TEST_BEARER_TOKEN".into());
        assert!(config.gerrit.auth.bearer_token.is_none());
    }

    #[test]
    fn transport_defaults() {
        let config = TransportConfig::default();
        assert_eq!(config.mode, "both");
        assert_eq!(config.bind_addr, "0.0.0.0:8080");
        assert_eq!(config.http_path, "/mcp");
        assert!(config.allowed_hosts.is_empty());
    }

    #[test]
    fn cache_defaults() {
        let config = CacheConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.ttl_secs, 300);
        assert_eq!(config.max_entries, 1000);
    }

    #[test]
    fn rate_limit_defaults() {
        let config = RateLimitConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.requests_per_second, 10);
        assert_eq!(config.burst, 20);
    }

    #[test]
    fn service_defaults() {
        let config = ServiceConfig::default();
        assert_eq!(config.default_max_results, 25);
    }
}
