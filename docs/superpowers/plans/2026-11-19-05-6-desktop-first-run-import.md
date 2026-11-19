# План 5.6. Десктоп, первый запуск и импорт — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** программа для компьютера (Tauri 2) со встроенным сервером на `127.0.0.1:0`; первый запуск — «Начать с
пустой базы» или «Перенести из MyWork 2»; импорт из интерфейса в процессе сервера с ходом по этапам и отчётом;
прошлые импорты в «Настройках».

**Architecture:** импорт остаётся в `mw-legacy::import_folder`; он получает обратный вызов хода по этапам, а
`mw-api` запускает его фоновой задачей (`tokio::spawn`) и отдаёт состояние по опросу. Импорт по пути на диске
сервера разрешён только серверу на loopback (локальный режим и десктоп); загрузка папки по сети — план 9. Первый
запуск определяется сервером (`setup_required`). Десктоп — отдельный Cargo-workspace `apps/desktop/src-tauri`,
зависит от `mw-server` по пути, поднимает `Server` и открывает окно на его адрес; интерфейс тот же, общение — HTTP;
через Tauri идут только выбор папки и события окна.

**Tech Stack:** Rust (Axum 0.8, SeaORM 2, tokio), Tauri 2 (`tauri`, `tauri-plugin-dialog`), `tracing-appender`,
SvelteKit 3 / Svelte 5, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` §6.8 («Импорт из MyWork 2», «О программе»),
§6.9; основная спека `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` §4 (главное правило HTTP,
режимы), «Где лежат данные» (папки ОС, `portable.txt`, `logs/`), §8 (сборка); заметки «План 5.6» в README.

## Global Constraints

- Всё из `AGENTS.md` (сервер и «Интерфейс»); `legacy/` и папка MyWork 2 — только чтение; реальные базы не попадают
  в репозиторий, фикстуры (только `mw_legacy::fixture`), тесты и логи.
- Зависимости — MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode; у новых Rust-крейтов лицензию проверить
  (`cargo metadata`) и перечислить в отчёте; `@tauri-apps/*` — в `dependencies` интерфейса, проверка `npm run licenses`.
- Интерфейс всегда говорит с сервером по HTTP; Tauri — только выбор папки, события окна и открытие папки в системе.
- Папка данных: Windows `%APPDATA%\MyWork3`, macOS `~/Library/Application Support/MyWork3`, Linux
  `~/.local/share/MyWork3`; `portable.txt` рядом с исполняемым файлом → `./data`; логи — `<папка данных>/logs/`.
- Проверки: Rust основного workspace — как в `AGENTS.md` (обе СУБД); десктоп — `cargo fmt --check && cargo clippy
  --all-targets -- -D warnings && cargo test` в `apps/desktop/src-tauri`; интерфейс — `npm run verify && npm run test:e2e`.
- Коммиты по-русски, последняя строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Импорт прерван посередине (закрыли окно, упал сервер): база остаётся как до импорта (одна транзакция), файлы —
   в промежуточной папке, которая удаляется при следующем запуске; повторный импорт возможен.
2. Папка выбрана не та (нет `baza.sqlite`, пустая, сама папка данных MyWork 3, папка внутри папки MyWork 2):
   «Проверить» говорит, что не так, по-русски, и не начинает перенос.
3. Два окна/вкладки запускают импорт одновременно: второй получает «Импорт уже идёт» и видит тот же ход.
4. Первый запуск на базе, где уже есть заказы (например, после импорта командой): экран первого запуска не
   показывается.
5. Десктоп запускают второй раз, пока первый открыт: вторая копия не поднимает второй сервер на той же базе
   (одна копия на папку данных — фокус на открытое окно или понятное сообщение).

---

### Task 1: Импорт в процессе сервера — ход, проверка папки, отчёты

**Files:** `crates/mw-legacy/src/{lib.rs,folder.rs,files.rs}`, `crates/mw-core/src/import/*` (список и чтение
отчётов), `crates/mw-api/src/routes/legacy_import.rs` (новый), `crates/mw-api/src/{state.rs,routes/mod.rs}`,
`crates/mw-server/src/import.rs` (общий код отчётов), контракт; тесты `crates/mw-legacy/tests/`, `crates/mw-api/tests/`.

**Interfaces — Produces:**
```rust
// mw-legacy
pub enum ImportStage { Reading, Converting, Copying, Writing, Placing, Done }   // code_enum "reading"…; Serialize, TS, ToSchema
pub type Progress = std::sync::Arc<dyn Fn(ImportStage, u8 /* 0..=100 внутри этапа */) + Send + Sync>;
// ImportOptions { …, progress: Option<Progress> }
pub struct FolderCheck { pub ok: bool, pub problem: Option<String>, pub schema_version: Option<String>,
  pub orders: u64, pub clients: u64, pub files: u64, pub files_bytes: u64 }        // Serialize, TS, ToSchema
pub async fn check_folder(source: &Path, data_dir: &Path) -> FolderCheck;          // только чтение, без записи
// mw-api, HTTP (право settings_manage + users_manage, как import_folder; только сервер на loopback, иначе 403
// «Импорт по пути доступен только на этом компьютере»)
POST /api/v1/legacy-import/check   { path }                 -> FolderCheck
POST /api/v1/legacy-import         { path, replace: bool }  -> ImportJob   (202; второй запуск во время первого → 409 «Импорт уже идёт»)
GET  /api/v1/legacy-import/current                          -> Option<ImportJob>
GET  /api/v1/legacy-imports                                  -> Vec<LegacyImportItem { id, imported_at, source_path, mode, orders, warnings }>
GET  /api/v1/legacy-imports/{id}                             -> ImportReportView
pub struct ImportJob { pub stage: ImportStage, pub percent: u8, pub error: Option<String>, pub import_id: Option<i64> }
pub struct ImportReportView { /* итоги: заказы, клиенты, пользователи, файлы (скопировано, байт, не найдено);
  groups: Vec<ReportGroup { code, title, count, order_ids: Vec<i64> }> — по WarningCode, order_ids — из Warning.key,
  если источник — заказы; warnings_text — тот же текст, что пишет команда (для «Сохранить отчёт») */ }
```
Правила: фоновая задача — `tokio::spawn`, файловые операции — `spawn_blocking`; ход хранится в `AppState`
(одна задача на сервер); по окончании — запись в `legacy_imports` (как сейчас) и файлы отчёта в
`<данные>/import-reports` (общий код с командой `import-legacy`); ошибка — `error` по-русски (через
`import_error_message`). Кэш настроек после импорта — уже через `commit_with_settings`. Промежуточная папка
копирования файлов, оставшаяся после обрыва, удаляется при старте сервера.

- [ ] Step 1: тесты — `check_folder` на фикстуре (`mw_legacy::fixture`): числа совпадают; нет `baza.sqlite` →
  `ok: false` с «В папке нет baza.sqlite»; папка данных внутри папки MyWork 2 → «Папка данных MyWork 3 лежит внутри
  папки MyWork 2»; ход: обратный вызов получает этапы по порядку и `Done` последним; API: 202 → опрос до `Done`,
  `import_id` есть, список и отчёт отдаются, группы с `order_ids`; второй запуск → 409; не-loopback → 403; без права
  → 403; непустая база без `replace` → ошибка с подсказкой; брошенная промежуточная папка удаляется при старте.
- [ ] Step 2: падают. Step 3: реализация, контракт. Step 4: полные проверки (обе СУБД). Step 5: Commit
  `feat(legacy,api): импорт из интерфейса — проверка папки, ход по этапам, отчёты`.

---

### Task 2: Первый запуск на сервере, логи в файл, e2e-сервер

**Files:** `crates/mw-core/src/{settings.rs,setup.rs (новый)}`, `crates/mw-api/src/routes/{meta.rs,setup.rs (новый)}`,
`crates/mw-server/src/{logging.rs (новый),cli.rs}`, `apps/ui/e2e/support/server.mjs`, контракт; тесты.

**Interfaces — Produces:**
```rust
// ServerInfo: + setup_required: bool   // нет отметки «настроено» И база пуста (нет заказов, клиентов, импортов)
pub struct SetupRequest { pub firm_name: String, pub point_name: String, pub masters: Vec<String> } // deny_unknown_fields
POST /api/v1/setup  SetupRequest -> 204   // право settings_manage; одна транзакция: firm_name, точка (и она же по
                                          // умолчанию), мастера в справочник (без повторов по normalize_search),
                                          // отметка setup_done = true; повтор после отметки → 409
// Импорт (Task 1) по успеху тоже ставит setup_done.
pub fn init_logging(log_dir: Option<&Path>) -> Result<LogGuard, ServerError>   // mw-server: stderr всегда, файл
  // `mywork.log` с ежедневной сменой (tracing-appender, хранить 14 файлов), если log_dir задан
```
`mw-server serve` получает `--log-dir` (по умолчанию `<данные>/logs`; `--log-dir none` — только stderr).
`server.mjs`: обработчики сигналов ставятся до `create-admin`; при выходе процесса-родителя (SIGKILL, обрыв stdin)
`mw-server` не остаётся сиротой — сервер запускается с `--exit-on-stdin-close` (новый флаг CLI, используется только
тестами и десктопом) и завершается, когда stdin закрыт.

- [ ] Step 1: тесты — `setup_required` true на новой базе, false после `POST /setup`, false на базе с заказом без
  отметки, false после импорта; `POST /setup` пишет всё в одной транзакции, мастера без повторов, повтор → 409, без
  права → 403; `init_logging` пишет строку в файл папки; `--exit-on-stdin-close` — процесс завершается при закрытии
  stdin (тест через `std::process::Command`). Step 2: падают. Step 3: реализация. Step 4: проверки (обе СУБД, e2e).
  Step 5: Commit `feat(core,api,server): первый запуск, логи в файл, сервер не остаётся сиротой`.

---

### Task 3: Интерфейс — первый запуск, мастер импорта, отчёт, «Настройки → Импорт»

**Files:** `apps/ui/src/routes/setup/+page.svelte` (+ `+page.ts`), `apps/ui/src/lib/setup/*` (форма, мастер импорта,
опрос хода), `apps/ui/src/lib/import/ReportView.svelte`, `apps/ui/src/routes/(app)/settings/import/+page.svelte`,
`apps/ui/src/lib/api/import.ts`, `apps/ui/src/lib/settings/sections.ts`, `(app)/+layout.ts` (переход на `/setup`),
`src/lib/i18n/ru.ts`, `src/lib/platform/folder.ts` (выбор папки: Tauri или поле ввода); e2e `e2e/setup.spec.ts`,
`e2e/import.spec.ts`, сцены в `e2e/layout.spec.ts`.

**Interfaces — Consumes:** Task 1 и Task 2 API. **Produces:**
`export async function pickFolder(): Promise<string | null>` (`#lib/platform/folder.js`): в Tauri — диалог выбора
папки (`@tauri-apps/plugin-dialog`, подключается в Task 4 — здесь только проверка `isTauri()` и запасной путь),
в браузере — `null` (интерфейс показывает поле «Путь к папке MyWork 2»).

Экраны (спека 6.9):
- `/setup` без каркаса, карточка по центру: «Начать с пустой базы» / «Перенести из MyWork 2». `(app)/+layout.ts`
  ведёт сюда, если `server-info.setup_required`.
- Пустая база: «Название сервиса», «Первая точка», «Мастера» (чипы, Enter добавляет) → `POST /setup` → `/orders`
  с подсказкой «Примите первое устройство (N)».
- Импорт: 1) папка («Выбрать папку…» в Tauri, поле пути в браузере); 2) «Проверить» → что нашлось (версия базы,
  заказы, клиенты, файлы и объём) или причина; 3) «Перенести»; 4) ход по этапам с полосой («Чтение базы»,
  «Разбор», «Копирование файлов», «Запись», «Установка файлов»), опрос `current` раз в секунду, ход переживает
  перезагрузку страницы; 5) отчёт.
- Отчёт (`ReportView`): плитки итогов; группы «Требует внимания» (раскрываются списком номеров заказов, щелчок
  открывает панель заказа); «Сохранить отчёт» (скачивание текста через `client.ts`/blob); «К заказам».
- «Настройки → Импорт из MyWork 2» (`import`, перед «О программе»): прошлые импорты (дата, папка, заказов,
  предупреждений) → отчёт; «Перенести снова» с заменой — `ConfirmDialog` «Данные будут заменены, перед этим
  сделается копия» (только SQLite; на PostgreSQL кнопки нет — из `about.database`).

- [ ] Step 1: тесты — unit (опрос хода, переход на `/setup`, разбор групп отчёта); e2e на отдельном сервере с
  пустой базой: e2e-конфиг получает третий сервер `4175` (новая папка данных на каждый запуск) — пустая база →
  `/setup` → «Начать с пустой базы» → форма → `/orders` с подсказкой; второй прогон этого файла — на той же базе
  `/setup` не показывается; импорт: фикстура MyWork 2 генерируется в tmp готовым бинарником `make_fixture` из `mw-legacy`
  (`--features fixtures`; локально `server.mjs` собирает его через cargo, в CI его собирает и выкладывает
  артефактом задание `e2e-server`, путь — `MW_E2E_FIXTURE_BIN`) → путь в поле → «Проверить» →
  числа → «Перенести» → ход → отчёт с группами → щелчок по номеру открывает панель; неверная папка → причина.
- [ ] Step 2: падают. Step 3: реализация; сцены `setup`, `import-report`, `settings-import` в обходе. Step 4:
  проверки. Step 5: Commit `feat(ui): первый запуск, перенос из MyWork 2 с ходом и отчётом`.

---

### Task 4: Программа для компьютера (Tauri 2)

**Files:** `apps/desktop/package.json` (`@tauri-apps/cli` в devDependencies), `apps/desktop/src-tauri/{Cargo.toml,
build.rs,tauri.conf.json,capabilities/default.json,icons/*,src/{main.rs,lib.rs,launch.rs,single.rs}}`,
`apps/ui/src/lib/platform/{folder.ts,window.ts}`, `apps/ui/src/lib/design/prefs.svelte.ts` (события окна),
`.github/workflows/ci.yml` (задание `desktop`), `README.md` (как запустить), `docs/superpowers/specs/...` (§11 решения).

**Interfaces — Consumes:** `mw_server::{Server, Config, paths::resolve_data_dir, logging::init_logging}`, флаг
сборки интерфейса. **Produces:** `launch::start(data_dir: &Path, ui_dir: &Path) -> Result<Running, ServerError>`
(`Running { url: String, shutdown: … }`), `launch::data_dir(exe_dir: &Path) -> PathBuf` (портативный режим).

Решения:
- Отдельный workspace (`apps/desktop/src-tauri` со своим `[workspace]`), чтобы `cargo check --workspace` основного
  проекта не требовал WebKitGTK на Linux; `mw-server` — зависимость по пути.
- Окно открывается на `http://127.0.0.1:<порт>/` (`WebviewUrl::External`); `capabilities` разрешают IPC только
  этому адресу (`remote.urls: ["http://127.0.0.1:*"]`) и только `dialog:allow-open`, события окна.
- Интерфейс в сборке — ресурсы Tauri (`bundle.resources`: `../../ui/build` → `ui/`); `beforeBuildCommand` собирает
  `apps/ui`; путь к ресурсам — `app.path().resource_dir()`.
- Масштаб страницы Tauri выключен (по умолчанию); `prefs.svelte.ts` в Tauri слушает `onMoved`/`onScaleChanged`
  окна (через `#lib/platform/window.js`) и пересчитывает окружение — «Авто» меняется при переезде на другой монитор.
- Одна копия на папку данных: файл-замок `<данные>/mywork.lock` (`fs4`/`fd-lock` с лицензией MIT/Apache, или
  `tauri-plugin-single-instance`); вторая копия показывает окно первой или сообщает «Программа уже запущена».
- Закрытие окна → `Server` останавливается штатно (`shutdown` + `wait_background` 30 с), затем выход.
- Логи — `init_logging(Some(<данные>/logs))`; паника и ошибка запуска — окно с текстом и путём к логам.
- Иконки — простая заглушка из `apps/desktop/icon.svg` через `tauri icon` (лицензия своя).
- CI: задание `desktop` (macOS, Windows, Ubuntu 24.04 с `libwebkit2gtk-4.1-dev libsoup-3.0-dev librsvg2-dev
  libayatana-appindicator3-dev`, установка с повтором как раньше): `cargo fmt --check`, `clippy -D warnings`,
  `cargo test` в `apps/desktop/src-tauri`; полной сборки установщиков нет (план 11).

- [ ] Step 1: тесты (без окна) — `launch::data_dir` с `portable.txt` и без; `launch::start` на временной папке
  поднимает сервер, `GET /server-info` отвечает, остановка закрывает базу; замок: вторая попытка взять замок на ту же
  папку не удаётся; UI unit — `pickFolder` в браузере → `null`, в «Tauri» (подмена `window.__TAURI_INTERNALS__`) →
  путь из подменённого диалога; подписка на события окна снимается.
- [ ] Step 2: падают. Step 3: реализация; `npm run tauri build -- --debug` на Mac собирает `.app` (не в CI);
  README — «Запуск программы для компьютера». Step 4: проверки (desktop, основной workspace, интерфейс).
- [ ] Step 5: Commit `feat(desktop): программа для компьютера на Tauri со встроенным сервером`.

---

### Task 5: Вкладки и сеанс — истечение отдельно от входа другого

**Files:** `apps/ui/src/lib/api/session.ts`, `apps/ui/src/lib/i18n/ru.ts`, тесты; README (заметки 5.6 → закрыть,
мобильную — в «План 10»).

Пункты:
1. Истечение сеанса во вкладке пишет в хранилище метку (`{ expired: true, user_id }`), другие вкладки показывают
   «Сеанс истёк — войдите снова», а не «вход другим пользователем»; тест на каждый переход.
2. Вкладка без «Запомнить» (`sessionStorage`) сбрасывается, когда в другой вкладке входит ДРУГОЙ пользователь
   (общий компьютер: прежний сотрудник не должен оставаться открытым); тот же пользователь — не трогается; тест.
3. Токен сеанса, потерянного вкладкой, отзывается: при сбросе вкладка отправляет `POST /auth/logout` со старым
   токеном, если он ещё был (`keepalive`), ошибка не мешает; тест.
4. README: заметки «План 5.6» закрыты; про `pagehide` на телефоне — в «План 10»; про масштаб страницы в браузере —
   «решено: в Tauri выключен».

- [ ] Step 1: тесты → падают. Step 2: реализация. Step 3: проверки. Step 4: Commit
  `fix(ui): вкладки различают истечение сеанса и вход другого, старый токен отзывается`.
