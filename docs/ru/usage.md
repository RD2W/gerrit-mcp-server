# Использование

Назад: [← Установка](./installation.md)
Далее: [Архитектура →](./architecture.md)

---

## Версия

```bash
gerrit-mcp --version
```

Вывод включает версию, автора, хэш коммита, дату сборки и целевую платформу.

> **Примечание для Docker:** при ручной сборке без `--build-arg GIT_HASH`
> хэш коммита будет показан как `unknown`. Для корректного отображения
> передавайте хэш при сборке:
>
> ```bash
> docker build --build-arg GIT_HASH=$(git rev-parse HEAD) .
> ```

## Справочник по конфигурации

Файл конфигурации: `config/config.toml`. Аннотированный шаблон: `config/config.example.toml`.
Переменные окружения переопределяют отдельные поля (перечислены ниже).

### `[gerrit]` — подключение

| Поле | Env var | По умолчанию | Описание |
|---|---|---|---|
| `base_url` | `GERRIT_URL` | `""` | **Обязательно.** Базовый URL Gerrit (например, `https://gerrit.example.com`) |
| `timeout_secs` | — | `30` | Таймаут HTTP-запроса в секундах |
| `ca_cert` | `GERRIT_CA_CERT` / `SSL_CERT_FILE` | — | Путь к PEM-файлу пользовательского CA |
| `ca_cert_dir` | `SSL_CERT_DIR` | — | Директория с CA-сертификатами |
| `verify_ssl` | `GERRIT_VERIFY_SSL=false` | `true` | Включение/отключение проверки TLS |

### `[gerrit.auth]` — аутентификация

| Поле | Описание |
|---|---|
| `mode` | Режим: `"http_basic"` / `"basic"`, `"bearer"` / `"token"`, `"git_cookies"` или `"none"` |
| `username_env` | Имя переменной окружения для имени пользователя Basic Auth (например, `GERRIT_USERNAME`) |
| `auth_token_env` | Имя переменной окружения для HTTP-пароля/токена (по умолчанию: `GERRIT_AUTH_TOKEN`) |
| `token_env` | Имя переменной окружения для Bearer-токена (по умолчанию: `GERRIT_TOKEN`) |
| `gitcookies_path` | Путь к файлу `.gitcookies` для аутентификации в Gerrit (формат Netscape) |

Учётные данные никогда не хранятся в файле конфигурации — только имена переменных окружения.

### `[service]` — поведение

| Поле | По умолчанию | Описание |
|---|---|---|
| `default_max_results` | `25` | Лимит результатов по умолчанию, если клиент не указал |
| `read_only` | `false` | Запрет всех операций записи (env: `READ_ONLY_MODE`) |

### `[cache]` — кэш в памяти

| Поле | По умолчанию | Описание |
|---|---|---|
| `enabled` | `false` | Включение/отключение TTL + LRU кэша |
| `ttl_secs` | `300` | Время жизни записи в секундах |
| `max_entries` | `1000` | Максимальное количество записей (LRU-вытеснение) |

### `[rate_limit]` — ограничение частоты (token bucket)

| Поле | По умолчанию | Описание |
|---|---|---|
| `enabled` | `false` | Включение/отключение ограничения |
| `requests_per_second` | `10` | Устойчивая частота запросов |
| `burst` | `20` | Ёмкость всплеска |

### `[transport]` — режим сервера

| Поле | По умолчанию | Описание |
|---|---|---|
| `mode` | `"both"` | `"stdio"`, `"http"` или `"both"` |
| `bind_addr` | `"127.0.0.1:8080"` | Адрес для HTTP (используйте `0.0.0.0:8080` для сетевого доступа) |
| `http_path` | `"/mcp"` | Путь эндпоинта MCP Streamable HTTP |
| `health_path` | `"/healthz"` | Эндпоинт живучести |
| `ready_path` | `"/readyz"` | Эндпоинт готовности |
| `metrics_path` | `"/metrics"` | Эндпоинт метрик Prometheus |
| `allowed_hosts` | — | Разрешённые значения заголовка Host (защита от DNS rebinding) |
| `mcp_auth_token` | `""` | Опциональный Bearer-токен для аутентификации на MCP-эндпоинте (пустая строка = отключено) |

### `[log]`

| Поле | По умолчанию | Описание |
|---|---|---|
| `level` | `"info"` | `trace`, `debug`, `info`, `warn`, `error` — переопределяется `RUST_LOG` |

---

## Режимы транспорта

### Режим stdio

```toml
[transport]
mode = "stdio"
```

Сервер читает MCP-сообщения из stdin и пишет в stdout. Используйте:

- С `docker exec` для контейнерных Gerrit sidecar-ов
- С Claude Desktop или другими локальными MCP-клиентами
- Для отладки — легко передать тестовые JSON-сообщения через pipe

### Режим HTTP (Streamable HTTP)

```toml
[transport]
mode = "http"
bind_addr = "0.0.0.0:8080"
```

Сервер запускает HTTP-сервер с:

- MCP-эндпоинтом по настроенному `http_path` (`/mcp`)
- Проверкой живучести на `/healthz`
- Проверкой готовности на `/readyz`
- Метриками Prometheus на `/metrics`

Используйте для многоклиентского развёртывания, удалённого доступа или когда
MCP-клиент не поддерживает запуск процессов.

### Режим both

```toml
[transport]
mode = "both"
```

Запускает stdio и HTTP одновременно. Полезно для отладки HTTP-развёртываний:
канал stdio позволяет инспектировать трафик, пока HTTP-сервер обрабатывает
боевую нагрузку.

---

## Токен-аутентификация MCP-эндпоинта

Когда `mcp_auth_token` задан (непустая строка), HTTP-транспорт требует
валидный Bearer-токен в каждом запросе к MCP-эндпоинту. Сравнение токена
использует равенство за константное время для предотвращения timing-атак.

```toml
[transport]
mode = "http"
mcp_auth_token = "мой-секретный-токен"
```

Клиенты обязаны включать заголовок:
```
Authorization: Bearer мой-секретный-токен
```

Запросы без валидного токена получают **401 Unauthorized**. Health и metrics
эндпоинты не затрагиваются — защищён только MCP-эндпоинт.

---

## Защита от DNS rebinding

В режиме HTTP rmcp проверяет заголовок `Host` по списку `allowed_hosts`.
Настройте его под своё развёртывание:

```toml
# Docker — клиенты подключаются по имени контейнера
allowed_hosts = ["localhost", "127.0.0.1", "gerrit-mcp", "gerrit-mcp:8080"]

# Публичное развёртывание за обратным прокси
allowed_hosts = ["localhost", "mcp.example.com"]
```

---

## Режим «только чтение»

При включении сервер блокирует все операции записи — создание, обновление,
удаление, отправку на слияние, cherry-pick и прочие изменяющие действия.
Инструменты чтения остаются полностью доступными.

```toml
[service]
read_only = true
```

Или через переменную окружения:

```bash
READ_ONLY_MODE=true
```

Заблокированные инструменты возвращают сообщение об ошибке:
`"Cannot create change in read-only mode."`

Используйте для CI-пайплайнов, аудит-сред или любых сценариев, где LLM
должна иметь право только на чтение данных из Gerrit.

---

## Health-эндпоинты

| Эндпоинт | Поведение |
|---|---|
| `GET /healthz` | Всегда `200 OK`, если процесс жив |
| `GET /readyz` | `200`, когда конфигурация загружена и процесс готов; базовый сигнал готовности |
| `GET /metrics` | Текстовый формат Prometheus — счётчики вызовов инструментов, ошибок, запросов, uptime |

### Проверка здоровья в Docker

```yaml
healthcheck:
  test: ["CMD", "wget", "-qO-", "http://localhost:8080/healthz"]
  interval: 30s
  retries: 3
```

---

## Справочник инструментов MCP

Сервер предоставляет **32 инструментов**, покрывающих всё Gerrit REST API.

### Запрос изменений

| Инструмент | Описание | Основные параметры |
|---|---|---|
| `query_changes` | Поиск изменений с синтаксисом Gerrit | `query`, `limit?`, `options?` |
| `query_changes_by_date_and_filters` | Поиск изменений в диапазоне дат с фильтрами | `start_date`, `end_date`, `project?`, `message_substring?`, `status?`, `limit?` |
| `get_change_details` | Детальная информация об изменении (ревизии, метки, ревьюеры) | `change_id`, `options?` |
| `get_most_recent_cl` | Последнее изменение от пользователя | `user` |
| `changes_submitted_together` | Изменения, отправленные вместе с данным | `change_id`, `options?` |

### Содержимое изменений

| Инструмент | Описание | Основные параметры |
|---|---|---|
| `get_commit_message` | Дословное сообщение коммита для изменения (`GET /changes/{id}/message`; на Gerrit < 3.10 — фолбэк на revision commit endpoint) | `change_id` |
| `get_revision_commit` | Полный объект коммита ревизии | `change_id`, `revision_id?` |
| `get_related_changes` | Изменения, связанные с ревизией (цепочка зависимостей) | `change_id`, `revision_id?` |
| `get_git_parent_changes` | Родительские изменения (`parentof:`-запрос) | `change_id`, `limit?` |
| `list_change_files` | Список изменённых файлов | `change_id` |
| `get_file_diff` | Diff для файла в изменении | `change_id`, `file_path` |
| `list_change_comments` | Опубликованные комментарии | `change_id` |
| `list_draft_comments` | Черновики комментариев | `change_id` |
| `get_bugs_from_cl` | Извлечение ссылок на баги из изменения | `change_id` |

### Жизненный цикл изменений

| Инструмент | Описание | Основные параметры |
|---|---|---|
| `create_change` | Создать новое изменение | `project`, `branch`, `subject`, `topic?`, `status?` |
| `set_ready_for_review` | Пометить как готовое к ревью | `change_id` |
| `set_work_in_progress` | Пометить как work-in-progress | `change_id`, `message?` |
| `set_topic` | Установить тему изменения; пустой `topic` удаляет её | `change_id`, `topic` |
| `abandon_change` | Отказаться от изменения | `change_id`, `message?` |
| `revert_change` | Откатить принятое изменение | `change_id`, `message?` |
| `revert_submission` | Откатить отправку | `change_id`, `message?` |
| `submit_change` | Отправить изменение на слияние | `change_id`, `wait_for_merge?` |

### Code review

| Инструмент | Описание | Основные параметры |
|---|---|---|
| `add_reviewer` | Добавить ревьюера | `change_id`, `reviewer`, `state?`, `confirmed?` |
| `suggest_reviewers` | Предложения ревьюеров | `change_id`, `query`, `limit?`, `exclude_groups?` |
| `set_labels` | Установить голоса меток на изменение | `change_id`, `labels`, `message?`, `gerrit_base_url?` |
| `post_review_comment` | Опубликовать комментарий ревью | `change_id`, `file_path`, `line_number`, `message`, `unresolved?`, `labels?` |
| `post_draft_comment` | Опубликовать черновик (диапазон, unresolved, встроенный suggestion) | `change_id`, `file_path`, `line_number`, `message`, `unresolved?`, `suggestion?`, `in_reply_to?`, `start_line?`, `start_character?`, `end_line?`, `end_character?` |
| `delete_draft_comment` | Удалить конкретный черновик | `change_id`, `draft_id` |
| `delete_draft_comments` | Удалить все черновики изменения | `change_id` |
| `publish_drafts` | Опубликовать черновики (отправляет `drafts=PUBLISH_ALL_REVISIONS`) | `change_id`, `message?`, `labels?` |

### Cherry-pick

| Инструмент | Описание | Основные параметры |
|---|---|---|
| `cherry_pick_change` | Перенести изменение в другую ветку | `change_id`, `destination`, `revision_id?`, `message?`, `keep_reviewers?`, `allow_conflicts?`, `allow_empty?` |
| `cherry_pick_chain` | Перенести цепочку изменений | `change_id`, `destination`, `revision_id?`, `keep_reviewers?`, `allow_conflicts?`, `allow_empty?` |

### Формат результатов

Все результаты поиска включают:

```json
{
  "results": [...],
  "total_hits": 1423,
  "page": 1,
  "has_more": true,
  "duration_ms": 230
}
```

- `has_more: true` сообщает LLM, что доступны дополнительные результаты — можно запросить следующую страницу
- `duration_ms` — время обработки на стороне сервера, полезно для отладки задержек

### Пагинация

Когда `has_more` равно `true`, клиент может запросить дополнительные страницы,
установив параметр `offset`:

```
Tool: query_changes
Query: "status:open"
Project: "aosp"
Offset: 25
```

Сервер прозрачно обрабатывает механику пагинации Gerrit и предоставляет
простой интерфейс на основе страниц.
