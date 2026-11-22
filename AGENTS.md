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
- Вложения: файлы — только через `mw-core::attachments`, пути — через `store::resolve`. Загрузка — телом `application/octet-stream`; `multipart/form-data` и прочие «простые» типы запрещены (защита от CSRF).
- Спецификация и планы лежат в `docs/superpowers/`.
- Зависимости — только с лицензиями MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode; MPL и копилефт в поставляемую программу не брать (`dirs` убран из-за крейта под MPL-2.0). Исключения:
  - OFL-1.1 — только для шрифтов (`@fontsource*`, шрифты документов `crates/mw-docs/fonts`);
  - MPL-2.0 — только для инструментов сборки, которые не попадают в программу: сейчас `lightningcss` (минификатор CSS в Vite), исключён по имени в `apps/ui/scripts/check-licenses.mjs`.
  Всё, что попадает в сборку интерфейса, лежит в `dependencies`; их лицензии проверяет `npm run licenses`.
  Программа для компьютера: `option-ext` (MPL-2.0, его тянет Tauri через `dirs`) заменён своей реализацией под MIT в `apps/desktop/src-tauri/vendor/option-ext` (`[patch.crates-io]`); лицензии того, что входит в программу (обычные зависимости на всех ОС), проверяет `npm run licenses` в `apps/desktop`.
- После изменения зависимостей в `crates/*` обновить `apps/desktop/src-tauri/Cargo.lock` (`cargo update -p <крейт>` или `cargo check` в `apps/desktop/src-tauri`): поставляемая программа должна собираться с теми же версиями.
- Типы API — это типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS` и `utoipa::ToSchema`. Входные типы — с `deny_unknown_fields`; в правках различие «не передано» и `null` — через `patch::present`/`patch::nullable`. `#[ts(export)]` не ставить.
- `crates/mw-api/openapi.json` и `crates/mw-api/bindings/` генерируются, руками не правятся. После изменения API:
  `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`, затем коммит вместе с кодом.
  Один поток нужен, потому что тесты делят файлы; внутри обновления `every_openapi_schema_has_a_typescript_type` может упасть один раз
  (проверяет привязки до их перегенерации) — тогда запустить `cargo test -p mw-api --test contract` без переменной, он должен пройти.
- `Actor::for_tests` и `Core::db()` — только в тестах (фича `testing`). Рабочий код получает `Actor` из `Core::authenticate` или `Actor::unrestricted`.
- Документы (`mw-docs`): данные заказа никогда не становятся разметкой Typst — только `data.json` и виртуальный `barcode.svg`.
  Рендерит их только `Core::export_document` (под семафором), сам `mw-docs` из API и интерфейса не зовут.
- Печать из программы для компьютера — только командами Tauri из `print/`; имя принтера передаётся в `lp` одним аргументом.
- Правка без изменений не создаёт версию и запись в журнал; устаревшая `version` в такой правке не даёт 409.
- Ограничитель попыток входа считает попытку при допуске, а не по результату: параллельными запросами его не обойти.
- Любая правка построителей `search_text` (`mw-core/src/search_index.rs`), `normalize_search`, `index_text` или нормализации телефонов (`mw_domain::phone`) увеличивает `SEARCH_INDEX_VERSION`: `Core::open` пересчитает индекс сам.
- Импорт MyWork 2 (`mw-legacy`): всё неразобранное попадает в отчёт и сохраняется в исходном виде (`legacy_rows`, `details.legacy`). Новая форма значения из старой базы — сначала строка в синтетической фикстуре с ожидаемым результатом в тесте, потом код разбора.
- Реальные базы клиентов не попадают в репозиторий, фикстуры, тесты и логи; фикстуры только генерируются кодом (`mw_legacy::fixture`). Проверка на настоящей папке — игнорируемый тест `real_db` (`MW_LEGACY_REAL_DIR`), он печатает только числа.
- Телефоны клиента — список `client_phones`, первый номер основной. Гарантия заказа — текст и выводимый срок.
- Интерфейс (`apps/ui`, SvelteKit + Svelte 5):
  - перед коммитом зелёным должно быть `cd apps/ui && npm run verify && npm run test:e2e`;
  - ширины раскладки — только `display` (`#lib/design/prefs.svelte.js`) и `:global([data-width=…])`; медиазапросы по ширине и `MediaQuery` запрещены (тест `no-width-media`); иконки растут с масштабом через `base.css`;
  - цвета, размеры, тени — только из `src/lib/design/tokens.css`; литералов цвета вне него нет (тест `no-raw-colors`);
  - Svelte 5 только на рунах; обработчики — пропсы `on…`;
  - в SvelteKit 3 нет `$lib`: импорты — `#lib/...` (`imports` в `package.json`) с явными расширениями (`.js` для модулей TS, `.svelte` для компонентов); типы API — только `import type` из `$api/...` (псевдоним tsconfig → `crates/mw-api/bindings/*`), руками не дублировать;
  - компоненты — из `#lib/ui/...`, новые — туда же, с разделом на витрине `/design`;
  - тексты — по-русски, общие строки — в `#lib/i18n/ru.ts`;
  - иконки — `@lucide/svelte`, по одной;
  - даты из API — местное время без смещения, только через `#lib/format/date.js`;
  - деньги — копейки, только через `#lib/format/money.js` и `MoneyInput`;
  - однобуквенные клавиши — через `registerHotkey`: в полях и при открытых слоях не срабатывают, буквы — по `code`;
  - `fetch` — только в `src/lib/api/client.ts`, данные — через `#lib/api/*.js`;
  - сквозные тесты — Chromium и WebKit, запросы по ролям и подписям; идут на настоящем `mw-server` (`e2e/support/server.mjs`), данные создают через API с меткой `uniq()`;
  - вид списка живёт в адресе (`viewToSearch`);
  - автосохранение — только через `createAutosave`;
  - картинки — только через `attachmentUrl` (`blob:`), не прямыми ссылками на API;
  - общие помощники: `createEntityEditor` (одна очередь записи на запись) и `createPagedList` (постраничный список) — новые экраны берут их, а не копируют;
  - камера в сквозных тестах — только проект Chromium (поддельная камера); в WebKit проверяется запасной путь;
  - Ctrl/⌘+Enter в формах диалогов — `submitShortcut`; «Выдать» из панели — `registerHotkey` без `allowWithOverlay`.
