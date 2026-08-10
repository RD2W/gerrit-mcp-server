# Development

Previous: [← Architecture](./architecture.md)

---

## Getting started

```bash
git clone <repo-url> gerrit-mcp-server
cd gerrit-mcp-server
cargo build --workspace
cargo test --workspace
```

Development happens on the `dev` branch. Cut feature branches from it:

```bash
git checkout dev
git checkout -b feat/my-feature
```

---

## Running tests

```bash
# All tests (276+). Use --test-threads=1 to avoid env var race conditions
cargo test --workspace -- --test-threads=1

# Specific crate
cargo test -p gerrit-core
cargo test -p gerrit-mcp

# With output
cargo test -- --nocapture

# Run ignored (integration) tests
cargo test -- --ignored
```

---

## CI pipeline

CI runs on every push to `dev`, `main`, and `ci` branches, and on all PRs:

| Job | Command | Purpose |
|---|---|---|
| Format | `cargo fmt --all -- --check` | Ensures consistent code style |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Catches common mistakes and style issues |
| Tests | `cargo test --workspace --locked` | Runs all unit and integration tests |
| Build | `cargo build --workspace --locked --release` | Verifies the release build compiles |

GitHub Actions workflow: `.github/workflows/ci.yml`

---

## Code conventions

### General

- **Language:** English comments and commit messages
- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/) —
  `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:`
- **Formatting:** `rustfmt` with default settings
- **Linting:** `clippy` with `-D warnings` — all warnings are errors in CI

### SPDX headers

Every new `.rs` file must start with:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
```

See any existing source file for the exact format.

### Module organisation

- Small, focused files with clear responsibilities
- One major type or concern per file
- `mod.rs` files for module declarations and re-exports only

---

## Adding a new MCP tool

1. **Add the domain type** in `gerrit-core/src/domain.rs` if the API returns
   a new response shape.

2. **Add the trait method** to `GerritRepository` in `domain.rs` and implement
   it in `infrastructure/client.rs`.

3. **Wire through the decorator** in `application.rs` — `GerritService` delegates
   to the inner repository.

4. **Define the tool parameter type** in `gerrit-mcp/src/mcp/tools.rs` with
   `schemars` derives for JSON Schema generation.

5. **Implement the tool handler** in the appropriate `mcp/` module
   (`changes.rs`, `reviews.rs`, or `comments.rs`):
   ```rust
   #[tool(description = "Get a single change by ID")]
   async fn get_change_details(
       &self,
       change_id: String,
       ...
   ) -> Result<CallToolResult, McpError> {
       // …
   }
   ```
   Use `#[param(description = "...")]` for every parameter — these descriptions
   are exposed to LLM clients and directly affect tool call quality.

6. **Register the handler** in `gerrit-mcp/src/mcp/mod.rs` — the tool is
   automatically discovered via the `#[tool]` macro.

7. **Add tests** — unit tests for the domain type, mock tests for the handler,
   and integration tests for the HTTP client.

---

## Documentation

When behaviour changes, update:

- The relevant `docs/en/` and `docs/ru/` pages (keep them in sync)
- `README.md` if it affects the quick start or feature list
- `CHANGELOG.md` — add an entry under `[Unreleased]`
- `config/config.example.toml` and `config/.env.example` — if configuration options change

---

## Pre-PR checklist

Before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

All four must pass. CI enforces the first three — the build check catches
compilation issues that tests might miss.

---

## Release workflow

Releases are automated via `.github/workflows/release.yml`:

1. Push a tag like `v1.1.1`
2. CI builds multi-arch Docker images and creates a GitHub Release
3. Binary artifacts are attached to the release

Manual release steps (for debugging the workflow):

```bash
docker build -t gerrit-mcp:v1.1.1 .
docker tag gerrit-mcp:v1.1.1 gerrit-mcp:latest
```
