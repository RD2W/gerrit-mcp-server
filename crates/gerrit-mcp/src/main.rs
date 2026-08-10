// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Gerrit MCP server — entry point.
//!
//! Loads configuration, initializes tracing, builds the Gerrit client,
//! and starts the selected transport (stdio / HTTP / both).

mod config;
mod health;
mod mcp;
mod transport;

use std::path::PathBuf;
use std::time::Duration;

use clap::Parser;
use gerrit_core::application::GerritService;
use gerrit_core::infrastructure::auth::AuthMode;
use gerrit_core::infrastructure::client::{GerritClient, GerritClientConfig};
use gerrit_core::infrastructure::tls::TlsConfig;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::mcp::GerritServer;
use crate::transport::run_transport;

/// Custom version string with full build metadata.
const VERSION_TEXT: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    "\n",
    "author:  ",
    env!("CARGO_PKG_AUTHORS"),
    "\n",
    "commit:  ",
    env!("GIT_HASH"),
    "\n",
    "built:   ",
    env!("BUILD_DATE"),
    "\n",
    "target:  ",
    env!("BUILD_TARGET"),
);

#[derive(Parser)]
#[command(name = env!("CARGO_PKG_NAME"))]
#[command(version = VERSION_TEXT)]
#[command(about = env!("CARGO_PKG_DESCRIPTION"), author = env!("CARGO_PKG_AUTHORS"))]
struct Args {
    /// Path to configuration file
    #[arg(short = 'c', long = "config", default_value = "config/config.toml")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config = Config::load(args.config.to_str())?;

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&config.log.level)),
        )
        .init();

    tracing::info!(version = env!("CARGO_PKG_VERSION"), "gerrit-mcp starting");

    let auth = resolve_auth(&config)?;

    let client_config = GerritClientConfig {
        base_url: config.gerrit.base_url.clone(),
        auth,
        timeout: Duration::from_secs(config.gerrit.timeout_secs),
        tls: TlsConfig {
            verify_ssl: config.gerrit.verify_ssl,
            ca_cert: config.gerrit.ca_cert.clone(),
            ca_cert_dir: config.gerrit.ca_cert_dir.clone(),
        },
        disable_url_normalization: false,
    };

    let client = GerritClient::new(client_config.clone())?;

    let mut service = GerritService::new(client);

    if config.cache.enabled {
        service = service.with_cache(
            Duration::from_secs(config.cache.ttl_secs),
            config.cache.max_entries,
        );
    }

    if config.rate_limit.enabled {
        service = service.with_rate_limit(
            config.rate_limit.requests_per_second,
            config.rate_limit.burst,
        );
    }

    let server = GerritServer::new(service)
        .with_client_factory(client_config)
        .with_read_only(config.service.read_only);

    run_transport(&config, server).await?;

    Ok(())
}

fn resolve_auth(config: &Config) -> anyhow::Result<AuthMode> {
    match config.gerrit.auth.mode.as_str() {
        "http_basic" | "basic" => {
            let username = config.gerrit.auth.username.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "http_basic auth mode requires username (set via GERRIT_USER env or config)"
                )
            })?;
            let password = config.gerrit.auth.auth_token.clone().unwrap_or_default();
            Ok(AuthMode::HttpBasic { username, password })
        }
        "git_cookies" => {
            let path = config.gerrit.auth.gitcookies_path.clone().ok_or_else(|| {
                anyhow::anyhow!("git_cookies auth mode requires gitcookies_path in config")
            })?;
            Ok(AuthMode::GitCookies {
                gitcookies_path: PathBuf::from(path),
            })
        }
        "bearer" | "token" => {
            let token = config.gerrit.auth.bearer_token.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "bearer auth mode requires {} env var to be set",
                    config
                        .gerrit
                        .auth
                        .token_env
                        .as_deref()
                        .unwrap_or("GERRIT_TOKEN")
                )
            })?;
            Ok(AuthMode::Bearer(token))
        }
        "none" => {
            tracing::warn!("no authentication configured; requests will be anonymous");
            Ok(AuthMode::Bearer(String::new()))
        }
        other => anyhow::bail!(
            "unknown auth mode: '{}' (expected http_basic, git_cookies, bearer, or none)",
            other
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn parse_default_config() {
        let args = Args::parse_from(["gerrit-mcp"]);
        assert_eq!(args.config, PathBuf::from("config/config.toml"));
    }

    #[test]
    fn parse_custom_config_long() {
        let args = Args::parse_from(["gerrit-mcp", "--config", "/etc/gerrit.toml"]);
        assert_eq!(args.config, PathBuf::from("/etc/gerrit.toml"));
    }

    #[test]
    fn parse_custom_config_short() {
        let args = Args::parse_from(["gerrit-mcp", "-c", "/etc/gerrit.toml"]);
        assert_eq!(args.config, PathBuf::from("/etc/gerrit.toml"));
    }

    #[test]
    fn parse_custom_config_equals() {
        let args = Args::parse_from(["gerrit-mcp", "--config=/etc/gerrit.toml"]);
        assert_eq!(args.config, PathBuf::from("/etc/gerrit.toml"));
    }

    #[test]
    fn version_text_contains_expected_fields() {
        assert!(VERSION_TEXT.contains("author:"), "missing author field");
        assert!(VERSION_TEXT.contains("commit:"), "missing commit field");
        assert!(VERSION_TEXT.contains("built:"), "missing built field");
        assert!(VERSION_TEXT.contains("target:"), "missing target field");
    }

    #[test]
    fn version_text_has_non_empty_hash() {
        let commit_line = VERSION_TEXT
            .lines()
            .find(|l| l.starts_with("commit:"))
            .expect("commit line not found");
        let hash = commit_line.trim_start_matches("commit:").trim();
        assert!(!hash.is_empty(), "commit hash should not be empty");
    }

    #[test]
    fn version_text_build_date_is_iso8601() {
        let date_line = VERSION_TEXT
            .lines()
            .find(|l| l.starts_with("built:"))
            .expect("built line not found");
        let date = date_line.trim_start_matches("built:").trim();
        assert!(date.contains('T'), "missing T separator in ISO 8601 date");
        assert!(date.ends_with('Z'), "missing Z suffix in ISO 8601 date");
        assert_eq!(
            date.len(),
            20,
            "expected ISO 8601 length (YYYY-MM-DDTHH:MM:SSZ)"
        );
    }

    #[test]
    fn version_text_target_is_not_empty() {
        let target_line = VERSION_TEXT
            .lines()
            .find(|l| l.starts_with("target:"))
            .expect("target line not found");
        let target = target_line.trim_start_matches("target:").trim();
        assert!(!target.is_empty(), "target should not be empty");
        assert!(target.contains('-'), "target triple should contain hyphens");
    }

    #[test]
    fn resolve_auth_http_basic_with_username() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "http_basic".into();
        config.gerrit.auth.username = Some("admin".into());
        let auth = resolve_auth(&config).unwrap();
        assert!(matches!(auth, AuthMode::HttpBasic { .. }));
    }

    #[test]
    fn resolve_auth_bearer_token() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "bearer".into();
        config.gerrit.auth.bearer_token = Some("tok_123".into());
        let auth = resolve_auth(&config).unwrap();
        assert_eq!(auth, AuthMode::Bearer("tok_123".into()));
    }

    #[test]
    fn resolve_auth_basic_alias() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "basic".into();
        config.gerrit.auth.username = Some("admin".into());
        let auth = resolve_auth(&config).unwrap();
        assert!(matches!(auth, AuthMode::HttpBasic { .. }));
    }

    #[test]
    fn resolve_auth_token_alias() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "token".into();
        config.gerrit.auth.bearer_token = Some("tok_456".into());
        let auth = resolve_auth(&config).unwrap();
        assert_eq!(auth, AuthMode::Bearer("tok_456".into()));
    }

    #[test]
    fn resolve_auth_none() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "none".into();
        let auth = resolve_auth(&config).unwrap();
        assert!(matches!(auth, AuthMode::Bearer(s) if s.is_empty()));
    }

    #[test]
    fn resolve_auth_git_cookies() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "git_cookies".into();
        config.gerrit.auth.gitcookies_path = Some("/tmp/cookies".into());
        let auth = resolve_auth(&config).unwrap();
        assert!(matches!(auth, AuthMode::GitCookies { .. }));
    }

    #[test]
    fn resolve_auth_unknown_mode() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "saml".into();
        let err = resolve_auth(&config).unwrap_err();
        assert!(err.to_string().contains("unknown auth mode"));
    }

    #[test]
    fn resolve_auth_http_basic_missing_username() {
        let mut config = Config::default();
        config.gerrit.base_url = "https://gerrit.example.com".into();
        config.gerrit.auth.mode = "http_basic".into();
        let err = resolve_auth(&config).unwrap_err();
        assert!(err.to_string().contains("username"));
    }
}
