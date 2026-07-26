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
│   │       ├── domain.rs       # Data models: Change, Account, Group, Project, …
│   │       ├── application.rs  # Service layer: query, changes, reviews logic
│   │       └── infrastructure/
│   │           ├── client.rs   # HTTP client for Gerrit REST API
│   │           ├── tls.rs      # TLS configuration builder (rustls + native certs)
│   │           ├── cache.rs    # In-memory TTL cache (DashMap)
│   │           └── rate_limit.rs # Token-bucket rate limiter (governor)
│   └── gerrit-mcp/             # Binary — MCP server layer
│       └── src/
│           ├── main.rs         # Entry point, CLI args, logging init
│           ├── config.rs       # TOML config loading + env var overrides
│           ├── mcp/
│           │   ├── mod.rs      # MCP server setup, tool dispatch
│           │   └── tools.rs    # Tool definitions (JSON Schema via schemars)
│           ├── transport/
│           │   ├── mod.rs      # Transport abstraction
│           │   ├── stdio.rs    # stdin/stdout transport
│           │   └── http.rs     # Axum + rmcp Streamable HTTP transport
│           └── health.rs       # /healthz, /readyz, /metrics endpoints
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
│  ├── application.rs  service layer  │
│  ├── domain.rs       data models    │
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
| `domain.rs` | All data types: `Change`, `Account`, `Group`, `Project`, `Review`, error types (`CoreError`) |
| `application.rs` | High-level operations: `list_changes()`, `get_review()`, `query_accounts()`, with pagination, caching |
| `infrastructure/client.rs` | `reqwest`-based HTTP client: request building, auth header injection, response parsing |
| `infrastructure/tls.rs` | TLS configuration: custom CA loading, rustls setup, PEM parsing |
| `infrastructure/cache.rs` | In-memory cache with TTL eviction using `DashMap` |
| `infrastructure/rate_limit.rs` | Token-bucket rate limiter via `governor` |

### `gerrit-mcp` — MCP server

| Module | Purpose |
|---|---|
| `mcp/mod.rs` | MCP server initialisation, tool handler dispatch, error mapping (`CoreError` → MCP error codes) |
| `mcp/tools.rs` | Tool type definitions with JSON Schema (schemars): names, descriptions, parameter types, defaults |
| `config.rs` | Config loading: TOML parsing, env var overrides, validation |
| `transport/http.rs` | Axum router: MCP endpoint, health, readiness, metrics |
| `transport/stdio.rs` | stdin/stdout transport via rmcp |
| `health.rs` | Health check handlers: liveness, readiness with Gerrit probe, Prometheus metrics collection |
| `main.rs` | Entry point: CLI parsing, config init, transport selection, shutdown signal handling |

---

## Data flow

```
LLM Client
  │
  │  MCP request: { tool: "list_changes", params: { query: "...", project: "aosp" } }
  ▼
transport/stdio.rs or http.rs   ← receives MCP message
  │
  ▼
mcp/mod.rs                       ← routes by tool name
  │
  ▼
gerrit-core::application.rs      ← service logic, cache check, rate limit
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

### Why DashMap for cache?

`DashMap` is a concurrent hashmap — it allows lock-free reads and fine-grained
locking for writes. For an MCP server handling parallel LLM requests, this avoids
contention that a `Mutex<RwLock<HashMap>>` would create.

### Why governor for rate limiting?

`governor` implements the Generic Cell Rate Algorithm (GCRA) — a token-bucket
variant. It's lightweight, async-compatible, and well-suited for protecting a
single backend (Gerrit) from excessive request rates.
