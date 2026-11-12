# Правила для людей и нейросетей

- Код и идентификаторы на английском; тексты для людей и документация на русском.
- `mw-domain` без ввода-вывода. К БД обращаться только через `mw-db` и `mw-core`; права проверяет `mw-core`.
- Сначала тест, потом код. Каждый тест с БД идёт по `all_backends()`.
- Перед коммитом должно быть зелёным:
  `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`.
  `cargo check` собирает без dev-зависимостей: рабочий код не должен пользоваться тем, что открыто только для тестов.
  PostgreSQL в тестах включается переменной `MW_TEST_PG_URL`
  (локально: `docker run -d --name mw-pg -e POSTGRES_PASSWORD=postgres -p 55432:5432 postgres:17`,
  `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`).
- Деньги — только `Money`, время — только `Core::now()` (местное время предприятия) и `Core::now_utc()` (интервалы, ограничение попыток), поиск без учёта регистра — только `mw_domain::search`.
- Выпущенные миграции не меняются, только добавляются новые.
- Новое право или изменение стартовых данных — только новой миграцией данных; выпущенные миграции не меняются.
- Любая запись в БД — только через `Core::begin_write()`.
- `legacy/` только для чтения.
- Никаких секретов и реальных данных клиентов в коде и тестах.
- Спецификация и планы лежат в `docs/superpowers/`.
- Зависимости — только с лицензиями MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode; MPL и копилефт не брать (`dirs` убран из-за крейта под MPL-2.0).
- Типы API — это типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS` и `utoipa::ToSchema`. Входные типы — с `deny_unknown_fields`; в правках различие «не передано» и `null` — через `patch::present`/`patch::nullable`. `#[ts(export)]` не ставить.
- `crates/mw-api/openapi.json` и `crates/mw-api/bindings/` генерируются, руками не правятся. После изменения API:
  `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`, затем коммит вместе с кодом.
  Один поток нужен, потому что тесты делят файлы; внутри обновления `every_openapi_schema_has_a_typescript_type` может упасть один раз
  (проверяет привязки до их перегенерации) — тогда запустить `cargo test -p mw-api --test contract` без переменной, он должен пройти.
- `Actor::for_tests` и `Core::db()` — только в тестах (фича `testing`). Рабочий код получает `Actor` из `Core::authenticate` или `Actor::unrestricted`.
- Правка без изменений не создаёт версию и запись в журнал; устаревшая `version` в такой правке не даёт 409.
- Ограничитель попыток входа считает попытку при допуске, а не по результату: параллельными запросами его не обойти.
