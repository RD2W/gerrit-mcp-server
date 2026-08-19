# Разработка

Назад: [← Архитектура](./architecture.md)

---

## Начало работы

```bash
git clone <repo-url> gerrit-mcp-server
cd gerrit-mcp-server
cargo build --workspace
cargo test --workspace
```

Разработка ведётся в ветке `dev`. Создавайте feature-ветки от неё:

```bash
git checkout dev
git checkout -b feat/моя-фича
```

---

## Запуск тестов

```bash
# Все тесты (338). Используйте --test-threads=1 во избежание гонок env-переменных
cargo test --workspace -- --test-threads=1

# Отдельный крейт
cargo test -p gerrit-core
cargo test -p gerrit-mcp

# С выводом
cargo test -- --nocapture

# Запуск игнорируемых (интеграционных) тестов
cargo test -- --ignored
```

---

## CI-пайплайн

CI запускается при каждом пуше в ветки `dev`, `main`, `ci` и для всех PR:

| Задача | Команда | Назначение |
|---|---|---|
| Форматирование | `cargo fmt --all -- --check` | Единый стиль кода |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | Поиск ошибок и стилистических проблем |
| Тесты | `cargo test --workspace --locked` | Запуск всех модульных и интеграционных тестов |
| Сборка | `cargo build --workspace --locked --release` | Проверка компиляции release-сборки |

Workflow GitHub Actions: `.github/workflows/ci.yml`

---

## Правила оформления кода

### Общие

- **Язык:** английские комментарии и сообщения коммитов
- **Коммиты:** [Conventional Commits](https://www.conventionalcommits.org/) —
  `feat:`, `fix:`, `chore:`, `docs:`, `test:`, `refactor:`
- **Форматирование:** `rustfmt` с настройками по умолчанию
- **Линтинг:** `clippy` с `-D warnings` — все предупреждения считаются ошибками в CI

### SPDX-заголовки

Каждый новый `.rs` файл должен начинаться с:

```rust
// SPDX-License-Identifier: GPL-3.0-or-later
```

Точный формат смотрите в любом существующем исходном файле.

### Организация модулей

- Небольшие, сфокусированные файлы с чёткими обязанностями
- Один основной тип или задача на файл
- `mod.rs` только для объявлений модулей и реэкспорта

---

## Добавление нового инструмента MCP

1. **Добавьте доменный тип** в `gerrit-core/src/domain.rs`, если API возвращает
   новую форму ответа.

2. **Добавьте метод типажа** в `GerritRepository` в `domain.rs` и реализуйте
   его в `infrastructure/client.rs`.

3. **Проведите через декоратор** в `application.rs` — `GerritService` делегирует
   внутреннему репозиторию.

4. **Определите тип параметров** в `gerrit-mcp/src/mcp/tools.rs` с
   derive-макросами `schemars` для генерации JSON Schema.

5. **Реализуйте обработчик** в соответствующем модуле `mcp/`
   (`changes.rs`, `reviews.rs` или `comments.rs`):
   ```rust
   #[tool(description = "Получение изменения по ID")]
   async fn get_change_details(
       &self,
       change_id: String,
       ...
   ) -> Result<CallToolResult, McpError> {
       // …
   }
   ```
   Используйте `#[param(description = "...")]` для каждого параметра — эти описания
   передаются LLM-клиентам и напрямую влияют на качество вызовов инструментов.

6. **Зарегистрируйте обработчик** в `gerrit-mcp/src/mcp/mod.rs` — инструмент
   автоматически обнаруживается через макрос `#[tool]`.

7. **Добавьте тесты** — модульные тесты для доменного типа, тесты с моком для
   обработчика и интеграционные тесты для HTTP-клиента.

---

## Документация

При изменении поведения обновляйте:

- Соответствующие страницы в `docs/en/` и `docs/ru/` (поддерживайте синхронизацию)
- `README.md`, если изменения затрагивают быстрый старт или список возможностей
- `CHANGELOG.md` — добавляйте запись в раздел `[Unreleased]`
- `config/config.example.toml` и `config/.env.example` — если меняются параметры конфигурации.
  Каждое поле конфигурации должно иметь соответствующую env-переменную в `config.rs::apply_env_overrides()`.
  Соглашение об именах: `GERRIT_*` для секции `[gerrit]`, `MCP_*` для серверных настроек.
  Булевы env-переменные принимают `true`/`1`/`yes` (включить) и `false`/`0`/`no` (выключить).

---

## Чеклист перед PR

Перед открытием pull request выполните:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --release
```

Все четыре должны проходить. CI проверяет первые три — проверка сборки
отлавливает проблемы компиляции, которые тесты могут пропустить.

---

## Процесс релиза

Релизы автоматизированы через `.github/workflows/release.yml`:

1. Отправьте тег, например `v1.3.0`
2. CI собирает мультиархитектурные Docker-образы и создаёт GitHub Release
3. Бинарные артефакты прикрепляются к релизу

Ручные шаги (для отладки workflow):

```bash
docker build -t gerrit-mcp:v1.3.0 .
docker tag gerrit-mcp:v1.3.0 gerrit-mcp:latest
```
