# Gerrit MCP Server

[![CI](https://github.com/rd2w/gerrit-mcp-server/actions/workflows/ci.yml/badge.svg)](https://github.com/rd2w/gerrit-mcp-server/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/rd2w/gerrit-mcp-server?color=blue)](https://github.com/rd2w/gerrit-mcp-server/releases)
[![Rust](https://img.shields.io/badge/rust-1.97%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2024-blue.svg)](https://doc.rust-lang.org/edition-guide/rust-2024/)
[![License](https://img.shields.io/badge/license-GPL--3.0--or--later-green.svg)](LICENSE)

---

## English

MCP (Model Context Protocol) server for [Gerrit](https://www.gerritcodereview.com/)
code review. Designed for **AOSP 15** scale workflows.

### Features

- **28 MCP tools** — full Gerrit REST API coverage: search, changes, reviews,
  accounts, groups, projects, branches, tags, plugins, and config
- **Dual transport** — stdio (`docker exec`) and Streamable HTTP (axum + rmcp)
- **Flexible auth** — Bearer token or HTTP Basic Auth to Gerrit
- **Custom CA** — TLS with corporate/self-signed certificates via
  `GERRIT_CA_CERT` / `SSL_CERT_FILE`
- **DNS rebinding protection** — `allowed_hosts` validation for Streamable HTTP
  (rmcp security check, configurable per deployment)
- **Optimized for AOSP** — result caching (TTL + eviction), rate limiting
  (token bucket), pagination with `has_more` hints
- **Health & metrics** — `/healthz`, `/readyz`, `/metrics` (Prometheus)
- **Docker** — multi-stage build (UPX-compressed 23 MB image), docker-compose

### Quick Start

#### Local

```bash
cp config/config.example.toml config/config.toml
# Edit config.toml — set base_url and auth mode
export GERRIT_USERNAME="user"
export GERRIT_PASSWORD="pass"
cargo run --release
```

#### Docker (local dev)

```bash
# Set credentials in config/.env (see config.example.toml)
docker compose up -d
```

#### Docker Hub (pre-built image)

Pre-built multi-arch images (linux/amd64, linux/arm64) are available on
[Docker Hub](https://hub.docker.com/r/rd2w/gerrit-mcp/tags):

```bash
# Pull the latest release
docker pull rd2w/gerrit-mcp:latest

# Use the docker-compose file for pre-built images
docker compose -f docker-compose.hub.yml up -d

# Or run directly with docker run:
docker run -d \
  --name gerrit-mcp \
  -p 8080:8080 \
  -v ./config:/config:ro \
  --env-file ./config/.env \
  rd2w/gerrit-mcp:latest
```

> The `docker-compose.hub.yml` uses `image:` instead of `build:`, so it pulls
> the pre-built image directly — no local Rust toolchain or compilation needed.

#### Docker (remote / air-gapped deployment)

For hosts **without internet access** (common in corporate environments), build
the image on a connected machine, then transfer the archive:

```bash
# 1. Build on a machine with internet (all dependencies baked in)
docker build -t gerrit-mcp:latest .

# 2. Export as a single portable archive (~23 MB, UPX-compressed)
docker save gerrit-mcp:latest | gzip > gerrit-mcp.tar.gz

# 3. Transfer to the air-gapped host (USB drive, scp, etc.)
scp gerrit-mcp.tar.gz docker-compose.yml remote-host:~/mcp/

# 4. On the remote host — load and run (no internet needed)
ssh remote-host
cd ~/mcp/
docker load < gerrit-mcp.tar.gz               # imports the image
mkdir -p config
cp /path/to/config.toml config/                 # your config
cp /path/to/your-ca.crt config/certs/           # CA cert (if needed)
# Create config/.env with secrets (never commit this file)
echo 'GERRIT_TOKEN=your-token' > config/.env
docker compose up -d
```

> The multi-stage build produces a fully self-contained Alpine-based image —
> no network access is required at runtime. TLS root certificates and all
> Gerrit client dependencies are baked into the image.

### Configuration

See `config/config.example.toml` for all options. Key sections:

| Section | Purpose |
|---|---|
| `[gerrit]` | Base URL, auth mode, TLS, CA cert path, timeout |
| `[gerrit.auth]` | `token` / `basic` / `none`, env var names for credentials |
| `[service]` | Default max results |
| `[cache]` | In-memory TTL cache for repeated queries |
| `[rate_limit]` | Token-bucket rate limiter (protects Gerrit) |
| `[transport]` | Transport mode, bind address, `allowed_hosts` for DNS rebinding |
| `[log]` | Log level (`RUST_LOG` overrides) |

#### Streamable HTTP & DNS rebinding protection

When `mode = "http"`, rmcp validates the `Host` header of incoming requests
against `allowed_hosts`. Requests with non-matching hosts receive **403 Forbidden**.

For Docker deployments where clients connect via container name:

```toml
[transport]
mode = "http"
bind_addr = "0.0.0.0:8004"
allowed_hosts = ["localhost", "127.0.0.1", "gerrit-mcp", "gerrit-mcp:8004"]
```

### Documentation

Full documentation is available in `docs/en/`:

- [Overview](docs/en/overview.md) — capabilities and project status
- [Installation](docs/en/installation.md) — requirements, build, Docker
- [Usage](docs/en/usage.md) — config reference, transport modes, all 28 MCP tools
- [Architecture](docs/en/architecture.md) — workspace layout, crate responsibilities, data flow
- [Development](docs/en/development.md) — contributing, testing, CI, conventions

### Development

```bash
cargo test                    # 199 tests
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Project structure

```
crates/gerrit-core/    # Library — domain models, HTTP client, TLS, cache, rate-limit
crates/gerrit-mcp/     # Binary — MCP server, config, transport (stdio + HTTP)
config/                # Configuration files and CA certificates (gitignored)
```

### License

GPL-3.0-or-later

---

## Русский

MCP (Model Context Protocol) сервер для [Gerrit](https://www.gerritcodereview.com/)
code review. Разработан для рабочих процессов масштаба **AOSP 15**.

### Возможности

- **28 инструментов MCP** — полное покрытие Gerrit REST API: поиск, изменения, ревью,
  аккаунты, группы, проекты, ветки, теги, плагины и конфигурация
- **Двойной транспорт** — stdio (`docker exec`) и Streamable HTTP (axum + rmcp)
- **Гибкая аутентификация** — Bearer-токен или HTTP Basic Auth для Gerrit
- **Пользовательские сертификаты** — TLS с корпоративными/самоподписанными сертификатами
  через `GERRIT_CA_CERT` / `SSL_CERT_FILE`
- **Защита от DNS rebinding** — проверка `allowed_hosts` для Streamable HTTP
  (механизм безопасности rmcp, настраивается под развёртывание)
- **Оптимизации для AOSP** — кэширование результатов (TTL + вытеснение), ограничение частоты
  (token bucket), пагинация с подсказками `has_more`
- **Health и метрики** — `/healthz`, `/readyz`, `/metrics` (Prometheus)
- **Docker** — многоэтапная сборка (образ 23 МБ, сжат UPX), docker-compose

### Быстрый старт

#### Локально

```bash
cp config/config.example.toml config/config.toml
# Отредактируйте config.toml — укажите base_url и режим аутентификации
export GERRIT_USERNAME="пользователь"
export GERRIT_PASSWORD="пароль"
cargo run --release
```

#### Docker (локальная разработка)

```bash
# Задайте учётные данные в config/.env (см. config.example.toml)
docker compose up -d
```

#### Docker Hub (готовый образ)

Готовые multi-arch образы (linux/amd64, linux/arm64) доступны на
[Docker Hub](https://hub.docker.com/r/rd2w/gerrit-mcp/tags):

```bash
# Загрузка последнего релиза
docker pull rd2w/gerrit-mcp:latest

# Используйте docker-compose файл для готовых образов
docker compose -f docker-compose.hub.yml up -d

# Или запустите напрямую через docker run:
docker run -d \
  --name gerrit-mcp \
  -p 8080:8080 \
  -v ./config:/config:ro \
  --env-file ./config/.env \
  rd2w/gerrit-mcp:latest
```

> Файл `docker-compose.hub.yml` использует `image:` вместо `build:`, поэтому
> загружает готовый образ напрямую — локальный Rust toolchain и компиляция не нужны.

#### Docker (удалённый / изолированный деплой)

Для хостов **без доступа в интернет** (корпоративные среды) образ собирается
на машине с сетью, затем переносится архивом:

```bash
# 1. Сборка на машине с интернетом (все зависимости вкомпилированы в образ)
docker build -t gerrit-mcp:latest .

# 2. Экспорт в один переносимый архив (~23 МБ, сжатый UPX)
docker save gerrit-mcp:latest | gzip > gerrit-mcp.tar.gz

# 3. Перенос на изолированный хост (USB-накопитель, scp и т.д.)
scp gerrit-mcp.tar.gz docker-compose.yml remote-host:~/mcp/

# 4. На удалённом хосте — загрузка и запуск (интернет не нужен)
ssh remote-host
cd ~/mcp/
docker load < gerrit-mcp.tar.gz               # импорт образа
mkdir -p config
cp /path/to/config.toml config/                 # ваша конфигурация
cp /path/to/your-ca.crt config/certs/           # CA-сертификат (если нужен)
# Создайте config/.env с учётными данными (никогда не коммитьте этот файл)
echo 'GERRIT_TOKEN=ваш-токен' > config/.env
docker compose up -d
```

> Многоэтапная сборка создаёт полностью самодостаточный образ на базе Alpine —
> сеть не требуется во время работы. Корневые TLS-сертификаты и все зависимости
> Gerrit-клиента вкомпилированы в образ.

### Конфигурация

Все опции в `config/config.example.toml`. Основные секции:

| Секция | Назначение |
|---|---|
| `[gerrit]` | Базовый URL, режим аутентификации, TLS, путь к CA-сертификату, таймаут |
| `[gerrit.auth]` | `token` / `basic` / `none`, имена переменных окружения для учётных данных |
| `[service]` | Максимум результатов по умолчанию |
| `[cache]` | TTL-кэш в памяти для повторных запросов |
| `[rate_limit]` | Ограничитель частоты token bucket (защищает Gerrit) |
| `[transport]` | Режим транспорта, адрес, `allowed_hosts` для защиты от DNS rebinding |
| `[log]` | Уровень логирования (переопределяется `RUST_LOG`) |

#### Streamable HTTP и защита от DNS rebinding

В режиме `mode = "http"` rmcp проверяет заголовок `Host` входящих запросов
по списку `allowed_hosts`. Запросы с несовпадающим хостом получают **403 Forbidden**.

Для Docker, где клиенты подключаются по имени контейнера:

```toml
[transport]
mode = "http"
bind_addr = "0.0.0.0:8004"
allowed_hosts = ["localhost", "127.0.0.1", "gerrit-mcp", "gerrit-mcp:8004"]
```

### Документация

Полная документация в `docs/ru/`:

- [Обзор](docs/ru/overview.md) — возможности и статус проекта
- [Установка](docs/ru/installation.md) — требования, сборка, Docker
- [Использование](docs/ru/usage.md) — справочник по конфигурации, режимы транспорта, все 28 инструментов MCP
- [Архитектура](docs/ru/architecture.md) — структура workspace, зоны ответственности крейтов, поток данных
- [Разработка](docs/ru/development.md) — участие, тестирование, CI, соглашения

### Разработка

```bash
cargo test                    # 102 теста
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

### Структура проекта

```
crates/gerrit-core/    # Библиотека — доменные модели, HTTP-клиент, TLS, кэш, rate-limit
crates/gerrit-mcp/     # Бинарный — MCP-сервер, конфигурация, транспорт (stdio + HTTP)
config/                # Файлы конфигурации и CA-сертификаты (в gitignore)
```

### Лицензия

GPL-3.0-or-later
