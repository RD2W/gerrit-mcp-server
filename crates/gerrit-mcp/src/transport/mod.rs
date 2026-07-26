// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Transport layer.
//!
//! Dispatches between stdio and Streamable HTTP transports
//! based on configuration.

mod http;
mod stdio;

use gerrit_core::domain::GerritRepository;

use crate::config::Config;
use crate::mcp::GerritServer;

use self::http::run_http;
use self::stdio::run_stdio;

/// Runs the MCP server on the selected transport(s).
///
/// # Errors
/// Returns an error if transport binding fails.
pub async fn run_transport<R: GerritRepository + Send + Sync + 'static>(
    config: &Config,
    repo: R,
) -> anyhow::Result<()> {
    let server = GerritServer::new(repo);

    tracing::info!(mode = %config.transport.mode, "starting transport");

    match config.transport.mode.as_str() {
        "stdio" => {
            run_stdio(server).await?;
        }
        "http" => {
            run_http(config, server).await?;
        }
        "both" => {
            let http_config = config.clone();
            let http_server = server.clone();

            let http_handle = tokio::spawn(async move {
                if let Err(e) = run_http(&http_config, http_server).await {
                    tracing::error!(error = %e, "HTTP transport failed");
                }
            });

            let stdio_handle = tokio::spawn(async move {
                if let Err(e) = run_stdio(server).await {
                    tracing::error!(error = %e, "stdio transport failed");
                }
            });

            tokio::select! {
                _ = http_handle => {}
                _ = stdio_handle => {}
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("shutting down...");
                }
            }
        }
        other => {
            anyhow::bail!(
                "unknown transport mode: '{}' (expected stdio, http, or both)",
                other
            )
        }
    }

    Ok(())
}
