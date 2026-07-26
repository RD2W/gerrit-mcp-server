// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Health check, readiness probe, and Prometheus metrics endpoints.
//!
//! - `/healthz` — liveness: always returns 200 if the process is alive.
//! - `/readyz`  — readiness: returns 200 if ready to serve.
//! - `/metrics` — Prometheus exposition format.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use axum::Json;
use serde_json::json;

// ---------------------------------------------------------------------------
// Metrics counters
// ---------------------------------------------------------------------------

/// Global metrics singleton (created once on first access).
#[derive(Debug)]
pub(crate) struct Metrics {
    /// Total MCP tool calls processed.
    tool_calls_total: AtomicU64,
    /// Tool calls that returned an error.
    tool_calls_errors: AtomicU64,
    /// Total queries processed (subset of tool_calls).
    queries_total: AtomicU64,
    /// Process start time (for uptime).
    started_at: Instant,
}

static METRICS: OnceLock<Metrics> = OnceLock::new();

#[must_use]
pub fn metrics() -> &'static Metrics {
    METRICS.get_or_init(Metrics::new)
}

impl Metrics {
    fn new() -> Self {
        Self {
            tool_calls_total: AtomicU64::new(0),
            tool_calls_errors: AtomicU64::new(0),
            queries_total: AtomicU64::new(0),
            started_at: Instant::now(),
        }
    }

    #[allow(dead_code)]
    pub fn record_tool_call(&self) {
        self.tool_calls_total.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn record_tool_error(&self) {
        self.tool_calls_errors.fetch_add(1, Ordering::Relaxed);
    }

    #[allow(dead_code)]
    pub fn record_query(&self) {
        self.queries_total.fetch_add(1, Ordering::Relaxed);
    }
}

// ---------------------------------------------------------------------------
// Axum handlers
// ---------------------------------------------------------------------------

/// Liveness handler — always 200 if the process is running.
pub async fn health_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ok"}))
}

/// Readiness handler — returns 200 indicating readiness.
pub async fn ready_handler() -> Json<serde_json::Value> {
    Json(json!({"status": "ready", "gerrit": "unknown"}))
}

/// Prometheus metrics exposition handler.
pub async fn metrics_handler() -> String {
    let m = metrics();
    let uptime = m.started_at.elapsed().as_secs();
    let tool_calls = m.tool_calls_total.load(Ordering::Relaxed);
    let errors = m.tool_calls_errors.load(Ordering::Relaxed);
    let queries = m.queries_total.load(Ordering::Relaxed);

    format!(
        "# HELP gerrit_mcp_up Whether the server is up (1=alive)\n\
         # TYPE gerrit_mcp_up gauge\n\
         gerrit_mcp_up 1\n\
         # HELP gerrit_mcp_uptime_seconds Server uptime in seconds\n\
         # TYPE gerrit_mcp_uptime_seconds counter\n\
         gerrit_mcp_uptime_seconds {uptime}\n\
         # HELP gerrit_mcp_tool_calls_total Total MCP tool calls\n\
         # TYPE gerrit_mcp_tool_calls_total counter\n\
         gerrit_mcp_tool_calls_total {tool_calls}\n\
         # HELP gerrit_mcp_tool_errors_total Tool calls that returned errors\n\
         # TYPE gerrit_mcp_tool_errors_total counter\n\
         gerrit_mcp_tool_errors_total {errors}\n\
         # HELP gerrit_mcp_queries_total Queries processed\n\
         # TYPE gerrit_mcp_queries_total counter\n\
         gerrit_mcp_queries_total {queries}\n"
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn health_returns_ok() {
        let resp = health_handler().await;
        assert_eq!(resp["status"], "ok");
    }

    #[tokio::test]
    async fn ready_returns_json() {
        let resp = ready_handler().await;
        assert_eq!(resp["status"], "ready");
        assert!(resp["gerrit"].is_string());
    }

    #[tokio::test]
    async fn metrics_includes_counters() {
        metrics().record_tool_call();
        metrics().record_query();
        metrics().record_tool_error();

        let out = metrics_handler().await;
        assert!(out.contains("gerrit_mcp_up 1"));
        assert!(out.contains("gerrit_mcp_tool_calls_total"));
        assert!(out.contains("gerrit_mcp_queries_total"));
        assert!(out.contains("gerrit_mcp_tool_errors_total"));
        assert!(out.contains("gerrit_mcp_uptime_seconds"));
    }

    #[test]
    fn metrics_singleton() {
        let m1 = metrics();
        let m2 = metrics();
        assert!(std::ptr::eq(m1, m2));
    }
}
