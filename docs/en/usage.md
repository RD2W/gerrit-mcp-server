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
| `timeout_secs` | — | `30` | HTTP request timeout in seconds |
| `ca_cert` | `GERRIT_CA_CERT` / `SSL_CERT_FILE` | — | Custom CA PEM path |
| `ca_cert_dir` | `SSL_CERT_DIR` | — | Directory of CA certs |
| `verify_ssl` | `GERRIT_VERIFY_SSL=false` | `true` | Enable/disable TLS verification |

### `[gerrit.auth]` — authentication

| Field | Description |
|---|---|
| `mode` | Auth mode: `"http_basic"` / `"basic"`, `"bearer"` / `"token"`, `"git_cookies"`, or `"none"` |
| `username_env` | Env var name for Basic auth username (e.g. `GERRIT_USERNAME`) |
| `auth_token_env` | Env var name for HTTP auth token/password (default: `GERRIT_AUTH_TOKEN`) |
| `token_env` | Env var name for the Bearer token (default: `GERRIT_TOKEN`) |
| `gitcookies_path` | Path to a `.gitcookies` file for Gerrit auth (Netscape format) |

Credentials are never stored in the config file — only the env var names.

### `[service]` — behaviour

| Field | Default | Description |
|---|---|---|
| `default_max_results` | `25` | Default result limit when client doesn't specify |
| `read_only` | `false` | Disable all write operations (env: `READ_ONLY_MODE`) |

### `[cache]` — in-memory cache

| Field | Default | Description |
|---|---|---|
| `enabled` | `false` | Enable/disable TTL + LRU cache |
| `ttl_secs` | `300` | Entry lifetime in seconds |
| `max_entries` | `1000` | Max cached responses (LRU eviction) |

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
| `bind_addr` | `"127.0.0.1:8080"` | HTTP bind address (use `0.0.0.0:8080` for network access) |
| `http_path` | `"/mcp"` | MCP Streamable HTTP endpoint path |
| `health_path` | `"/healthz"` | Liveness endpoint |
| `ready_path` | `"/readyz"` | Readiness endpoint |
| `metrics_path` | `"/metrics"` | Prometheus metrics endpoint |
| `allowed_hosts` | — | Allowed Host header values (DNS rebinding protection) |
| `mcp_auth_token` | `""` | Optional Bearer token for MCP endpoint auth (empty = disabled) |

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

## MCP endpoint token authentication

When `mcp_auth_token` is set to a non-empty value, the HTTP transport requires
a valid Bearer token on every request to the MCP endpoint. Token comparison
uses constant-time equality to prevent timing attacks.

```toml
[transport]
mode = "http"
mcp_auth_token = "my-secret-token"
```

Clients must include the header:
```
Authorization: Bearer my-secret-token
```

Requests without a valid token receive **401 Unauthorized**. Health and metrics
endpoints are not affected — only the MCP endpoint is protected.

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

## Read-only mode

When enabled, the server blocks all write operations — creates, updates,
deletes, submissions, cherry-picks, and other modifying actions. Read tools
remain fully available.

```toml
[service]
read_only = true
```

Or via environment variable:

```bash
READ_ONLY_MODE=true
```

Blocked tools return an error message: `"Cannot create change in read-only mode."`

Use this for CI pipelines, audit environments, or any scenario where an LLM
should only be permitted to read Gerrit data.

---

## Health endpoints

| Endpoint | Behaviour |
|---|---|
| `GET /healthz` | Always `200 OK` if the process is alive |
| `GET /readyz` | `200` when config is loaded and process is ready; serves as a basic readiness signal |
| `GET /metrics` | Prometheus text format — tool call counters, errors, queries, uptime |

### Docker health check

```yaml
healthcheck:
  test: ["CMD", "wget", "-qO-", "http://localhost:8080/healthz"]
  interval: 30s
  retries: 3
```

---

## MCP tools reference

The server exposes **29 tools** covering the full Gerrit REST API.

### Querying changes

| Tool | Description | Key parameters |
|---|---|---|
| `query_changes` | Search changes with Gerrit query syntax | `query`, `limit?`, `options?` |
| `query_changes_by_date_and_filters` | Query changes in a date range with filters | `start_date`, `end_date`, `project?`, `message_substring?`, `status?`, `limit?` |
| `get_change_details` | Get detailed change info (revisions, labels, reviewers) | `change_id`, `options?` |
| `get_most_recent_cl` | Get the most recent change from a user | `user` |
| `changes_submitted_together` | List changes submitted together with this one | `change_id`, `options?` |

### Change content

| Tool | Description | Key parameters |
|---|---|---|
| `get_commit_message` | Get verbatim commit message for a change (`GET /changes/{id}/message`) | `change_id` |
| `get_revision_commit` | Get the full commit object of a revision | `change_id`, `revision_id?` |
| `get_related_changes` | Get changes related to a revision (relation chain) | `change_id`, `revision_id?` |
| `get_git_parent_changes` | Get parent changes of a change (`parentof:` query) | `change_id`, `limit?` |
| `list_change_files` | List files modified in a change | `change_id` |
| `get_file_diff` | Get the diff for a file in a change | `change_id`, `file_path` |
| `list_change_comments` | List published comments on a change | `change_id` |
| `list_draft_comments` | List draft comments on a change | `change_id` |
| `get_bugs_from_cl` | Extract bug references from a change | `change_id` |

### Change lifecycle

| Tool | Description | Key parameters |
|---|---|---|
| `create_change` | Create a new change | `project`, `branch`, `subject`, `topic?`, `status?` |
| `set_ready_for_review` | Mark a change as ready for review | `change_id` |
| `set_work_in_progress` | Mark a change as work-in-progress | `change_id`, `message?` |
| `set_topic` | Set the topic for a change; empty `topic` deletes it | `change_id`, `topic` |
| `abandon_change` | Abandon a change | `change_id`, `message?` |
| `revert_change` | Revert a merged change | `change_id`, `message?` |
| `revert_submission` | Revert a submission | `change_id`, `message?` |
| `submit_change` | Submit a change for merge | `change_id`, `wait_for_merge?` |

### Code review

| Tool | Description | Key parameters |
|---|---|---|
| `add_reviewer` | Add a reviewer to a change | `change_id`, `reviewer`, `state?`, `confirmed?` |
| `suggest_reviewers` | Get reviewer suggestions | `change_id`, `query`, `limit?`, `exclude_groups?` |
| `set_labels` | Set one or more label votes on a change | `change_id`, `labels`, `message?`, `gerrit_base_url?` |
| `post_review_comment` | Post a review comment | `change_id`, `file_path`, `line_number`, `message`, `unresolved?`, `labels?` |
| `post_draft_comment` | Post a draft comment (range, unresolved, inline suggestion) | `change_id`, `file_path`, `line_number`, `message`, `unresolved?`, `suggestion?`, `in_reply_to?`, `start_line?`, `start_character?`, `end_line?`, `end_character?` |
| `delete_draft_comment` | Delete a specific draft | `change_id`, `draft_id` |
| `delete_draft_comments` | Delete all drafts on a change | `change_id` |
| `publish_drafts` | Publish draft comments (sends `drafts=PUBLISH_ALL_REVISIONS`) | `change_id`, `message?`, `labels?` |

### Cherry-pick

| Tool | Description | Key parameters |
|---|---|---|
| `cherry_pick_change` | Cherry-pick to a destination branch | `change_id`, `destination`, `revision_id?`, `message?`, `keep_reviewers?`, `allow_conflicts?`, `allow_empty?` |
| `cherry_pick_chain` | Cherry-pick a chain of changes | `change_id`, `destination`, `revision_id?`, `keep_reviewers?`, `allow_conflicts?`, `allow_empty?` |

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
