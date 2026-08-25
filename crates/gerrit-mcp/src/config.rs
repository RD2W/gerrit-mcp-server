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
        // --- gerrit ---
        if let Ok(val) = std::env::var("GERRIT_URL") {
            self.gerrit.base_url = val;
        }

        if let Ok(val) = std::env::var("GERRIT_TIMEOUT_SECS")
            && let Ok(v) = val.parse::<u64>()
        {
            self.gerrit.timeout_secs = v;
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

        apply_bool_env("GERRIT_VERIFY_SSL", &mut self.gerrit.verify_ssl);

        // --- gerrit.auth ---
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

        // --- service ---
        if let Ok(val) = std::env::var("MCP_DEFAULT_MAX_RESULTS")
            && let Ok(v) = val.parse::<u32>()
        {
            self.service.default_max_results = v;
        }
        apply_bool_env("READ_ONLY_MODE", &mut self.service.read_only);

        // --- cache ---
        apply_bool_env("MCP_CACHE_ENABLED", &mut self.cache.enabled);
        if let Ok(val) = std::env::var("MCP_CACHE_TTL_SECS")
            && let Ok(v) = val.parse::<u64>()
        {
            self.cache.ttl_secs = v;
        }
        if let Ok(val) = std::env::var("MCP_CACHE_MAX_ENTRIES")
            && let Ok(v) = val.parse::<usize>()
        {
            self.cache.max_entries = v;
        }

        // --- rate_limit ---
        apply_bool_env("MCP_RATE_LIMIT_ENABLED", &mut self.rate_limit.enabled);
        if let Ok(val) = std::env::var("MCP_RATE_LIMIT_RPS")
            && let Ok(v) = val.parse::<u32>()
        {
            self.rate_limit.requests_per_second = v;
        }
        if let Ok(val) = std::env::var("MCP_RATE_LIMIT_BURST")
            && let Ok(v) = val.parse::<u32>()
        {
            self.rate_limit.burst = v;
        }

        // --- transport ---
        if let Ok(val) = std::env::var("MCP_TRANSPORT") {
            self.transport.mode = val;
        }
        if let Ok(val) = std::env::var("MCP_BIND_ADDR") {
            self.transport.bind_addr = val;
        }
        if let Ok(val) = std::env::var("MCP_HTTP_PATH") {
            self.transport.http_path = val;
        }
        if let Ok(val) = std::env::var("MCP_HEALTH_PATH") {
            self.transport.health_path = val;
        }
        if let Ok(val) = std::env::var("MCP_READY_PATH") {
            self.transport.ready_path = val;
        }
        if let Ok(val) = std::env::var("MCP_METRICS_PATH") {
            self.transport.metrics_path = val;
        }
        if let Ok(val) = std::env::var("MCP_ALLOWED_HOSTS") {
            self.transport.allowed_hosts = val
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        if let Ok(val) = std::env::var("MCP_AUTH_TOKEN") {
            self.transport.mcp_auth_token = val;
        }

        // --- log ---
        if let Ok(val) = std::env::var("RUST_LOG") {
            self.log.level = val;
        }
        if let Ok(val) = std::env::var("MCP_LOG_LEVEL") {
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
    /// Disable all write operations (create, update, delete).
    pub read_only: bool,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            default_max_results: 25,
            read_only: false,
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
    /// Bearer token for MCP endpoint authentication. When set, clients
    /// must include `Authorization: Bearer <token>` in requests.
    /// Token auth is disabled when this is empty.
    #[serde(default)]
    pub mcp_auth_token: String,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            mode: "both".into(),
            bind_addr: "127.0.0.1:8080".into(),
            http_path: "/mcp".into(),
            health_path: "/healthz".into(),
            ready_path: "/readyz".into(),
            metrics_path: "/metrics".into(),
            allowed_hosts: vec![],
            mcp_auth_token: String::new(),
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
// Helpers
// ---------------------------------------------------------------------------

/// Applies an env var to a boolean field.
///
/// Accepts `true` / `1` / `yes` (case-insensitive) for true and
/// `false` / `0` / `no` for false. Unknown values are ignored.
fn apply_bool_env(var: &str, target: &mut bool) {
    if let Ok(val) = std::env::var(var) {
        match val.to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" => *target = true,
            "false" | "0" | "no" => *target = false,
            _ => {} // unknown value — keep default
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
    use std::cell::Cell;
    use std::sync::Mutex;

    // --- helpers ---

    /// Serializes access to process environment variables between tests.
    /// Tests run on parallel threads, and env-based tests must not observe
    /// each other's temporary values.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    thread_local! {
        /// True while this thread holds `ENV_LOCK` — makes nested `with_env`
        /// calls (e.g. `mcp_log_level_wins_over_rust_log`) re-entrant instead
        /// of deadlocking on the non-reentrant mutex.
        static ENV_LOCK_HELD: Cell<bool> = const { Cell::new(false) };
    }

    /// Helper that sets an env var for the duration of the closure.
    fn with_env<R>(k: &str, v: &str, f: impl FnOnce() -> R) -> R {
        let nested = ENV_LOCK_HELD.with(Cell::get);
        // The guard must outlive the closure — declaring it inside the `if`
        // would drop the lock immediately and un-serialize this section.
        let _guard = if nested {
            None
        } else {
            let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            ENV_LOCK_HELD.set(true);
            Some(guard)
        };
        // SAFETY: serialized by ENV_LOCK (or already held by this thread)
        unsafe { std::env::set_var(k, v) };
        let result = f();
        // SAFETY: cleanup, serialized by ENV_LOCK
        unsafe { std::env::remove_var(k) };
        if !nested {
            ENV_LOCK_HELD.set(false);
        }
        result
    }

    /// Like `with_env` but also temporarily clears `SSL_CERT_FILE` and
    /// `SSL_CERT_DIR` to avoid interference from system environment.
    fn with_clean_tls_env<R>(k: &str, v: &str, f: impl FnOnce() -> R) -> R {
        let nested = ENV_LOCK_HELD.with(Cell::get);
        let _guard = if nested {
            None
        } else {
            let guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            ENV_LOCK_HELD.set(true);
            Some(guard)
        };
        // SAFETY: serialized by ENV_LOCK
        unsafe { std::env::remove_var("SSL_CERT_FILE") };
        unsafe { std::env::remove_var("SSL_CERT_DIR") };
        // SAFETY: serialized by ENV_LOCK
        unsafe { std::env::set_var(k, v) };
        let result = f();
        // SAFETY: cleanup
        unsafe { std::env::remove_var(k) };
        if !nested {
            ENV_LOCK_HELD.set(false);
        }
        result
    }

    // --- validation ---

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

    // --- TOML parsing ---

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

    // --- env overrides: gerrit ---

    #[test]
    fn env_override_gerrit_url() {
        with_env("GERRIT_URL", "https://gerrit-env.example.com", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.base_url, "https://gerrit-env.example.com");
        });
    }

    #[test]
    fn env_override_gerrit_timeout_secs() {
        with_env("GERRIT_TIMEOUT_SECS", "60", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.timeout_secs, 60);
        });
    }

    #[test]
    fn env_override_gerrit_timeout_ignores_invalid() {
        with_env("GERRIT_TIMEOUT_SECS", "not_a_number", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.timeout_secs, 30); // default preserved
        });
    }

    #[test]
    fn env_override_gerrit_ca_cert() {
        with_clean_tls_env("GERRIT_CA_CERT", "/custom/ca.pem", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.ca_cert.as_deref(), Some("/custom/ca.pem"));
        });
    }

    #[test]
    fn env_override_ssl_cert_file_falls_back_to_ca_cert() {
        with_clean_tls_env("SSL_CERT_FILE", "/fallback.pem", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.ca_cert.as_deref(), Some("/fallback.pem"));
        });
    }

    #[test]
    fn env_override_ssl_cert_dir() {
        with_env("SSL_CERT_DIR", "/my/certs", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.gerrit.ca_cert_dir.as_deref(), Some("/my/certs"));
        });
    }

    #[test]
    fn env_override_verify_ssl_true() {
        with_env("GERRIT_VERIFY_SSL", "true", || {
            let mut config = Config::default();
            config.gerrit.verify_ssl = false;
            config.apply_env_overrides();
            assert!(config.gerrit.verify_ssl);
        });
    }

    #[test]
    fn env_override_verify_ssl_false() {
        with_env("GERRIT_VERIFY_SSL", "false", || {
            let mut config = Config::default();
            config.gerrit.verify_ssl = true;
            config.apply_env_overrides();
            assert!(!config.gerrit.verify_ssl);
        });
    }

    #[test]
    fn env_override_verify_ssl_0() {
        with_env("GERRIT_VERIFY_SSL", "0", || {
            let mut config = Config::default();
            config.gerrit.verify_ssl = true;
            config.apply_env_overrides();
            assert!(!config.gerrit.verify_ssl);
        });
    }

    // --- env overrides: auth (indirection) ---

    #[test]
    fn auth_username_from_env() {
        with_env("TEST_USER_VAR", "gerrit_admin", || {
            let mut config = Config::default();
            config.gerrit.auth.username_env = Some("TEST_USER_VAR".into());
            config.apply_env_overrides();
            assert_eq!(config.gerrit.auth.username.as_deref(), Some("gerrit_admin"));
        });
    }

    #[test]
    fn auth_token_from_env() {
        with_env("TEST_AUTH_TOKEN", "secret123", || {
            let mut config = Config::default();
            config.gerrit.auth.auth_token_env = Some("TEST_AUTH_TOKEN".into());
            config.apply_env_overrides();
            assert_eq!(config.gerrit.auth.auth_token.as_deref(), Some("secret123"));
        });
    }

    #[test]
    fn bearer_token_from_env() {
        with_env("TEST_BEARER_TOKEN", "bearer_secret", || {
            let mut config = Config::default();
            config.gerrit.auth.token_env = Some("TEST_BEARER_TOKEN".into());
            config.apply_env_overrides();
            assert_eq!(
                config.gerrit.auth.bearer_token.as_deref(),
                Some("bearer_secret")
            );
        });
    }

    // --- env overrides: service ---

    #[test]
    fn env_override_default_max_results() {
        with_env("MCP_DEFAULT_MAX_RESULTS", "50", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.service.default_max_results, 50);
        });
    }

    // --- env overrides: cache ---

    #[test]
    fn env_override_cache_enabled() {
        with_env("MCP_CACHE_ENABLED", "1", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert!(config.cache.enabled);
        });
    }

    #[test]
    fn env_override_cache_ttl() {
        with_env("MCP_CACHE_TTL_SECS", "600", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.cache.ttl_secs, 600);
        });
    }

    #[test]
    fn env_override_cache_max_entries() {
        with_env("MCP_CACHE_MAX_ENTRIES", "500", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.cache.max_entries, 500);
        });
    }

    // --- env overrides: rate_limit ---

    #[test]
    fn env_override_rate_limit_enabled() {
        with_env("MCP_RATE_LIMIT_ENABLED", "yes", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert!(config.rate_limit.enabled);
        });
    }

    #[test]
    fn env_override_rate_limit_rps() {
        with_env("MCP_RATE_LIMIT_RPS", "15", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.rate_limit.requests_per_second, 15);
        });
    }

    #[test]
    fn env_override_rate_limit_burst() {
        with_env("MCP_RATE_LIMIT_BURST", "30", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.rate_limit.burst, 30);
        });
    }

    // --- env overrides: transport ---

    #[test]
    fn env_override_transport_mode() {
        with_env("MCP_TRANSPORT", "http", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.mode, "http");
        });
    }

    #[test]
    fn env_override_bind_addr() {
        with_env("MCP_BIND_ADDR", "0.0.0.0:9090", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.bind_addr, "0.0.0.0:9090");
        });
    }

    #[test]
    fn env_override_http_path() {
        with_env("MCP_HTTP_PATH", "/custom-mcp", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.http_path, "/custom-mcp");
        });
    }

    #[test]
    fn env_override_health_path() {
        with_env("MCP_HEALTH_PATH", "/health", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.health_path, "/health");
        });
    }

    #[test]
    fn env_override_ready_path() {
        with_env("MCP_READY_PATH", "/ready", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.ready_path, "/ready");
        });
    }

    #[test]
    fn env_override_metrics_path() {
        with_env("MCP_METRICS_PATH", "/mymetrics", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.metrics_path, "/mymetrics");
        });
    }

    #[test]
    fn env_override_allowed_hosts() {
        with_env("MCP_ALLOWED_HOSTS", "  host1 , host2 , , host3  ", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(
                config.transport.allowed_hosts,
                vec!["host1", "host2", "host3"]
            );
        });
    }

    #[test]
    fn env_override_mcp_auth_token() {
        with_env("MCP_AUTH_TOKEN", "secret-mcp-token", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.transport.mcp_auth_token, "secret-mcp-token");
        });
    }

    // --- env overrides: log ---

    #[test]
    fn env_override_log_level_via_rust_log() {
        with_env("RUST_LOG", "debug", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.log.level, "debug");
        });
    }

    #[test]
    fn env_override_log_level_via_mcp_log_level() {
        with_env("MCP_LOG_LEVEL", "trace", || {
            let mut config = Config::default();
            config.apply_env_overrides();
            assert_eq!(config.log.level, "trace");
        });
    }

    #[test]
    fn mcp_log_level_wins_over_rust_log() {
        with_env("RUST_LOG", "info", || {
            with_env("MCP_LOG_LEVEL", "warn", || {
                let mut config = Config::default();
                config.apply_env_overrides();
                assert_eq!(config.log.level, "warn");
            });
        });
    }

    // --- apply_bool_env ---

    #[test]
    fn bool_env_true_variants() {
        for v in &["true", "True", "TRUE", "1", "yes", "YES"] {
            let mut flag = false;
            with_env("TEST_BOOL", v, || {
                apply_bool_env("TEST_BOOL", &mut flag);
            });
            assert!(flag, "expected true for '{v}'");
        }
    }

    #[test]
    fn bool_env_false_variants() {
        for v in &["false", "False", "FALSE", "0", "no", "NO"] {
            let mut flag = true;
            with_env("TEST_BOOL", v, || {
                apply_bool_env("TEST_BOOL", &mut flag);
            });
            assert!(!flag, "expected false for '{v}'");
        }
    }

    #[test]
    fn bool_env_unknown_value_preserves_default() {
        let mut flag = true;
        with_env("TEST_BOOL", "garbage", || {
            apply_bool_env("TEST_BOOL", &mut flag);
        });
        assert!(flag, "garbage value should preserve existing value");
    }

    // --- defaults ---

    #[test]
    fn transport_defaults() {
        let config = TransportConfig::default();
        assert_eq!(config.mode, "both");
        assert_eq!(config.bind_addr, "127.0.0.1:8080");
        assert_eq!(config.http_path, "/mcp");
        assert!(config.allowed_hosts.is_empty());
        assert!(config.mcp_auth_token.is_empty());
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
        assert!(!config.read_only);
    }
}
