# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] — 2026-08-25

### Added

- **3 new core Gerrit API tools** — `get_revision_commit` (full commit object
  for a revision), `get_related_changes` (relation-chain panel with
  subject/status), `get_git_parent_changes` (changes that are parents of a
  change via the `parentof:` query). All pure core API; plugin tools remain
  out of scope.

### Changed

- **Minimum supported Rust version raised to 1.98** — the project and the
  published Docker image are now built with Rust 1.98 (edition 2024).

### Fixed

- **`add_reviewer` no longer bypasses server confirmation policy** — the tool
  no longer sends `confirmed: true` unconditionally. `confirmed` is now an
  optional parameter (default: not sent, like the reference client), so
  `addreviewer.maxWithoutConfirmation` applies normally unless the caller
  explicitly requests confirmed status.

- **Gerrit API parity fixes** — draft comments now send `unresolved`/`range`/
  embedded `` ```suggestion `` blocks and review comments forward `labels`;
  deleting a topic (204 empty body) no longer errors; `get_change_details`
  extracts bugs from the current revision; cherry-pick tools forward
  `keep_reviewers`/`allow_conflicts`/`allow_empty` and chain cherry-picks use
  the current revision SHA as `base`; `query_changes_by_date_and_filters`
  validates both dates and quotes the `message:` filter; bug extraction
  matches the reference implementation (footer `b/` ids, space-separated
  lists, case-insensitive inline `b/NNN`).

  The legacy `suggestion:`-prefixed `message` special case is removed — use
  the dedicated `suggestion` argument instead, which embeds a
  `` ```suggestion `` block.

- **`get_commit_message` works on Gerrit < 3.10** — when `GET /changes/{id}/message`
  (added in Gerrit 3.10) returns 404, the tool now falls back to
  `GET /changes/{id}/revisions/current/commit` and returns `CommitInfo.message`
  unchanged. Verbatim contract on 3.10+ is preserved.

- **`get_commit_message` returns the verbatim commit message** — the tool now
  reads `GET /changes/{id}/message` and returns `full_message` as-is, instead of
  a reformatted `CommitInfo` summary with synthetic headers and 8-character
  truncated parent SHAs.

- **`changes_submitted_together` accepts both Gerrit response shapes** — Gerrit's
  `GET /changes/{id}/submitted_together` returns either a bare JSON array of
  `ChangeInfo` (all changes visible) or an object with `changes` /
  `non_visible_changes` (some changes not visible). Previously the client only
  deserialized the object shape, so the tool failed with
  `JSON parse error: invalid type: map, expected a sequence` (as observed on
  change 35250). The response is now parsed via an untagged
  `SubmittedTogetherResponse` enum that accepts both forms and normalized to
  `SubmittedTogether` (the array form maps `non_visible_changes` to 0).

- **`publish_drafts` now actually publishes drafts** — the tool posts to Gerrit's
  "Set Review" endpoint (`POST /changes/{id}/revisions/current/review`) with
  `"drafts": "PUBLISH_ALL_REVISIONS"`, and now forwards the optional `message` and
  `labels` arguments. Previously the request body was empty and ignored the
  message/labels params; Gerrit defaults the `drafts` field to `KEEP` (see the
  `ReviewInput` source: *"If not set, the default is `KEEP`"*), so the endpoint
  returned success while leaving the draft comments unpublished ("false success").
  `PUBLISH_ALL_REVISIONS` (matching the reference Python client) publishes drafts
  from every revision of the change, not just the current one. The success
  message is only accurate now.

## [1.3.0] — 2026-08-19

### Added

- **`set_labels` tool** — set one or more label votes (e.g. `READY-FOR-CI: 1`,
  `TARGET: 3`) in a single `POST /changes/{id}/revisions/current/review` call,
  with an optional review `message` and no inline comment.

### Changed

- **rmcp upgraded to 3.1.3** (was 3.1.2) — client-side OAuth fixes (query-param
  resource matching, issuer state retention, discovery probe timeout).
- **Updated dependencies** to latest semver-compatible versions (h2, quinn-proto,
  rustls-webpki, uuid, futures, icu, etc.).

### Fixed

- **`gerrit_base_url` override now works on all write tools** — `create_change`,
  `set_ready_for_review`, `set_work_in_progress`, `set_topic`, `abandon_change`,
  `revert_change`, `revert_submission`, `submit_change`, `post_review_comment`,
  `post_draft_comment`, `delete_draft_comment`, `delete_draft_comments`,
  `publish_drafts`, `add_reviewer`, `cherry_pick_change`, and `cherry_pick_chain`
  now honor the `gerrit_base_url` parameter (previously declared but silently
  ignored). The parameter is now optional (`None` = configured server) on these
  tools, matching the read tools. All MCP tools now resolve the target server
  through a shared `resolve_repo` helper.
- **Container health restored** — `/healthz`, `/readyz`, `/metrics` are now public
  (token auth applies only to `/mcp`), and the Docker healthcheck resolves
  `127.0.0.1:${HEALTHCHECK_PORT}${HEALTHCHECK_PATH}` via runtime `ENV` vars
  (previously unset `ARG`s made the container report `unhealthy`).

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
