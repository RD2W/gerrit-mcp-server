# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.2.0] — 2026-08-10

### Added

- **Read-only mode** (`READ_ONLY_MODE` env / `service.read_only` config) — disables
  all 16 write tools while keeping 12 read tools operational. Useful for CI pipelines,
  audit environments, or LLM agents that should only read Gerrit data.
- **Environment variable overrides for all configuration fields** — every setting in
  `config.toml` now has a corresponding env var (`GERRIT_URL`, `MCP_CACHE_ENABLED`,
  `MCP_RATE_LIMIT_RPS`, etc.). Sensitive credentials loaded indirectly via env var
  indirection (config specifies the env var name, code reads the value).
- **MCP endpoint Bearer token authentication** — `mcp_auth_token` config field +
  `MCP_AUTH_TOKEN` env var. When set, HTTP clients must include
  `Authorization: Bearer <token>`. Constant-time comparison prevents timing attacks.

### Changed

- **Refactored codebase**: split monolithic modules into focused files, wired
  Prometheus metrics (`tool_calls_total`, `tool_errors_total`, `queries_total`),
  switched cache from `DashMap`-based to `LRU` with TTL.
- **Unpinned reqwest version constraint** — allows semver-compatible updates
  without manual `Cargo.toml` changes.
- **Config and CI docs updated** for the env var override system.

### Fixed

- **`get_commit_message` endpoint** corrected — was hitting a wrong Gerrit API path.
- **`get_file_diff` handler** now respects the `gerrit_base_url` parameter
  (previously ignored, always routing through the default service).
- **`build_options_query`** and **`get_commit_message`** Gerrit API calls fixed
  for correct URL construction.
- **Topic and reviewer fields** now properly populated in change and detail models.
- **Multi-instance support**: per-URL Gerrit client cache for on-demand connections.
- **Default bind address** set to `127.0.0.1` (was `0.0.0.0`) for security.
- **`AuthMode` Debug output** no longer leaks secrets.
- **CI**: UPX binary compression for `aarch64` release binaries.
- **CI**: Dependabot target branch set to `dev`.

## [1.1.1] — 2026-08-09

### Changed

- **rmcp upgraded to 3.1.2** (was 3.0.1) — MCP protocol conformance fixes:
  stateless request routing for 2026-07-28, MRTR round-trip support,
  per-request protocol metadata validation, cancel-safe receive.

### Fixed

- **HTTP transport: disabled legacy session mode** via
  `StreamableHttpServerConfig::with_legacy_session_mode(false)`.
  rmcp 3.1.x requires this when using `NeverSessionManager` —
  otherwise all requests are routed through the legacy session
  creation path, which `NeverSessionManager` rejects.

### Added

- **Docker: UPX binary compression** for smaller image size (Dockerfile + CI release workflow).
- **CI: install-upx composite action** and UPX step in binary release builds.

### Changed

- **Updated 23 dependencies** to latest semver-compatible versions
  (base64, clap, thiserror, zerocopy, async-trait, aws-lc-rs, etc.).

## [1.1.0] — 2026-07-31

### Changed

- **rmcp upgraded to 3.0** (was 2.2) — MCP 2026-07-28 protocol support:
  stateless discovery, protocol negotiation, multi round-trip requests (MRTR).
  Tool return types handled automatically by `#[tool_router]` macro.
- **HTTP transport: swapped `LocalSessionManager` → `NeverSessionManager`**
  for fully stateless Streamable HTTP (no session tracking, no GET/DELETE).
- **Server announces both protocol versions**: 2026-07-28 (preferred) and
  2025-11-25 (legacy fallback) via `supported_protocol_versions()`.

## [1.0.0] — 2026-07-27

First stable release. 🚀

### MCP Tools (28 total)

- **Querying changes**: `query_changes`, `query_changes_by_date_and_filters`,
  `get_change_details`, `get_most_recent_cl`, `changes_submitted_together`
- **Change content**: `get_commit_message`, `list_change_files`, `get_file_diff`,
  `list_change_comments`, `list_draft_comments`, `get_bugs_from_cl`
- **Change lifecycle**: `create_change`, `set_ready_for_review`, `set_work_in_progress`,
  `set_topic`, `abandon_change`, `revert_change`, `revert_submission`, `submit_change`
- **Code review**: `add_reviewer`, `suggest_reviewers`, `post_review_comment`,
  `post_draft_comment`, `delete_draft_comment`, `delete_draft_comments`, `publish_drafts`
- **Cherry-pick**: `cherry_pick_change`, `cherry_pick_chain`

### Authentication

- Bearer token authentication
- HTTP Basic authentication (username + token)
- Git cookies (Netscape-format `.gitcookies`) support
- Host-based `AuthManager` for multi-instance deployments
- All credentials sourced from environment variables — never stored in config

### Transport

- **stdio** — local subprocess / Claude Desktop / `docker exec`
- **Streamable HTTP** — axum + rmcp for multi-client remote deployments
- **Both** — simultaneous stdio + HTTP for debugging
- DNS rebinding protection via `allowed_hosts`

### TLS

- rustls-based TLS (pure Rust, no OpenSSL dependency)
- System trust store integration via `rustls-native-certs`
- Custom CA certificate file or directory support
- Optional verification disable for trusted internal networks

### Caching & Rate Limiting

- In-memory TTL cache (`DashMap`-based) with configurable TTL and max entries
- Token-bucket rate limiting via `governor` (GCRA algorithm)
- Both applied transparently via `GerritService` decorator pattern

### Health & Metrics

- `GET /healthz` — liveness probe
- `GET /readyz` — readiness probe
- `GET /metrics` — Prometheus-formatted metrics (`uptime_seconds`, `tool_calls_total`,
  `tool_errors_total`, `queries_total`)

### Deployment

- Multi-stage Docker image (~35 MB, Alpine 3.24)
- Multi-arch support: `linux/amd64`, `linux/arm64`
- Docker Compose files for local build and Docker Hub images
- Read-only config volume, secrets via `config/.env`
- Air-gapped deployment guide

### Documentation

- Bilingual documentation (EN + RU): overview, installation, usage, architecture,
  development guide
- Bilingual README.md
- Annotated configuration example (`config/config.example.toml`)
- Contributing guide and security policy

### CI/CD

- GitHub Actions: format (`cargo fmt`), lint (`clippy -D warnings`), test, release build
- Multi-arch Docker release on tag push
- Pre-built images on Docker Hub (`rd2w/gerrit-mcp`)

### Testing

- 199 tests (unit, integration, MCP tool pipeline)
- Wiremock-based HTTP client integration tests
- `MockGerritRepository` for full pipeline testing

## [0.1.0] — 2026-07-26

Initial pre-release.
