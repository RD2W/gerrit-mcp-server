# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

- 102 tests (unit, integration, MCP tool pipeline)
- Wiremock-based HTTP client integration tests
- `MockGerritRepository` for full pipeline testing

## [0.1.0] — 2026-07-26

Initial pre-release.
