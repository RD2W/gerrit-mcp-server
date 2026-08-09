// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Authentication configuration for Gerrit API.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::domain::DomainError;

/// Supported authentication methods for Gerrit.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum AuthMode {
    HttpBasic { username: String, password: String },
    GitCookies { gitcookies_path: PathBuf },
    Bearer(String),
}

impl std::fmt::Debug for AuthMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HttpBasic { username, .. } => f
                .debug_struct("HttpBasic")
                .field("username", username)
                .field("password", &"[redacted]")
                .finish(),
            Self::Bearer(_) => f.write_str("Bearer([redacted])"),
            Self::GitCookies { gitcookies_path } => f
                .debug_tuple("GitCookies")
                .field(gitcookies_path)
                .finish(),
        }
    }
}

/// A host specification used to match a Gerrit URL against an auth mode.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct HostConfig {
    pub host: String,
}

impl HostConfig {
    pub fn new(host: impl Into<String>) -> Self {
        Self { host: host.into() }
    }
}

/// Parse a Netscape-format cookies file, returning `name=value` for the first
/// line whose domain column contains the given `domain` substring.
pub fn parse_gitcookies(path: &Path, domain: &str) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line.ok()?;
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() >= 7 && parts[0].contains(domain) {
            return Some(format!("{}={}", parts[5], parts[6]));
        }
    }
    None
}

/// Normalize a Gerrit base URL for authenticated requests.
///
/// - Forces HTTPS (replaces `http://` with `https://`).
/// - Strips any trailing slash.
/// - Appends `/a` for `HttpBasic` and `GitCookies` auth modes.
pub fn normalize_url_for_auth(url: String, auth: &AuthMode) -> String {
    let url = url.trim_end_matches('/');
    let url = if url.starts_with("http://") {
        url.replacen("http://", "https://", 1)
    } else {
        url.to_string()
    };
    match auth {
        AuthMode::HttpBasic { .. } | AuthMode::GitCookies { .. } => {
            format!("{}/a", url)
        }
        AuthMode::Bearer(_) => url.to_string(),
    }
}

/// Apply authentication headers to a `reqwest::RequestBuilder`.
///
/// For `GitCookies` the `base_url` is used to extract the domain so that the
/// matching cookie line can be found in the gitcookies file.
pub fn apply_auth(
    builder: reqwest::RequestBuilder,
    base_url: &str,
    auth: &AuthMode,
) -> reqwest::RequestBuilder {
    match auth {
        AuthMode::HttpBasic { username, password } => builder.basic_auth(username, Some(password)),
        AuthMode::Bearer(token) => builder.bearer_auth(token),
        AuthMode::GitCookies { gitcookies_path } => {
            let domain = url::Url::parse(base_url)
                .ok()
                .and_then(|u| u.host_str().map(|h| h.to_string()));
            if let Some(domain) = domain
                && let Some(cookie) = parse_gitcookies(gitcookies_path, &domain)
            {
                return builder.header("Cookie", cookie);
            }
            builder
        }
    }
}

/// Maps hostnames to their configured `AuthMode`.
pub struct AuthManager {
    configs: BTreeMap<HostConfig, AuthMode>,
}

impl AuthManager {
    pub fn new(configs: BTreeMap<HostConfig, AuthMode>) -> Self {
        Self { configs }
    }

    /// Look up the `AuthMode` for the host extracted from `base_url`.
    pub fn get_auth(&self, base_url: &str) -> Result<AuthMode, DomainError> {
        let url = url::Url::parse(base_url)
            .map_err(|e| DomainError::Auth(format!("invalid URL for auth: {}", e)))?;
        let host = url
            .host_str()
            .ok_or_else(|| DomainError::Auth(format!("no host in URL: {}", base_url)))?;

        for (config, auth) in &self.configs {
            if config.host == host {
                return Ok(auth.clone());
            }
        }

        Err(DomainError::Auth(format!(
            "no auth configured for host: {}",
            host
        )))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp_gitcookies(content: &str) -> tempfile::TempPath {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.into_temp_path()
    }

    // --- parse_gitcookies --------------------------------------------------

    #[test]
    fn test_parse_gitcookies() {
        let path = write_temp_gitcookies(
            "# Netscape HTTP Cookie File\n\
             gerrit.example.com\tTRUE\t/\tFALSE\t0\tmycookie\tsecret123\n",
        );

        let cookie = parse_gitcookies(&path, "gerrit.example.com").unwrap();
        assert_eq!(cookie, "mycookie=secret123");
    }

    #[test]
    fn test_gitcookies_no_match() {
        let path = write_temp_gitcookies(
            "# Netscape HTTP Cookie File\n\
             gerrit.example.com\tTRUE\t/\tFALSE\t0\tmycookie\tsecret123\n",
        );

        let cookie = parse_gitcookies(&path, "other.example.com");
        assert!(cookie.is_none());
    }

    // --- normalize_url_for_auth --------------------------------------------

    #[test]
    fn test_normalize_url_appends_a_for_basic() {
        let auth = AuthMode::HttpBasic {
            username: "u".into(),
            password: "p".into(),
        };
        let result = normalize_url_for_auth("http://gerrit.example.com/".into(), &auth);
        assert_eq!(result, "https://gerrit.example.com/a");
    }

    #[test]
    fn test_normalize_url_appends_a_for_gitcookies() {
        let auth = AuthMode::GitCookies {
            gitcookies_path: "/tmp/f".into(),
        };
        let result = normalize_url_for_auth("https://gerrit.example.com/".into(), &auth);
        assert_eq!(result, "https://gerrit.example.com/a");
    }

    #[test]
    fn test_normalize_url_no_a_for_bearer() {
        let auth = AuthMode::Bearer("token".into());
        let result = normalize_url_for_auth("http://gerrit.example.com/".into(), &auth);
        assert_eq!(result, "https://gerrit.example.com");
    }

    #[test]
    fn test_normalize_url_strips_trailing_slash() {
        let auth = AuthMode::Bearer("token".into());
        let result = normalize_url_for_auth("https://gerrit.example.com////".into(), &auth);
        assert_eq!(result, "https://gerrit.example.com");
    }

    #[test]
    fn test_normalize_url_already_https() {
        let auth = AuthMode::HttpBasic {
            username: "u".into(),
            password: "p".into(),
        };
        let result = normalize_url_for_auth("https://gerrit.example.com".into(), &auth);
        assert_eq!(result, "https://gerrit.example.com/a");
    }

    // --- AuthManager -------------------------------------------------------

    #[test]
    fn test_auth_manager_returns_matching_auth() {
        let mut configs = BTreeMap::new();
        configs.insert(
            HostConfig::new("gerrit.example.com"),
            AuthMode::Bearer("my-token".into()),
        );
        let manager = AuthManager::new(configs);
        let auth = manager
            .get_auth("https://gerrit.example.com/projects/")
            .unwrap();
        assert_eq!(auth, AuthMode::Bearer("my-token".into()));
    }

    #[test]
    fn test_auth_manager_unknown_host() {
        let manager = AuthManager::new(BTreeMap::new());
        let err = manager.get_auth("https://gerrit.example.com/").unwrap_err();
        assert!(err.to_string().contains("no auth configured for host"));
    }
}
