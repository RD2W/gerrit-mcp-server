// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Maxim Krutovercev (RD2W) <mkrutovercev@yandex.ru>

//! Stdio transport.

use gerrit_core::domain::GerritRepository;
use rmcp::ServiceExt;

use crate::mcp::GerritServer;

/// Runs the MCP server over stdio (for docker exec / local usage).
pub async fn run_stdio<R: GerritRepository + Send + Sync + 'static>(
    server: GerritServer<R>,
) -> anyhow::Result<()> {
    tracing::info!("starting stdio transport");
    let handle = server.serve(rmcp::transport::io::stdio()).await?;
    handle.waiting().await?;
    Ok(())
}
