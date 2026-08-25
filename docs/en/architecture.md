# Architecture

Previous: [← Usage](./usage.md)
Next: [Development →](./development.md)

---

## Workspace structure

```
gerrit-mcp-server/
├── crates/
│   ├── gerrit-core/            # Library — no MCP dependencies
│   │   └── src/
│   │       ├── domain.rs       # Data models + GerritRepository trait
│   │       ├── domain/
│   │       │   ├── error.rs    # Error types (DomainError, CacheError, RateLimitError)
│   │       │   └── mock.rs     # MockGerritRepository for testing
│   │       ├── application.rs  # GerritService — caching + rate-limit decorator
│   │       └── infrastructure/
│   │           ├── auth.rs     # AuthMode enum, gitcookies parser, AuthManager
│   │           ├── client.rs   # GerritClient — reqwest HTTP client for Gerrit REST API
│   │           ├── tls.rs      # TLS configuration builder (rustls + native certs)
│   │           ├── cache.rs    # In-memory TTL + LRU cache (lru::LruCache + Mutex)
│   │           └── rate_limit.rs # Token-bucket rate limiter (governor GCRA)
│   └── gerrit-mcp/             # Binary — MCP server layer
│       └── src/
│           ├── main.rs         # Entry point, CLI args, auth resolution
│           ├── config.rs       # TOML config loading + env var overrides
│           ├── health.rs       # /healthz, /readyz, /metrics handlers
│           ├── mcp/
│           │   ├── mod.rs      # GerritServer, tool router, helpers
│           │   ├── tools.rs    # Tool parameter types (JSON Schema via schemars)
│           │   ├── changes.rs  # Change lifecycle tool handlers
│           │   ├── reviews.rs  # Review and cherry-pick tool handlers
│           │   └── comments.rs # Comment and draft tool handlers
│           ├── transport/
│           │   ├── mod.rs      # Transport dispatcher (stdio / http / both)
│           │   ├── stdio.rs    # stdin/stdout transport
│           │   └── http.rs     # Axum + rmcp Streamable HTTP transport
│           └── tests/          # CLI and integration tests
├── config/
│   ├── config.example.toml     # Annotated configuration template
│   ├── config.toml             # Your local config (gitignored)
│   ├── .env                    # Secret env vars (gitignored)
│   └── certs/                  # CA certificates for TLS (gitignored)
├── Dockerfile                  # Multi-stage Alpine build
└── docker-compose.yml          # Local dev setup
```

---

## Layer architecture

```
┌─────────────────────────────────────┐
│         LLM Client (MCP)            │
├─────────────────────────────────────┤
│  gerrit-mcp (binary)                │
│  ├── transport/      stdio / HTTP   │
│  ├── mcp/tools.rs    tool schemas   │
│  ├── mcp/mod.rs      tool handlers  │
│  ├── config.rs       config load    │
│  └── health.rs       health/metrics │
├─────────────────────────────────────┤
│  gerrit-core (library)              │
│  ├── application.rs  caching/rate   │
│  ├── domain.rs       trait + models │
│  └── infrastructure/                │
│      ├── client.rs   HTTP client    │
│      ├── tls.rs      TLS setup      │
│      ├── cache.rs    response cache │
│      └── rate_limit  rate limiter   │
├─────────────────────────────────────┤
│         Gerrit API (REST)           │
└─────────────────────────────────────┘
```

### Dependency direction

`gerrit-mcp` depends on `gerrit-core`. `gerrit-core` has **no MCP
dependencies** — it's a pure HTTP client library that can be reused in
other contexts.

---

## Crate responsibilities

### `gerrit-core` — domain & infrastructure

| Module | Purpose |
|---|---|
| `domain.rs` | Data types: `Change`, `ChangeDetail`, `RevisionInfo`, `Comment`, etc. `GerritRepository` trait (25 async methods) covering all Gerrit API operations |
| `domain/error.rs` | `DomainError` enum with variants: `HttpStatus`, `Network`, `Decode`, `Tls`, `Auth`, `Cache`, `RateLimit`, `NotImplemented` |
| `domain/mock.rs` | `MockGerritRepository` — full in-memory mock for linearised testing |
| `application.rs` | `GerritService<R>` — decorator over any `GerritRepository`. Applies optional `MemoryCache` (TTL + LRU) and `TokenBucket` rate limiting. Implements `GerritRepository` trait |
| `infrastructure/client.rs` | `GerritClient` — `reqwest`-based implementation of `GerritRepository`. Handles XSSI prefix stripping, percent-encoding, JSON decoding, HTTP error mapping |
| `infrastructure/auth.rs` | `AuthMode` enum (`HttpBasic`, `Bearer`, `GitCookies`). `parse_gitcookies()` for Netscape-format cookies. URL normalisation (forces HTTPS, appends `/a` for HTTP Basic and GitCookies). `AuthManager` for per-host auth lookup |
| `infrastructure/tls.rs` | `TlsConfig` + `build_tls_connector()`. System trust store via `rustls-native-certs`, custom CA file/dir via `rustls-pemfile`, `NoVerifier` for disabled verification |
| `infrastructure/cache.rs` | `MemoryCache<K,V>` — TTL + LRU using `lru::LruCache` + `Mutex`. Thread-safe, lazy expiry on access |
| `infrastructure/rate_limit.rs` | `TokenBucket` — wraps `governor::RateLimiter` with GCRA algorithm. Blocking `acquire()` and non-blocking `check()` |

### `gerrit-mcp` — MCP server

| Module | Purpose |
|---|---|
| `mcp/mod.rs` | `GerritServer<R>` — MCP server holding an `Arc<R>` repository. 32 `#[tool]`-annotated methods. Dynamic client resolution for multi-instance Gerrit (via `gerrit_base_url` param). Helpers: `extract_bugs()`, `sort_by_date()`, `merge_options()` |
| `mcp/tools.rs` | Parameter types with JSON Schema (schemars) for all 32 tools |
| `mcp/changes.rs` | Tool implementations for change lifecycle: query, create, set ready/WIP/topic, abandon, revert, submit |
| `mcp/reviews.rs` | Tool implementations for reviews and cherry-picks: list files, get diff, suggest/add reviewer, cherry-pick single/chain |
| `mcp/comments.rs` | Tool implementations for comments: list, post, delete drafts, publish |
| `config.rs` | `Config` struct + sub-sections. TOML parsing, env var overrides, validation. `ConfigError` enum |
| `transport/http.rs` | Axum router with rmcp `StreamableHttpService`. `NeverSessionManager` for stateless MCP 2026-07-28. Optional `mcp_auth_token` middleware (constant-time comparison). DNS rebinding protection via `allowed_hosts` |
| `transport/stdio.rs` | stdin/stdout transport via rmcp |
| `health.rs` | Global `Metrics` singleton with atomic counters. Handlers for `/healthz`, `/readyz`, `/metrics` (Prometheus format) |
| `main.rs` | Entry point: CLI parsing, config init, transport selection, shutdown signal handling |

---

## Data flow

```
LLM Client
  │
  │  MCP request: { tool: "query_changes", params: { query: "...", project: "aosp" } }
  ▼
transport/stdio.rs or http.rs   ← receives MCP message
  │
  ▼
mcp/mod.rs                       ← routes by tool name
  │
  ▼
gerrit-core::application.rs      ← cache check, rate limit
  │
  ▼
gerrit-core::infrastructure/
  ├── cache.rs                   ← return cached if hit
  ├── rate_limit.rs              ← wait if throttled
  ├── client.rs                  ← HTTP request to Gerrit
  │     │
  │     ▼
  │   tls.rs                     ← TLS with custom CA (if configured)
  │     │
  │     ▼
  │   Gerrit API
  │
  ▼
application.rs                   ← paginate, build response with has_more
  │
  ▼
mcp/mod.rs                       ← serialize to MCP response
  │
  ▼
transport/                       ← send response back to LLM
  │
  ▼
LLM Client
```

---

## Design decisions

### Why two crates?

The split between `gerrit-core` (library) and `gerrit-mcp` (binary) keeps MCP
dependencies out of the core HTTP client. This means:

- The core can be used in non-MCP contexts (e.g., a CLI tool or web UI)
- Compile times are faster when working on the core
- Dependencies are clearly separated — `rmcp`, `axum`, `schemars` only appear in the binary

### Why reqwest + rustls?

- `reqwest` is the de-facto Rust HTTP client — well-tested, async, supports TLS
- `rustls` is a pure-Rust TLS implementation — avoids OpenSSL linkage issues, especially in
  Docker Alpine builds
- `rustls-native-certs` provides integration with the system trust store when needed

### Why LRU cache with Mutex?

`lru::LruCache` wrapped in `Mutex` provides a simple, correct concurrent cache.
For an MCP server handling parallel LLM requests, a coarse lock on the cache
is acceptable because cache operations are fast and the primary latency comes
from Gerrit API calls, not cache access.

### Why governor for rate limiting?

`governor` implements the Generic Cell Rate Algorithm (GCRA) — a token-bucket
variant. It's lightweight, async-compatible, and well-suited for protecting a
single backend (Gerrit) from excessive request rates.
