# Usage

Previous: [← Installation](./installation.md)
Next: [Architecture →](./architecture.md)

---

## Version

```bash
gerrit-mcp --version
```

Output includes version, author, commit hash, build date, and target platform.

> **Docker note:** When building the Docker image manually without `--build-arg GIT_HASH`,
> the commit hash will show as `unknown`. For correct metadata, pass the hash:
>
> ```bash
> docker build --build-arg GIT_HASH=$(git rev-parse HEAD) .
> ```

## Configuration reference

The configuration file is `config/config.toml`. See `config/config.example.toml` for
the annotated template. Environment variables override specific fields (listed below).

### `[gerrit]` — connection

| Field | Env var | Default | Description |
|---|---|---|---|
| `base_url` | `GERRIT_URL` | `""` | **Required.** Gerrit base URL (e.g. `https://gerrit.example.com`) |
| `timeout_secs` | — | `30` | HTTP request timeout |
| `ca_cert` | `GERRIT_CA_CERT` / `SSL_CERT_FILE` | — | Custom CA PEM path |
| `ca_cert_dir` | `SSL_CERT_DIR` | — | Directory of CA certs |
| `verify_ssl` | `GERRIT_VERIFY_SSL=false` | `true` | Enable/disable TLS verification |

### `[gerrit.auth]` — authentication

| Field | Description |
|---|---|
| `mode` | `"token"`, `"basic"`, or `"none"` |
| `token_env` | Env var name for the Bearer token (default: `GERRIT_TOKEN`) |
| `username_env` | Env var name for Basic auth username (default: `GERRIT_USERNAME`) |
| `auth_token_env` | Env var name for HTTP auth token (default: `GERRIT_AUTH_TOKEN`) |
| `gitcookies_path` | Path to a `.gitcookies` file for Gerrit auth |

Credentials are never stored in the config file — only the env var names.

### `[service]` — behaviour

| Field | Default | Description |
|---|---|---|
| `default_max_results` | `25` | Default result limit when client doesn't specify |

### `[cache]` — in-memory cache

| Field | Default | Description |
|---|---|---|
| `enabled` | `false` | Enable/disable cache |
| `ttl_secs` | `300` | Entry lifetime |
| `max_entries` | `1000` | Max cached responses |

### `[rate_limit]` — token bucket

| Field | Default | Description |
|---|---|---|
| `enabled` | `false` | Enable/disable rate limiting |
| `requests_per_second` | `10` | Sustained request rate |
| `burst` | `20` | Burst capacity |

### `[transport]` — server mode

| Field | Default | Description |
|---|---|---|
| `mode` | `"both"` | `"stdio"`, `"http"`, or `"both"` |
| `bind_addr` | `"0.0.0.0:8080"` | HTTP bind address |
| `http_path` | `"/mcp"` | MCP endpoint path |
| `health_path` | `"/healthz"` | Liveness endpoint |
| `ready_path` | `"/readyz"` | Readiness endpoint |
| `metrics_path` | `"/metrics"` | Prometheus metrics endpoint |
| `allowed_hosts` | — | Allowed Host header values (DNS rebinding protection) |

### `[log]`

| Field | Default | Description |
|---|---|---|
| `level` | `"info"` | `trace`, `debug`, `info`, `warn`, `error` — overridden by `RUST_LOG` |

---

## Transport modes

### stdio mode

```toml
[transport]
mode = "stdio"
```

The server reads MCP messages from stdin and writes to stdout. Use this:

- With `docker exec` for containerised Gerrit sidecars
- With Claude Desktop or other local MCP clients
- For debugging — easy to pipe test JSON messages

### HTTP mode (Streamable HTTP)

```toml
[transport]
mode = "http"
bind_addr = "0.0.0.0:8080"
```

The server starts an HTTP server with:

- MCP endpoint at the configured `http_path` (`/mcp`)
- Health check at `/healthz`
- Readiness check at `/readyz`
- Prometheus metrics at `/metrics`

Use this for multi-client deployments, remote access, or when the MCP client
doesn't support process spawning.

### both mode

```toml
[transport]
mode = "both"
```

Runs stdio and HTTP simultaneously. Useful for debugging HTTP deployments:
the stdio channel lets you inspect traffic while the HTTP server handles
production load.

---

## DNS rebinding protection

In HTTP mode, rmcp validates the `Host` header against `allowed_hosts`. Configure
it for your deployment:

```toml
# Docker — clients connect via container name
allowed_hosts = ["localhost", "127.0.0.1", "gerrit-mcp", "gerrit-mcp:8080"]

# Public deployment behind a reverse proxy
allowed_hosts = ["localhost", "mcp.example.com"]
```

---

## Health endpoints

| Endpoint | Behaviour |
|---|---|
| `GET /healthz` | Always `200 OK` if the process is alive |
| `GET /readyz` | `200` when config is loaded and Gerrit responds to a lightweight probe; `503` otherwise |
| `GET /metrics` | Prometheus text format — request counters, latencies, cache hits/misses |

### Docker health check

```yaml
healthcheck:
  test: ["CMD", "wget", "-qO-", "http://localhost:8080/healthz"]
  interval: 30s
  retries: 3
```

---

## MCP tools reference

The server exposes **28 tools** covering the full Gerrit REST API.

### Changes tools

| Tool | Description | Key parameters |
|---|---|---|
| `list_changes` | List changes with optional filtering | `query`, `status`, `project`, `limit`, `offset` |
| `get_change` | Get a single change by ID | `change_id` |
| `get_change_detail` | Get change with extra metadata | `change_id` |
| `get_commit` | Get commit info for a change revision | `change_id`, `revision_id` |
| `get_topic` | Get the topic for a change | `change_id` |
| `submit_change` | Submit a change | `change_id` |
| `abandon_change` | Abandon a change | `change_id` |
| `restore_change` | Restore an abandoned change | `change_id` |
| `revert_change` | Revert a merged change | `change_id` |

### Reviews tools

| Tool | Description | Key parameters |
|---|---|---|
| `list_reviewers` | List reviewers for a change | `change_id` |
| `suggest_reviewers` | Get reviewer suggestions | `change_id`, `query` |
| `get_review` | Get review details for a revision | `change_id`, `revision_id` |
| `set_review` | Set a review on a revision | `change_id`, `revision_id`, `message`, `labels`, `reviewers` |

### Search tools

| Tool | Description | Key parameters |
|---|---|---|
| `query_changes` | Search changes with Gerrit query syntax | `query`, `limit`, `offset`, `options` |

### Accounts tools

| Tool | Description | Key parameters |
|---|---|---|
| `get_account` | Get account details | `account_id` |
| `list_accounts` | List accounts | `query`, `limit` |
| `query_accounts` | Query accounts with Gerrit search | `query`, `limit` |

### Groups tools

| Tool | Description | Key parameters |
|---|---|---|
| `list_groups` | List groups | `query`, `limit` |
| `get_group` | Get group details | `group_id` |
| `get_group_members` | List members of a group | `group_id` |

### Projects tools

| Tool | Description | Key parameters |
|---|---|---|
| `list_projects` | List projects | `query`, `limit`, `prefix` |
| `get_project` | Get project details | `project_name` |
| `create_project` | Create a new project | `project_name`, `parent`, `branches` |
| `get_project_config` | Get project configuration | `project_name` |
| `list_branches` | List branches for a project | `project_name`, `limit` |
| `list_tags` | List tags for a project | `project_name`, `limit` |

### Plugins tools

| Tool | Description | Key parameters |
|---|---|---|
| `list_plugins` | List installed plugins | — |
| `get_plugin_status` | Get plugin status | `plugin_name` |

### Server tools

| Tool | Description | Key parameters |
|---|---|---|
| `get_server_info` | Get Gerrit server information | — |
| `get_server_version` | Get Gerrit server version | — |

### Result format

All search results include:

```json
{
  "results": [...],
  "total_hits": 1423,
  "page": 1,
  "has_more": true,
  "duration_ms": 230
}
```

- `has_more: true` tells the LLM that more results are available — it can request the next page
- `duration_ms` is the server-side processing time, useful for latency debugging

### Pagination

When `has_more` is `true`, the client can request additional pages by setting
an offset or page parameter:

```
Tool: query_changes
Query: "status:open"
Project: "aosp"
Offset: 25
```

The server transparently handles Gerrit's pagination mechanics and exposes
a simple page-based interface.
