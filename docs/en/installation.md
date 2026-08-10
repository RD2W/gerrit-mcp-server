# Installation

Next: [Usage →](./usage.md)
Previous: [← Overview](./overview.md)

---

## Requirements

- **Rust** 1.97 or later (edition 2024)
- **Gerrit** instance accessible over HTTP(S)
- **Docker** (optional — for containerised deployment)

---

## Building from source

```bash
git clone <repo-url> gerrit-mcp-server
cd gerrit-mcp-server

# Build in release mode
cargo build --release

# The binary is at:
#   target/release/gerrit-mcp
```

### Configuration

```bash
cp config/config.example.toml config/config.toml
```

Edit `config/config.toml` — at minimum, set:

```toml
[gerrit]
base_url = "https://your-gerrit.example.com"

[gerrit.auth]
mode = "bearer"   # or "http_basic" / "git_cookies" / "none"
token_env = "GERRIT_TOKEN"
```

Set credentials via environment variables:

```bash
# Bearer token auth:
export GERRIT_TOKEN="your-token-here"

# HTTP Basic auth:
export GERRIT_USERNAME="user"
export GERRIT_AUTH_TOKEN="your-http-password"

# Git cookies auth — config only:
# Set gitcookies_path in config.toml
```

### Run

```bash
cargo run --release
```

The server starts in stdio mode by default, ready for MCP clients.

---

## Docker

### Local development

```bash
# Set credentials in config/.env:
#   GERRIT_TOKEN=your-token
#   GERRIT_URL=https://gerrit.example.com

docker compose up -d
```

### Docker Hub (pre-built image)

Pre-built multi-arch images (linux/amd64, linux/arm64) are published to
[Docker Hub](https://hub.docker.com/r/rd2w/gerrit-mcp/tags) on every
tagged release.

```bash
# Pull the latest release
docker pull rd2w/gerrit-mcp:latest

# Or a specific version
docker pull rd2w/gerrit-mcp:v1.1.1

# Use the docker-compose file for pre-built images
docker compose -f docker-compose.hub.yml up -d
```

The `docker-compose.hub.yml` is identical to `docker-compose.yml` except it
uses `image:` instead of `build:` — no Rust toolchain or compilation required
on the target host.

### Remote / air-gapped deployment

For hosts **without internet access** (common in corporate environments), build
the image on a connected machine, then transfer it as a self-contained archive.
The multi-stage Docker build bakes all dependencies into the image — no network
access is required at runtime.

```bash
# 1. Build on a machine with internet access
#    (pulls base images, fetches Rust crates, compiles — all baked in)
docker build -t gerrit-mcp:latest .

# 2. Export as a single portable archive (~23 MB)
docker save gerrit-mcp:latest | gzip > gerrit-mcp.tar.gz

# 3. Transfer to the air-gapped host (USB drive, scp to jump host, etc.)
scp gerrit-mcp.tar.gz docker-compose.yml remote-host:~/mcp/

# 4. On the remote host — load and run (no internet needed)
ssh remote-host
cd ~/mcp/
docker load < gerrit-mcp.tar.gz               # imports the image

# Prepare configuration
mkdir -p config
cp /path/to/config.toml config/                 # your config
cp /path/to/your-ca.crt config/certs/           # CA cert (if using custom TLS)

# Create config/.env with secrets (never commit this file)
echo 'GERRIT_TOKEN=your-token' > config/.env
echo 'GERRIT_URL=https://gerrit.example.com' >> config/.env

docker compose up -d
```

> **Air-gapped checklist:** The image includes the Alpine base, `ca-certificates`,
> the compiled binary, and all Rust dependencies. The only external dependency is
> the Gerrit instance itself — the MCP server makes **outbound** HTTPS requests
> to it, so the host needs network access to Gerrit (but not to the internet
> at large).

### Image size

The multi-stage Docker build produces a UPX-compressed Alpine-based image of approximately
**23 MB** — small enough for easy transfer over slow connections.

---

## TLS with custom CAs

If your Gerrit instance uses a corporate or self-signed certificate:

1. Place the CA certificate PEM file at `config/certs/`:
   ```bash
   cp your-ca.crt config/certs/
   ```

2. Configure in `config.toml`:
   ```toml
   [gerrit]
   ca_cert = "./config/certs/your-ca.crt"
   ```

3. Or use environment variables:
   ```bash
   export GERRIT_CA_CERT=/path/to/ca.pem
   export SSL_CERT_FILE=/path/to/ca.pem
   export SSL_CERT_DIR=/path/to/certs/
   ```

For Docker, the `config/` directory is mounted read-only — certificates are
picked up automatically.

### Disabling TLS verification (insecure!)

Only for trusted internal networks:

```toml
[gerrit]
verify_ssl = false
```

Or `export GERRIT_VERIFY_SSL=false`.
