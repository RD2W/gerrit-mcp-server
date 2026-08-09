# Overview

`gerrit-mcp` is an MCP (Model Context Protocol) server that bridges LLM clients to
[Gerrit](https://www.gerritcodereview.com/) code review. It was designed for
**AOSP 15-scale** workflows — hundreds of projects, thousands of changes, and complex
review pipelines.

Next: [Installation →](./installation.md)

---

## What problem does it solve?

LLM agents (Claude Desktop, Codex, etc.) need to interact with Gerrit code reviews in
large monorepos. Gerrit provides a REST API for searching changes, reviewing code,
managing accounts, groups, projects, and more, but its raw API is not LLM-friendly:

- Large response payloads need pagination and `has_more` hints
- Authentication and TLS with corporate CAs must be configured
- The API has specific behaviours that need normalisation
- Rate limiting is needed to protect the Gerrit server

`gerrit-mcp` wraps all of this into 28 clean MCP tools with proper error
handling, rate limiting, and result formatting.

---

## Features

### 28 MCP tools — full Gerrit REST API coverage

| Category | Tools |
|---|---|
| Querying changes (5) | `query_changes`, `query_changes_by_date_and_filters`, `get_change_details`, `get_most_recent_cl`, `changes_submitted_together` |
| Change content (6) | `get_commit_message`, `list_change_files`, `get_file_diff`, `list_change_comments`, `list_draft_comments`, `get_bugs_from_cl` |
| Change lifecycle (8) | `create_change`, `set_ready_for_review`, `set_work_in_progress`, `set_topic`, `abandon_change`, `revert_change`, `revert_submission`, `submit_change` |
| Code review (7) | `add_reviewer`, `suggest_reviewers`, `post_review_comment`, `post_draft_comment`, `delete_draft_comment`, `delete_draft_comments`, `publish_drafts` |
| Cherry-pick (2) | `cherry_pick_change`, `cherry_pick_chain` |

Each tool declares its parameters via JSON Schema (schemars), so LLM clients
automatically know the expected inputs and outputs — no manual prompt engineering
needed.

### Dual transport

| Mode | Use case |
|---|---|
| **stdio** | Direct process launch: `docker exec`, Claude Desktop local subprocess, debugging |
| **Streamable HTTP** | Network deployment: remote server, multiple clients, health checks, metrics |

The `both` mode runs stdio and HTTP simultaneously.

### Flexible authentication

| Mode | Description |
|---|---|
| `http_basic` / `basic` | HTTP Basic Auth (username + password from env vars) |
| `bearer` / `token` | Bearer token from an environment variable |
| `git_cookies` | Gerrit gitcookies file (Netscape cookie format) |
| `none` | No authentication — for open instances |

Credentials are **never** stored in the config file — only environment variable names.

### TLS with custom CAs

Corporate or self-signed CA certificates are supported through:

- `GERRIT_CA_CERT` / `SSL_CERT_FILE` — a single PEM file
- `SSL_CERT_DIR` — a directory of certificate files
- `config/certs/` directory (mounted read-only in Docker)

### DNS rebinding protection

When running in HTTP mode, rmcp validates the `Host` header against a configurable
`allowed_hosts` list. Requests with non-matching hosts receive **403 Forbidden**.
This prevents DNS rebinding attacks when the server is exposed on a network.

### MCP endpoint token authentication

The Streamable HTTP transport supports an optional Bearer token authentication
on the MCP endpoint (`mcp_auth_token`). When configured, clients must include
`Authorization: Bearer <token>` in requests. Token comparison uses
constant-time equality to prevent timing attacks.

### Optimised for AOSP-scale codebases

| Feature | Purpose |
|---|---|
| **In-memory cache** | TTL + LRU cache with configurable size, avoids repeated API calls |
| **Rate limiting** | Token-bucket limiter protects the Gerrit backend from overload |
| **Pagination hints** | `has_more` field in responses tells the LLM when more results are available |

### Health & metrics

| Endpoint | Purpose |
|---|---|
| `/healthz` | Liveness — always returns 200 if the server is running |
| `/readyz` | Readiness — 200 when config is loaded and process is ready |
| `/metrics` | Prometheus-format metrics (tool call counters, errors, uptime) |

### Docker

Multi-stage build producing a UPX-compressed **~23 MB** Alpine-based image. Docker Compose
config for local development and remote deployment.

---

## Current status

**v1.1.1.** The core HTTP client, all 28 MCP tools, dual transport, caching, rate
limiting, TLS, health endpoints, and Docker packaging are implemented and covered
by **216 tests**. Supports MCP 2026-07-28 protocol (stateless Streamable HTTP,
protocol negotiation) with legacy 2025-11-25 fallback.
