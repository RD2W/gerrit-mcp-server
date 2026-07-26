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
| Changes | `list_changes`, `get_change`, `get_change_detail`, `get_commit`, `get_topic`, `submit_change`, `abandon_change`, `restore_change`, `revert_change` |
| Reviews | `list_reviewers`, `suggest_reviewers`, `get_review`, `set_review` |
| Search | `query_changes` |
| Accounts | `get_account`, `list_accounts`, `query_accounts` |
| Groups | `list_groups`, `get_group`, `get_group_members` |
| Projects | `list_projects`, `get_project`, `create_project`, `get_project_config`, `list_branches`, `list_tags` |
| Plugins | `list_plugins`, `get_plugin_status` |
| Server | `get_server_info`, `get_server_version` |

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
| `token` | Bearer token from an environment variable |
| `basic` | HTTP Basic Auth (username/password from env vars) |
| `none` | No authentication header — for open instances |

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

### Optimised for AOSP-scale codebases

| Feature | Purpose |
|---|---|
| **In-memory cache** | TTL-based cache with configurable size, avoids repeated API calls |
| **Rate limiting** | Token-bucket limiter protects the Gerrit backend from overload |
| **Pagination hints** | `has_more` field in responses tells the LLM when more results are available |

### Health & metrics

| Endpoint | Purpose |
|---|---|
| `/healthz` | Liveness — always returns 200 if the server is running |
| `/readyz` | Readiness — 200 when config is loaded and Gerrit is reachable |
| `/metrics` | Prometheus-format metrics (request counts, latencies, cache stats) |

### Docker

Multi-stage build producing a **~35 MB** Alpine-based image. Docker Compose config
for local development and remote deployment.

---

## Current status

**Pre-1.0.** The core HTTP client, all 28 MCP tools, dual transport, caching, rate
limiting, TLS, health endpoints, and Docker packaging are implemented and covered
by tests. The API is stable but may evolve before 1.0.
