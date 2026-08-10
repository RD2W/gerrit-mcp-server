// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Domain error types for the Gerrit core library.

use thiserror::Error;

/// Top-level error type for the Gerrit core library.
#[derive(Error, Debug, Clone)]
pub enum DomainError {
    #[error("query string must not be empty")]
    EmptyQuery,

    #[error("invalid change ID format")]
    InvalidChangeId,

    #[error("HTTP {status}: {body}")]
    HttpStatus { status: u16, body: String },

    #[error("network error: {0}")]
    Network(String),

    #[error("JSON decode error: {0}")]
    Decode(String),

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("authentication error: {0}")]
    Auth(String),

    #[error(transparent)]
    Cache(#[from] CacheError),

    #[error(transparent)]
    RateLimit(#[from] RateLimitError),

    #[error("not implemented")]
    NotImplemented,
}

impl From<reqwest::Error> for DomainError {
    fn from(e: reqwest::Error) -> Self {
        DomainError::Network(e.to_string())
    }
}

impl From<serde_json::Error> for DomainError {
    fn from(e: serde_json::Error) -> Self {
        DomainError::Decode(e.to_string())
    }
}

/// Cache-related errors.
#[derive(Error, Debug, Clone)]
pub enum CacheError {
    #[error("cache capacity exceeded")]
    CapacityExceeded,
}

/// Rate-limit errors.
#[derive(Error, Debug, Clone)]
pub enum RateLimitError {
    #[error("rate limit exceeded; retry after {retry_after_secs}s")]
    Exceeded { retry_after_secs: u64 },
}
