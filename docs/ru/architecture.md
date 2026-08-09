# Архитектура

Назад: [← Использование](./usage.md)
Далее: [Разработка →](./development.md)

---

## Структура workspace

```
gerrit-mcp-server/
├── crates/
│   ├── gerrit-core/            # Библиотека — без зависимостей MCP
│   │   └── src/
│   │       ├── domain.rs       # Модели данных + типаж GerritRepository
│   │       ├── domain/
│   │       │   ├── error.rs    # Типы ошибок (DomainError, CacheError, RateLimitError)
│   │       │   └── mock.rs     # MockGerritRepository для тестирования
│   │       ├── application.rs  # GerritService — декоратор с кэшем и rate-limit
│   │       └── infrastructure/
│   │           ├── auth.rs     # Перечисление AuthMode, парсер gitcookies, AuthManager
│   │           ├── client.rs   # GerritClient — HTTP-клиент на reqwest для Gerrit REST API
│   │           ├── tls.rs      # Построитель TLS-конфигурации (rustls + системные сертификаты)
│   │           ├── cache.rs    # TTL + LRU кэш в памяти (lru::LruCache + Mutex)
│   │           └── rate_limit.rs # Ограничитель частоты token bucket (governor GCRA)
│   └── gerrit-mcp/             # Бинарный крейт — слой MCP-сервера
│       └── src/
│           ├── main.rs         # Точка входа, аргументы CLI, определение режима аутентификации
│           ├── config.rs       # Загрузка TOML-конфигурации + переопределение через env
│           ├── health.rs       # Обработчики /healthz, /readyz, /metrics
│           ├── mcp/
│           │   ├── mod.rs      # GerritServer, маршрутизатор инструментов, хелперы
│           │   ├── tools.rs    # Типы параметров инструментов (JSON Schema через schemars)
│           │   ├── changes.rs  # Обработчики жизненного цикла изменений
│           │   ├── reviews.rs  # Обработчики ревью и cherry-pick
│           │   └── comments.rs # Обработчики комментариев и черновиков
│           ├── transport/
│           │   ├── mod.rs      # Диспетчер транспорта (stdio / http / both)
│           │   ├── stdio.rs    # Транспорт stdin/stdout
│           │   └── http.rs     # Axum + rmcp Streamable HTTP транспорт
│           └── tests/          # CLI и интеграционные тесты
├── config/
│   ├── config.example.toml     # Аннотированный шаблон конфигурации
│   ├── config.toml             # Локальная конфигурация (в gitignore)
│   ├── .env                    # Секретные переменные окружения (в gitignore)
│   └── certs/                  # CA-сертификаты для TLS (в gitignore)
├── Dockerfile                  # Многоэтапная сборка на Alpine
└── docker-compose.yml          # Локальное dev-окружение
```

---

## Слоевая архитектура

```
┌─────────────────────────────────────┐
│         LLM-клиент (MCP)            │
├─────────────────────────────────────┤
│  gerrit-mcp (бинарный)              │
│  ├── transport/      stdio / HTTP   │
│  ├── mcp/tools.rs    схемы инструм. │
│  ├── mcp/mod.rs      обработчики    │
│  ├── config.rs       загрузка конф. │
│  └── health.rs       health/metrics │
├─────────────────────────────────────┤
│  gerrit-core (библиотека)           │
│  ├── application.rs  кэш/rate-limit │
│  ├── domain.rs       типаж + модели │
│  └── infrastructure/                │
│      ├── client.rs   HTTP-клиент    │
│      ├── tls.rs      настройка TLS  │
│      ├── cache.rs    кэш ответов    │
│      └── rate_limit  ограничитель   │
├─────────────────────────────────────┤
│         Gerrit API (REST)           │
└─────────────────────────────────────┘
```

### Направление зависимостей

`gerrit-mcp` зависит от `gerrit-core`. `gerrit-core` **не имеет зависимостей
MCP** — это чистая HTTP-клиентская библиотека, которую можно переиспользовать
в других контекстах.

---

## Зоны ответственности крейтов

### `gerrit-core` — домен и инфраструктура

| Модуль | Назначение |
|---|---|
| `domain.rs` | Типы данных: `Change`, `ChangeDetail`, `RevisionInfo`, `Comment` и др. Типаж `GerritRepository` (25 асинхронных методов) покрывает все операции Gerrit API |
| `domain/error.rs` | Перечисление `DomainError` с вариантами: `HttpStatus`, `Network`, `Decode`, `Tls`, `Auth`, `Cache`, `RateLimit`, `NotImplemented` |
| `domain/mock.rs` | `MockGerritRepository` — полная in-memory реализация для линеаризованного тестирования |
| `application.rs` | `GerritService<R>` — декоратор над любым `GerritRepository`. Применяет опциональный `MemoryCache` (TTL + LRU) и `TokenBucket` rate limiting. Реализует типаж `GerritRepository` |
| `infrastructure/client.rs` | `GerritClient` — реализация `GerritRepository` на `reqwest`. Обрабатывает XSSI-префиксы, percent-encoding, JSON-декодирование, HTTP-ошибки |
| `infrastructure/auth.rs` | Перечисление `AuthMode` (`HttpBasic`, `Bearer`, `GitCookies`). `parse_gitcookies()` для cookies в формате Netscape. Нормализация URL (HTTPS, добавление `/a` для HTTP Basic и GitCookies). `AuthManager` для поиска аутентификации по хосту |
| `infrastructure/tls.rs` | `TlsConfig` + `build_tls_connector()`. Системное хранилище через `rustls-native-certs`, пользовательские CA через `rustls-pemfile`, `NoVerifier` для отключённой проверки |
| `infrastructure/cache.rs` | `MemoryCache<K,V>` — TTL + LRU на основе `lru::LruCache` + `Mutex`. Потокобезопасный, ленивое истечение при доступе |
| `infrastructure/rate_limit.rs` | `TokenBucket` — обёртка над `governor::RateLimiter` с алгоритмом GCRA. Блокирующий `acquire()` и неблокирующий `check()` |

### `gerrit-mcp` — MCP-сервер

| Модуль | Назначение |
|---|---|
| `mcp/mod.rs` | `GerritServer<R>` — MCP-сервер с `Arc<R>` репозиторием. 28 методов с аннотацией `#[tool]`. Динамическое разрешение клиента для multi-instance Gerrit (через параметр `gerrit_base_url`). Хелперы: `extract_bugs()`, `sort_by_date()`, `merge_options()` |
| `mcp/tools.rs` | Типы параметров с JSON Schema (schemars) для всех 28 инструментов |
| `mcp/changes.rs` | Реализации инструментов жизненного цикла: запрос, создание, установка ready/WIP/topic, abandon, revert, submit |
| `mcp/reviews.rs` | Реализации инструментов ревью и cherry-pick: список файлов, diff, предложение/добавление ревьюеров, cherry-pick одного/цепочки |
| `mcp/comments.rs` | Реализации инструментов комментариев: список, публикация, удаление черновиков, publish |
| `config.rs` | Структура `Config` + подсекции. Разбор TOML, переопределение через env, валидация. Перечисление `ConfigError` |
| `transport/http.rs` | Маршрутизатор Axum с `StreamableHttpService` от rmcp. `NeverSessionManager` для stateless MCP 2026-07-28. Опциональная middleware `mcp_auth_token` (сравнение за константное время). Защита от DNS rebinding через `allowed_hosts` |
| `transport/stdio.rs` | Транспорт stdin/stdout через rmcp |
| `health.rs` | Глобальный синглтон `Metrics` с атомарными счётчиками. Обработчики `/healthz`, `/readyz`, `/metrics` (формат Prometheus) |
| `main.rs` | Точка входа: разбор CLI, инициализация конфигурации, выбор транспорта, обработка сигналов завершения |

---

## Поток данных

```
LLM-клиент
  │
  │  MCP-запрос: { tool: "query_changes", params: { query: "...", project: "aosp" } }
  ▼
transport/stdio.rs или http.rs   ← получение MCP-сообщения
  │
  ▼
mcp/mod.rs                       ← маршрутизация по имени инструмента
  │
  ▼
gerrit-core::application.rs      ← проверка кэша, ограничение частоты
  │
  ▼
gerrit-core::infrastructure/
  ├── cache.rs                   ← возврат из кэша при попадании
  ├── rate_limit.rs              ← ожидание при превышении лимита
  ├── client.rs                  ← HTTP-запрос к Gerrit
  │     │
  │     ▼
  │   tls.rs                     ← TLS с пользовательским CA (если настроен)
  │     │
  │     ▼
  │   Gerrit API
  │
  ▼
application.rs                   ← пагинация, формирование ответа с has_more
  │
  ▼
mcp/mod.rs                       ← сериализация в MCP-ответ
  │
  ▼
transport/                       ← отправка ответа обратно LLM
  │
  ▼
LLM-клиент
```

---

## Проектные решения

### Почему два крейта?

Разделение на `gerrit-core` (библиотека) и `gerrit-mcp` (бинарный) исключает
зависимости MCP из ядра HTTP-клиента. Это означает:

- Ядро можно использовать в не-MCP контекстах (например, CLI-инструмент или веб-интерфейс)
- Ускоряется компиляция при работе с ядром
- Зависимости чётко разделены — `rmcp`, `axum`, `schemars` есть только в бинарном крейте

### Почему reqwest + rustls?

- `reqwest` — де-факто стандартный HTTP-клиент Rust: хорошо протестирован, асинхронный, с поддержкой TLS
- `rustls` — реализация TLS на чистом Rust: исключает проблемы линковки OpenSSL, особенно
  в Docker-сборках на Alpine
- `rustls-native-certs` обеспечивает интеграцию с системным хранилищем сертификатов при необходимости

### Почему LRU-кэш с Mutex?

`lru::LruCache`, обёрнутый в `Mutex`, обеспечивает простой и корректный конкурентный кэш.
Для MCP-сервера, обрабатывающего параллельные LLM-запросы, грубая блокировка на кэше
приемлема, так как операции с кэшем быстры, а основная задержка исходит от вызовов
Gerrit API, а не от доступа к кэшу.

### Почему governor для ограничения частоты?

`governor` реализует Generic Cell Rate Algorithm (GCRA) — вариант token bucket.
Он лёгкий, совместим с async и хорошо подходит для защиты одного бэкенда (Gerrit)
от чрезмерной частоты запросов.
