# Changelog

All notable changes to this project are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Bilingual documentation (EN + RU) in `docs/en/` and `docs/ru/`: overview, installation,
  usage (config reference + all 28 MCP tools), architecture (workspace layout, crate
  responsibilities, data flow, design decisions), and development guide (contributing,
  testing, CI, adding new tools).
- Bilingual README.md with English and Russian sections.

## [0.1.0] — 2026-07-26

### Added

- Initial release with 28 MCP tools covering the full Gerrit REST API
- Dual transport support: stdio and Streamable HTTP (axum + rmcp)
- Flexible authentication: Bearer token, HTTP Basic Auth, gitcookies
- TLS with custom CA certificate support
- DNS rebinding protection for Streamable HTTP mode
- In-memory TTL cache with configurable size
- Token-bucket rate limiting to protect Gerrit backend
- Health endpoints: `/healthz`, `/readyz`, `/metrics` (Prometheus)
- Multi-stage Docker build producing ~35 MB Alpine-based image
- Bilingual documentation (EN + RU)
- CI/CD pipeline: format, clippy, test, build, multi-arch Docker release
