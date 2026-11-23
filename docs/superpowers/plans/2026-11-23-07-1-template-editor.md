# План 7.1 «Редактор шаблонов» — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Цель:** свои шаблоны актов и наклейки хранятся в БД, общие для всех рабочих мест. Их правят в редакторе с живым превью и переносят файлом `.mwtemplate`.

**Архитектура:**
- Модель шаблона переезжает из `mw-docs` в `mw-domain` и становится типом API: TS, OpenAPI, `deny_unknown_fields`, проверка границ.
- Ядро хранит модель в таблице `document_templates` и печатает по ней. Шаблон, равный стандартному, хранится как «стандартный».
- Превью несохранённой модели идёт тем же `Core::export_document` под семафором.
- Редактор — экран SvelteKit с тремя колонками. Он состоит из:
  - чистых модулей на TS: операции над деревом блоков, история отмены, очередь превью;
  - автосохранения через `createEntityEditor`.

**Технологии:** Rust (SeaORM 2, Axum 0.8, Typst 0.15.1 в `mw-docs`), SvelteKit 3 + Svelte 5 (руны), Vitest, Playwright.

**Спека:**
- `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md`, §5.1 «Редактор актов и наклеек»;
- `docs/superpowers/specs/2026-11-15-ui-design-system-design.md`, раздел 7, пункт «Редактор шаблонов (план 7)»;
- заметки «План 7» в `docs/superpowers/plans/README.md`.

## Global Constraints

- Код и идентификаторы на английском; тексты для людей и документация на русском.
- `mw-domain` без ввода-вывода. К БД обращаться только через `mw-db` и `mw-core`; права проверяет `mw-core`. Любая запись в БД — только через `Core::begin_write()`.
- Сначала тест, потом код. Каждый тест с БД идёт по `all_backends()`.
- Перед коммитом зелёным должно быть: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`. Для интерфейса: `cd apps/ui && npm run verify && npm run test:e2e`.
- Время — только `Core::now()`. Поиск без учёта регистра — только `mw_domain::search`.
- Выпущенные миграции не меняются, только добавляются новые.
- Типы API — это типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS` и `utoipa::ToSchema`. Входные типы — с `deny_unknown_fields`. `#[ts(export)]` не ставить.
- `crates/mw-api/openapi.json` и `crates/mw-api/bindings/` генерируются: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`. Затем `cargo test -p mw-api --test contract`; коммит вместе с кодом.
- Документы: данные заказа никогда не становятся разметкой Typst — только `data.json` и виртуальный `barcode.svg`. Рендерит только `Core::export_document` (под семафором); `mw-docs` из API и интерфейса не зовут.
- Правка без изменений не создаёт версию и запись в журнал; устаревшая `version` в такой правке не даёт 409.
- Спека 5.1:
  - модель — JSON в БД, «общая для всех рабочих мест»; экспорт и импорт файлом `.mwtemplate`;
  - раскладка «1 на лист» или «2 на лист A4»; **по умолчанию для актов — 2 на лист**;
  - редактор: слева список блоков (добавить, перетащить, удалить), справа свойства выбранного блока, в центре живое превью на данных выбранного реального заказа или демо-заказа;
  - меню «Вставить поле» со всеми доступными подстановками на русском; отмена и повтор; кнопка «Сбросить к стандартному».
- Спека интерфейса, раздел 7: «Три колонки: блоки (≈ 260px), живое превью, свойства (≈ 320px). Сверху выбор шаблона и заказа для примера, отмена и повтор, «Сбросить к стандартному»».
- Правила интерфейса (`AGENTS.md`):
  - ширины раскладки — только `display` и `:global([data-width=…])`;
  - цвета и размеры — только из `tokens.css`;
  - Svelte 5 только на рунах, обработчики — пропсы `on…`;
  - импорты — `#lib/...` с явными расширениями, типы API — `import type` из `$api/...`;
  - компоненты — из `#lib/ui/...`, новые общие — туда же, с разделом на витрине `/design`;
  - тексты — в `#lib/i18n/ru.ts`; иконки — `@lucide/svelte`, по одной;
  - `fetch` — только в `src/lib/api/client.ts`;
  - автосохранение — только через `createAutosave`, редактор записи — `createEntityEditor`;
  - сквозные тесты — Chromium и WebKit, запросы по ролям и подписям, данные через API с `uniq()`.
- Окружение для каждой команды:
  ```
  export PATH="/opt/homebrew/opt/node@24/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
  export CARGO_TARGET_DIR=/Users/ruf/dev/MyWork2/target
  export MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres
  ```
  `rustc`/`cargo` не запускать вне worktree: rustup сам поставит лишний stable. Программу для компьютера не запускать.

## Решения плана

1. **Модель — `mw_domain::document_template`.** `mw_docs::model` остаётся путём-реэкспортом (`pub use mw_domain::document_template::*;`), внутренний код `mw-docs` не переименовывается.
2. **Права.**
   - Писать шаблоны и строить превью несохранённой модели можно с правом `SettingsManage`. Нового права нет.
   - Читать шаблоны, список полей и файл может любой вошедший.
3. **«Стандартный» хранится как `NULL`.**
   - Сохранение модели, равной `standard_template(kind)`, делает шаблон стандартным (`custom: false`). Поэтому улучшения стандартного шаблона доходят до тех, кто его не менял.
   - «Сбросить к стандартному» — обычная правка модели (её можно отменить), а не отдельный запрос удаления.
4. **Версия не обнуляется.** Строка таблицы живёт и после сброса: версия только растёт, поэтому устаревший редактор не перезапишет чужое сохранение.
5. **Нечитаемая модель в БД печатается стандартной.**
   - Так бывает после отката версии программы или ручной правки БД.
   - `Core` пишет `tracing::warn!`, `get` отдаёт стандартный шаблон с `custom: false` и версией строки.
   - Приём заказа не ломается из-за шаблона.
6. **Калибровку не правят.**
   - `DocumentKind::editable()` — все виды, кроме `Calibration`.
   - Блок `CalibrationMarks` допустим только в шаблоне калибровки.
7. **Раскладки.** Новая раскладка `SheetPerCopy` (`sheet_per_copy`) — «1 на лист»: экземпляр сервиса на первом листе, экземпляр клиента на втором. Подписи раскладок в интерфейсе:
   - `OnePerPage` — «Один экземпляр»;
   - `SheetPerCopy` — «1 на лист»;
   - `TwoPerA4` — «2 на лист A4».

   Наклейка и калибровка допускают только `OnePerPage`.
8. **`fill` штрихкода внутри `Columns` запрещён** проверкой (`fill_in_columns`). Колонки внутри колонок тоже запрещены (`nested_columns`).
9. **Импорт `.mwtemplate` не сохраняет.**
   - Сервер разбирает и проверяет файл и отдаёт модель для вида, в который импортируют (`kind` заменяется).
   - Редактор кладёт её в историю: это отменяемая правка, и сохраняет её автосохранение.
   - Экспорт отдаёт сохранённую модель, поэтому перед ним редактор дожидается своей записи (`flush`).
10. **Превью несохранённой модели** — `DocumentRequest.draft: Option<Template>`; тот же семафор рендера.
    - В интерфейсе превью идёт очередью: не больше одного рендера за раз от редактора, и строится всегда последняя модель.
11. **Маршрут редактора — `/templates/[kind]`**, вне раскладки настроек (ей не хватит ширины на три колонки). Вход — кнопка «Изменить» у документа в «Настройки → Документы и реквизиты». Без права `SettingsManage` кнопки нет, а маршрут показывает пояснение.
12. **В план 7.2 перенесены:**
    - «свой Typst-код» вместе с пределом времени компиляции;
    - массовая печать;
    - штрихкоды MyWork 2 не в UPC;
    - запасной шрифт;
    - блок логотипа или картинки;
    - перенос `Shablon_*.htm` и `[ACTS] BarcodeH/BarcodeW` из `legacy_rows`.

    QR-код остаётся в плане 9, таблица запчастей — в плане 8.

## Review Focus

1. **Длинный договор в «2 на лист A4»** не помещается в половину листа. Ожидается то же, что сейчас: каждый экземпляр печатается на своём листе, и в превью видно оба листа. Тест — задача 3, `two_per_a4_overflow_still_splits_with_user_template`.
2. **Быстрый набор текста в свойствах.** На сервер одновременно уходит не больше одного превью от редактора, и последнее превью построено по последней модели. Тест — задача 6, `preview_queue_runs_one_at_a_time_with_the_latest`.
3. **Модель в БД не читается** (испорчена или от более новой версии). Печать акта приёма идёт по стандартному шаблону, а не падает с 500. Тест — задача 4, `unreadable_stored_model_prints_standard`.
4. **Импорт `.mwtemplate` от другого вида**, который этому виду не подходит (акт «2 на лист A4» в наклейку). Ответ 422 с понятным текстом, модель в редакторе не меняется. Тесты — задача 5, `import_into_wrong_kind_is_422`; задача 8, `bad_file_leaves_editor_unchanged`.
5. **Ctrl/⌘Z в редакторе.** Вне полей отменяется правка шаблона, а не последнее действие из уведомления (`Toaster` тоже слушает `Mod+Z`). В поле ввода работает отмена самого браузера. Тест — задача 7, сквозной `undo_hotkey_undoes_template_edit`.

---

### Task 1: Разделить `typst_gen.rs` без изменения поведения

**Files:**
- Create: `crates/mw-docs/src/typst_gen/mod.rs`, `crates/mw-docs/src/typst_gen/page.rs`, `crates/mw-docs/src/typst_gen/blocks.rs`, `crates/mw-docs/src/typst_gen/text.rs`
- Delete: `crates/mw-docs/src/typst_gen.rs`
- Test: `crates/mw-docs/src/layout_tests.rs` (без изменений), временная проверка вывода

**Interfaces:**
- Produces: те же публичные `DocumentData`, `RenderOptions`, `render`, `to_typst`. Внутри `text.rs` — одна функция `fn text_expr(text: &str, mode: TextMode) -> String` с `enum TextMode { Wrap, ShortLine, OneLine }` вместо `expr`, `short_line_expr` и `one_line_expr`.

Что куда:
- `mod.rs` — типы данных и опций, `render`, `to_typst`, пролог Typst;
- `page.rs` — геометрия страницы, раскладки экземпляров, смещение, линия отреза;
- `blocks.rs` — вывод каждого `BlockBody`;
- `text.rs` — выражения текста с подстановками.

- [ ] **Step 1: Сохранить вывод до разделения.** Временный тест (не коммитить) пишет `to_typst` для каждого `standard_template(kind)` из `DocumentKind::ALL` в файлы вне репозитория, в каталог scratchpad. Опции — два набора: `RenderOptions::default()` и `RenderOptions { label_mm: Some((40.0, 30.0)), offset_mm: (1.0, -1.0), dpi: Some(203) }`.
- [ ] **Step 2: Разделить файл** по модулям выше. Помощники `expr`/`short_line_expr`/`one_line_expr` свести в `text_expr` так, чтобы итоговая строка Typst не изменилась.
- [ ] **Step 3: Сравнить вывод.** Тот же временный тест после разделения. Ожидается: все 14 файлов совпадают байт в байт (`cmp`). Затем удалить временный тест.
- [ ] **Step 4: Прогнать проверки.** Run: `cargo test -p mw-docs && cargo clippy -p mw-docs --all-targets -- -D warnings`. Expected: PASS, предупреждений нет.
- [ ] **Step 5: Commit** — `refactor(docs): typst_gen по модулям, один text_expr`.

### Task 2: Модель шаблона — тип API в `mw-domain`, проверка, файл `.mwtemplate`

**Files:**
- Create: `crates/mw-domain/src/document_template.rs`
- Modify: `crates/mw-domain/src/lib.rs` (модуль), `crates/mw-domain/src/documents.rs` (`editable`, `ordinal`, `label_ru`), `crates/mw-domain/src/validation.rs` (русские тексты новых кодов), `crates/mw-docs/src/model.rs` (становится реэкспортом), `crates/mw-docs/src/standard.rs` (тест)
- Test: тесты в `crates/mw-domain/src/document_template.rs`; `crates/mw-core/tests/api_types.rs` (новые коды в `field_error_messages_are_russian`)

**Interfaces:**
- Produces:
  - `mw_domain::document_template::{Template, PageSetup, PageSize, Margins, Layout, Block, CopyFilter, Align, BlockBody, FieldRow}` — те же поля и имена serde, что сейчас в `mw_docs::model`. К ним — derive `TS` и `ToSchema` и `#[serde(deny_unknown_fields)]` на всех структурах и на `BlockBody`.
  - `impl Template { pub fn validate(&self) -> Result<(), Vec<FieldError>> }`.
  - `pub const TEMPLATE_FILE_FORMAT: &str = "mwtemplate"; pub const TEMPLATE_FILE_VERSION: u32 = 1; pub const TEMPLATE_FILE_MAX_BYTES: usize = 1_048_576;`
  - `#[serde(deny_unknown_fields)] pub struct TemplateFile { pub format: String, pub format_version: u32, pub template: Template }` с `TemplateFile::new(template) -> Self`, `TemplateFile::from_value(value: serde_json::Value, kind: DocumentKind) -> Result<Template, Vec<FieldError>>`, `TemplateFile::parse(json: &str, kind) -> Result<Template, Vec<FieldError>>`.
  - `DocumentKind::editable(self) -> bool` (все, кроме `Calibration`).
  - `DocumentKind::ordinal(self) -> i64` — позиция в `DocumentKind::ALL`, начиная с 1.
  - `DocumentKind::label_ru(self) -> &'static str` — те же имена, что `ru.print.documents`: «Акт приёма», «Акт выдачи», «Акт приёма по гарантии», «Акт выдачи после гарантии», «Пустой акт приёма», «Наклейка», «Пробная наклейка».

Границы `validate`. Поле ошибки — путь через точку: `blocks.{i}`, во вложенных — `blocks.{i}.columns.{c}.{j}`, затем имя поля.

| Что | Условие | Поле | Код |
|---|---|---|---|
| размер `Custom` | ширина и высота в 15.0..=420.0 | `page.size.width_mm` / `page.size.height_mm` | `out_of_range` |
| поля страницы | каждое в 0.0..=50.0 | `page.margins_mm.top` и т. д. | `out_of_range` |
| место под содержимое | ширина и высота за вычетом полей ≥ 10.0 мм (с учётом `landscape`) | `page.margins_mm` | `too_large` |
| `TwoPerA4` | только A4 и `landscape == false` | `layout` | `requires_a4` |
| `Sticker`, `Calibration` | только `OnePerPage` | `layout` | `not_allowed` |
| `font_size_pt` | 5.0..=36.0 | `font_size_pt` | `out_of_range` |
| всего блоков с вложенными | ≤ 200 | `blocks` | `too_many` |
| `Text.text` | ≤ 10 000 символов | `blocks.{i}.text` | `too_long` |
| `Text.size_pt` | `Some` в 5.0..=72.0 | `blocks.{i}.size_pt` | `out_of_range` |
| `Fields.rows` | ≤ 50 | `blocks.{i}.rows` | `too_many` |
| `FieldRow.label` / `value` | ≤ 200 / ≤ 2 000 символов | `blocks.{i}.rows.{k}.label` / `.value` | `too_long` |
| `Barcode.height_mm` | 4.0..=60.0 | `blocks.{i}.height_mm` | `out_of_range` |
| `Signatures.left` / `right` | ≤ 200 символов | `blocks.{i}.left` / `.right` | `too_long` |
| `Spacer.mm` | 0.0..=200.0 | `blocks.{i}.mm` | `out_of_range` |
| `Columns.columns` | 2..=4 колонки | `blocks.{i}.columns` | `out_of_range` |
| `Columns.gap_mm` | 0.0..=30.0 | `blocks.{i}.gap_mm` | `out_of_range` |
| `Columns` в колонке | запрещено | путь вложенного блока | `nested_columns` |
| `Barcode{fill: true}` в колонке | запрещено | путь вложенного блока | `fill_in_columns` |
| `CalibrationMarks` | только при `kind == Calibration` | путь блока | `not_allowed` |

Длина считается в `chars()`. Диапазоны проверяются через `contains`, поэтому `NaN` тоже вне диапазона.

`TemplateFile::from_value` проверяет по порядку и останавливается на первой ошибке разбора:
1. значение — объект, иначе `file`/`invalid`;
2. `format == "mwtemplate"`, иначе `format`/`invalid`;
3. `format_version` в 1..=`TEMPLATE_FILE_VERSION`, иначе `format_version`/`unsupported`;
4. `template` разбирается в `Template`, иначе `file`/`invalid`;
5. `template.kind = kind`, затем `validate()`.

`parse` сначала проверяет `json.len() <= TEMPLATE_FILE_MAX_BYTES` (иначе `file`/`too_large`) и синтаксис JSON (`file`/`invalid`).

Русские тексты новых кодов в `FieldError::message_ru` (с заглавной буквы):
- `too_many` — «Слишком много элементов»;
- `too_long` — «Слишком длинный текст»;
- `too_large` — «Поля страницы не оставляют места для содержимого»;
- `requires_a4` — «Раскладка «2 на лист A4» — только для страницы A4 в книжной ориентации»;
- `not_allowed` — «Не подходит для этого документа»;
- `nested_columns` — «Колонки нельзя вкладывать в колонки»;
- `fill_in_columns` — «Растягивать штрихкод на высоту нельзя внутри колонок»;
- `unsupported` — «Файл сделан в более новой версии программы»;
- `mismatch` — «Шаблон другого документа»;
- `not_editable` — «Этот документ не настраивается».

- [ ] **Step 1: Написать падающие тесты** в `document_template.rs`:
  ```rust
  #[test] fn template_round_trips_through_json()      // serde_json туда и обратно == исходному
  #[test] fn unknown_field_is_rejected()              // {"type":"text",...,"colour":1} → Err
  #[test] fn two_per_a4_requires_portrait_a4()        // A5 + TwoPerA4 → [("layout","requires_a4")]; A4 + landscape → то же
  #[test] fn sticker_allows_only_one_per_page()       // Sticker + TwoPerA4 → ("layout","not_allowed")
  #[test] fn calibration_marks_only_in_calibration()  // IntakeAct с CalibrationMarks в blocks[0] → ("blocks.0","not_allowed")
  #[test] fn limits_report_their_paths()              // Text size_pt 100 в blocks[1] → ("blocks.1.size_pt","out_of_range");
                                                      // text 10 001 символ в blocks[0].columns[1][0] → ("blocks.0.columns.1.0.text","too_long")
  #[test] fn nested_columns_and_fill_in_columns_are_rejected()
  #[test] fn too_many_blocks()                        // 201 Spacer → ("blocks","too_many")
  #[test] fn margins_must_leave_room()                // Custom 20×20, все поля 6 → ("page.margins_mm","too_large")
  #[test] fn file_checks_format_version_and_kind()    // format "x" → format/invalid; version 0 и 2 → format_version/unsupported;
                                                      // "{" → file/invalid; файл IntakeAct, разобранный для IssueAct → kind == IssueAct;
                                                      // строка > 1 МБ → file/too_large
  ```
  В `crates/mw-docs/src/standard.rs`: `#[test] fn standard_templates_are_valid()` — для каждого вида `standard_template(kind).validate() == Ok(())`. В `api_types.rs` дописать новые коды в список `field_error_messages_are_russian`.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-domain document_template`. Expected: FAIL (модуля нет).
- [ ] **Step 3: Перенести модель и реализовать проверку и файл.**
  - Перенести типы в `mw_domain::document_template`.
  - `mw-docs/src/model.rs` свести к `pub use mw_domain::document_template::*;`, и `crate::fields` в doc-ссылке модели заменить текстом.
  - Добавить методы `DocumentKind` и тексты кодов.
  - `serde_json` в `crates/mw-domain/Cargo.toml` перенести из `[dev-dependencies]` в `[dependencies]` (он нужен `TemplateFile::from_value`; MIT/Apache-2.0, уже в workspace). Затем `cargo check` в `apps/desktop/src-tauri` обновляет его `Cargo.lock`; коммитить вместе.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-domain && cargo test -p mw-docs && cargo test -p mw-core --test api_types`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(domain): модель шаблона документа — тип API, проверка и файл .mwtemplate`.

### Task 3: Новые возможности модели в генераторе и стандартных шаблонах

**Files:**
- Modify: `crates/mw-domain/src/document_template.rs`, `crates/mw-docs/src/typst_gen/{page,blocks,text}.rs`, `crates/mw-docs/src/standard.rs`
- Test: `crates/mw-docs/src/layout_tests.rs`, тесты в `document_template.rs`

**Interfaces:**
- Consumes: модель задачи 2, `text_expr` задачи 1.
- Produces: в модели появляются
  - `FieldRow.bold: bool` (`#[serde(default)]`);
  - `BlockBody::Barcode.stretch: bool` (`#[serde(default)]`);
  - `Layout::SheetPerCopy` (`sheet_per_copy`).

  `validate` допускает `SheetPerCopy` для всех видов, кроме `Sticker` и `Calibration` (там `not_allowed`).

Поведение:
- **`FieldRow.bold`** — строка печатается жирной: и название, и значение.
- **`Barcode.stretch`** — модуль растёт, пока штрихкод помещается в ширину колонки: с DPI — целым числом точек принтера, без DPI — непрерывно. Номинальных 0,33 мм модуль никогда не меньше, кроме уже действующего сжатия на узкой наклейке. Без `stretch` всё как сейчас.
- **`SheetPerCopy`** — как запасной путь `TwoPerA4` при переполнении: сервисный экземпляр на одной странице, клиентский на следующей. Размер страницы — из шаблона.
- **`one_line`** — что не помещается, обрезается с «…» (U+2026) в конце, а не по краю. Подбор — двоичным поиском по `str.clusters()` внутри `layout(size => …)` с `measure`. Текст по-прежнему приходит только из `data.json`.
- **Стандартный акт выдачи** — строка «К оплате» с `bold: true`.

- [ ] **Step 1: Написать падающие тесты** в `layout_tests.rs`, по образцу тех, что уже меряют штрихкод и текст:
  ```rust
  #[test] fn bold_row_is_bold()                       // to_typst шаблона с одной строкой bold содержит жирное начертание у этой строки и только у неё
  #[test] fn stretched_barcode_fills_the_column()     // страница Custom 100×40, поля 2, dpi 203, stretch → ширина штрихкода ≥ ширины колонки − 1 модуль,
                                                      // модуль — целое число точек 203 dpi; без stretch — 3 точки, как сейчас
  #[test] fn sheet_per_copy_puts_each_copy_on_its_page() // A5 + SheetPerCopy, CopyLabel → 2 страницы 148×210; на первой «Экземпляр сервиса», на второй «Экземпляр клиента»
  #[test] fn one_line_ends_with_ellipsis()            // наклейка 40×30, устройство из 200 символов → в тексте есть «…», хвоста строки нет
  #[test] fn issue_act_to_pay_is_bold()               // стандартный IssueAct: строка «К оплате» с bold == true
  #[test] fn two_per_a4_overflow_still_splits_with_user_template() // TwoPerA4, Text из 6 000 символов → 2 страницы, экземпляры не на одной странице
  ```
  В `document_template.rs`: `sheet_per_copy_is_not_for_stickers` (Sticker + SheetPerCopy → `layout`/`not_allowed`; IntakeAct + SheetPerCopy → Ok) и `old_json_without_new_fields_still_reads` (строка без `bold` и `stretch` разбирается, значения `false`).
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-docs && cargo test -p mw-domain document_template`. Expected: FAIL.
- [ ] **Step 3: Реализовать** поля модели, вывод в `blocks.rs`/`page.rs`/`text.rs` и правку стандартного акта выдачи.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-docs && cargo test -p mw-domain && cargo test -p mw-core --test documents`. Expected: PASS. Документы ядра не изменились, кроме жирной строки «К оплате».
- [ ] **Step 5: Commit** — `feat(docs): жирные строки, растягиваемый штрихкод, «1 на лист», многоточие`.

### Task 4: Хранение шаблонов в БД и ядро

**Files:**
- Create: `crates/mw-db/src/migration/m20261123_000001_document_templates.rs`, `crates/mw-db/src/entity/document_template.rs`, `crates/mw-core/src/document_templates.rs`, `crates/mw-core/tests/document_templates.rs`
- Modify: `crates/mw-db/src/migration/mod.rs`, `crates/mw-db/src/entity/mod.rs`, `crates/mw-core/src/lib.rs`, `crates/mw-core/src/documents.rs`, `crates/mw-api/src/routes/documents.rs` (`DocumentRequest` больше не `Copy`)

**Interfaces:**
- Consumes: `Template::validate`, `DocumentKind::{editable, ordinal, label_ru}` (задача 2), `mw_docs::standard_template`, `mw_docs::fields::fields()`.
- Produces (mw-core, все с `Serialize`, `TS`, `ToSchema`):
  ```rust
  pub struct DocumentTemplate { pub id: i64 /* kind.ordinal() */, pub kind: DocumentKind, pub template: Template,
      pub standard: Template, pub custom: bool, pub version: i64, pub updated_at: Option<NaiveDateTime> }
  pub struct DocumentTemplateInfo { pub kind: DocumentKind, pub custom: bool, pub version: i64, pub updated_at: Option<NaiveDateTime> }
  // входной тип: Deserialize + TS + ToSchema вместо Serialize
  #[serde(deny_unknown_fields)] pub struct TemplateSave { pub template: Template, pub version: i64 }
  pub struct DocumentField { pub placeholder: String, pub label: String }

  impl Core {
      pub async fn list_document_templates(&self, actor: &Actor) -> Result<Vec<DocumentTemplateInfo>, CoreError>;
      pub async fn get_document_template(&self, actor: &Actor, kind: DocumentKind) -> Result<DocumentTemplate, CoreError>;
      pub async fn save_document_template(&self, actor: &Actor, kind: DocumentKind, save: TemplateSave) -> Result<DocumentTemplate, CoreError>;
      pub async fn import_document_template(&self, actor: &Actor, kind: DocumentKind, file: serde_json::Value) -> Result<Template, CoreError>;
      pub fn document_fields(&self) -> Vec<DocumentField>;
  }
  // documents.rs:
  pub struct DocumentRequest { /* как было */ pub draft: Option<Template> } // Clone, не Copy
  ```

Таблица `document_templates`:
- `kind` TEXT PRIMARY KEY;
- `model` TEXT NULL — JSON модели; `NULL` — стандартный шаблон;
- `version` BIGINT NOT NULL;
- `updated_at` TIMESTAMP NOT NULL.

Правила:
- **`list`** — только `editable()` виды в порядке `DocumentKind::ALL`, без строки в БД — `custom: false`, `version: 0`, `updated_at: None`.
- **`get`, `save`, `import` для `Calibration`** — `Validation[("kind","not_editable")]`.
- **`save`:**
  1. проверить право `SettingsManage`;
  2. `template.kind != kind` — `("kind","mismatch")`;
  3. `validate()`;
  4. если модель равна `standard_template(kind)`, хранить `NULL`;
  5. в `begin_write()` прочитать строку (нет строки — версия 0);
  6. новое значение `model` равно сохранённому — вернуть текущее без записи и без журнала, даже при устаревшей `version`;
  7. иначе при `save.version != текущей` — `Conflict { entity: "document_template", id: kind.ordinal(), current_version }`;
  8. иначе записать с `version + 1` и `updated_at = now()`.
- **Журнал:**
  - действие `document_template.updated`;
  - `summary` — «Изменён шаблон «{label_ru}»», а когда новое значение `NULL` — «Шаблон «{label_ru}» сброшен к стандартному»;
  - `details` — `{"kind": "<kind>"}` без модели.
- **Модель в БД не разбирается или не проходит `validate`** — шаблон считается стандартным (`tracing::warn!` с видом и ошибкой), как в решении 5.
- **Рендер** (`Core::rendered`):
  - модель — `draft`, если он задан, иначе сохранённая, иначе стандартная; у `Calibration` — всегда стандартная;
  - `draft` требует `SettingsManage`, `editable()`, `draft.kind == req.kind` и `validate()`; ошибки — `Validation` до рендера.
- **`import_document_template`** — право `SettingsManage`, затем `TemplateFile::from_value(file, kind)`, ошибки — `Validation`; ничего не пишет.

- [ ] **Step 1: Написать падающие тесты** в `crates/mw-core/tests/document_templates.rs`, каждый по `all_backends()`:
  ```rust
  standard_until_saved                 // get(IntakeAct): custom false, version 0, template == standard
  save_then_get_and_render_use_it      // первый Text стандартного акта → «Проверка шаблона», save v0 → version 1, custom true;
                                       // render_document(IntakeAct, sample) — в тексте есть «Проверка шаблона»
  stale_version_conflicts              // два разных save с version 0 → второй Conflict { current_version: 1 }
  unchanged_save_is_noop               // тот же template с version 0 после версии 1 → Ok, version 1, записей журнала не прибавилось
  saving_the_standard_resets           // save(standard) → custom false, version 2, в журнале «Шаблон «Акт приёма» сброшен к стандартному»
  needs_settings_manage                // актёр без права: save и import → Forbidden(SettingsManage); get, list → Ok
  kind_mismatch_and_invalid            // template.kind = IssueAct в save(IntakeAct) → kind/mismatch; A5 + TwoPerA4 → layout/requires_a4
  calibration_is_not_editable          // get/save(Calibration) → kind/not_editable; list без Calibration (6 видов)
  draft_renders_without_saving         // render_document с draft (Text «Черновик») → в тексте «Черновик»; get → custom false;
                                       // draft у актёра без права → Forbidden
  unreadable_stored_model_prints_standard // в строку записать model = "{" (через Core::db() в тесте) → get: custom false;
                                          // render_document(IntakeAct, sample) — Ok
  import_replaces_kind                 // файл IntakeAct → import(IssueAct) → template.kind == IssueAct, в БД ничего
  fields_are_listed                    // document_fields(): 33 поля, первое { "заказ.номер", "Номер заказа" }
  ```
  В `crates/mw-db` — тест миграции по образцу соседних: таблица создаётся на обоих движках.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-core --test document_templates`. Expected: FAIL.
- [ ] **Step 3: Реализовать** миграцию, сущность, `document_templates.rs`, поле `draft` и выбор модели в `rendered`. Вызовы `DocumentRequest` в API и тестах дополнить `draft: None`.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-db && cargo test -p mw-core && cargo clippy --workspace --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): шаблоны документов в БД — версии, журнал, черновик для превью`.

### Task 5: API шаблонов и контракт

**Files:**
- Create: `crates/mw-api/src/routes/document_templates.rs`, `crates/mw-api/tests/document_templates.rs`
- Modify: `crates/mw-api/src/routes/mod.rs`, `crates/mw-api/src/routes/documents.rs` (общие имя файла и ответ PNG), `crates/mw-api/src/dto.rs` (`TemplatePreview`), `crates/mw-api/openapi.json`, `crates/mw-api/bindings/*` (генерируются)

**Interfaces:**
- Consumes: методы и типы задачи 4.
- Produces (тег OpenAPI «Документы»):

| Метод и путь | Тело | Ответ | Право |
|---|---|---|---|
| `GET /api/v1/document-templates` | — | `Vec<DocumentTemplateInfo>` | вход |
| `GET /api/v1/document-templates/{kind}` | — | `DocumentTemplate` | вход |
| `PUT /api/v1/document-templates/{kind}` | `TemplateSave` | `DocumentTemplate` | `settings_manage` |
| `POST /api/v1/document-templates/{kind}/preview` | `TemplatePreview` | `DocumentPages` | `settings_manage` |
| `GET /api/v1/document-templates/{kind}/file` | — | `TemplateFile` как вложение | вход |
| `POST /api/v1/document-templates/{kind}/file` | JSON файла (`TemplateFile` в схеме) | `Template` (не сохраняется) | `settings_manage` |
| `GET /api/v1/document-fields` | — | `Vec<DocumentField>` | вход |

- **`TemplatePreview`** (в `dto.rs`, рядом с `DocumentQuery`, `deny_unknown_fields`) — `template: Template` и необязательные `order_id: Option<i64>`, `dpi`, `printer_dpi`, `label_w_mm`, `label_h_mm`, `offset_x_mm`, `offset_y_mm`.
  - Опции и умолчания — как у `DocumentQuery`: `dpi` 110, границы проверяет ядро.
  - Без `order_id` — образец для видов из заказа, у пустого акта заказа нет.
  - Ответ строится тем же кодом PNG, что у документов.
- **Имя файла** — латиницей, как у PDF: `file_name` в `documents.rs` выносится в функцию, которая отдаёт основу (`akt-priema`).
  - Файл шаблона называется `{основа}.mwtemplate` и отдаётся с `Content-Type: application/json` и `Content-Disposition: attachment`.
- **Тело `POST …/file`** читается как `serde_json::Value`, поэтому ошибки формата отвечают 422 по полям, а не 400.

- [ ] **Step 1: Написать падающие тесты** в `crates/mw-api/tests/document_templates.rs` по образцу `tests/documents.rs`:
  ```rust
  templates_round_trip_over_http   // GET list → 6 видов; GET intake_act → version 0; PUT изменённый → 200 version 1;
                                   // PUT с version 0 → 409; PUT с лишним полем в блоке → 400; PUT A5+two_per_a4 → 422 с полем layout
  preview_returns_pages            // POST preview (admin) → 200, pages.len() ≥ 1, width_mm 210; без права → 403
  file_export_and_import           // GET file → Content-Disposition с «akt-priema.mwtemplate», JSON format "mwtemplate", format_version 1;
                                   // POST его в issue_act/file → 200, kind "issue_act"; POST {"format":"x"} → 422 поле format
  import_into_wrong_kind_is_422    // файл акта (two_per_a4) → POST sticker/file → 422, поле layout, код not_allowed
  calibration_is_422               // GET document-templates/calibration → 422 kind/not_editable
  fields_listed                    // GET document-fields → 33 элемента
  printing_uses_saved_template     // PUT шаблон с Text «Проверка API» → GET /api/v1/documents/intake_act?format=png → 200
                                   // (текст проверяет ядро; здесь — что путь печати не сломан сохранённой моделью)
  ```
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-api --test document_templates`. Expected: FAIL.
- [ ] **Step 3: Реализовать** маршруты и `TemplatePreview`, затем обновить контракт: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-api` (контракт — без переменной). Expected: PASS; в `bindings/` есть `Template.ts`, `DocumentTemplate.ts`, `TemplateSave.ts`, `TemplateFile.ts`, `DocumentField.ts`, `TemplatePreview.ts`.
- [ ] **Step 5: Commit** — `feat(api): шаблоны документов, превью черновика и файл .mwtemplate`.

### Task 6: Интерфейс — данные и логика редактора без экрана

**Files:**
- Create:
  - `apps/ui/src/lib/api/document-templates.ts` + `.test.ts`;
  - `apps/ui/src/lib/templates/ops.ts` + `.test.ts`;
  - `apps/ui/src/lib/templates/history.svelte.ts` + `history.test.ts`;
  - `apps/ui/src/lib/templates/defaults.ts` + `.test.ts`;
  - `apps/ui/src/lib/templates/preview-queue.ts` + `.test.ts`
- Modify: `apps/ui/src/lib/api/documents.ts` (вынести разбор имени файла из `Content-Disposition` для общего использования, если он внутренний)

**Interfaces:**
- Consumes: типы `$api/Template`, `$api/Block`, `$api/BlockBody`, `$api/DocumentTemplate`, `$api/DocumentTemplateInfo`, `$api/DocumentField`, `$api/DocumentPages`, `$api/TemplatePreview`, `$api/DocumentKind`.
- Produces:
  ```ts
  // #lib/api/document-templates.js
  listTemplates(): Promise<DocumentTemplateInfo[]>
  getTemplate(kind: DocumentKind): Promise<DocumentTemplate>
  saveTemplate(kind: DocumentKind, template: Template, version: number): Promise<DocumentTemplate>
  previewTemplate(kind: DocumentKind, body: TemplatePreview, signal?: AbortSignal): Promise<DocumentPages>
  exportTemplateFile(kind: DocumentKind): Promise<{ blob: Blob; filename: string }>
  importTemplateFile(kind: DocumentKind, file: unknown): Promise<Template>
  documentFields(): Promise<DocumentField[]>

  // #lib/templates/ops.js — неизменяемые операции; путь [i] — блок верхнего уровня, [i, c, j] — блок j колонки c блока i
  type BlockPath = readonly number[];
  type Container = readonly [] | readonly [number, number]; // верхний уровень или колонка
  blockAt(t: Template, path: BlockPath): Block | undefined
  replaceBlock(t: Template, path: BlockPath, block: Block): Template
  insertBlock(t: Template, at: BlockPath, block: Block): Template      // at — место нового блока
  removeBlock(t: Template, path: BlockPath): Template
  duplicateBlock(t: Template, path: BlockPath): Template               // копия сразу после
  canMove(t: Template, from: BlockPath, to: BlockPath): boolean        // false: Columns в колонку, fill-штрихкод в колонку
  moveBlock(t: Template, from: BlockPath, to: BlockPath): Template     // to — место в итоговом шаблоне; при !canMove — t как есть
  reorder(t: Template, container: Container, order: number[]): Template // order — прежние индексы в новом порядке (как отдаёт SortableList)
  flatten(t: Template): { path: BlockPath; block: Block; depth: 0 | 1; column?: number }[]

  // #lib/templates/history.svelte.js
  createHistory<T>(initial: T, opts?: { limit?: number; mergeMs?: number }): {
    readonly current: T; readonly canUndo: boolean; readonly canRedo: boolean;
    push(next: T, mergeKey?: string): void; undo(): T | undefined; redo(): T | undefined; reset(to: T): void }
  // limit 100, mergeMs 1000: push с тем же mergeKey, что у предыдущего, не позже mergeMs — заменяет верхнюю запись

  // #lib/templates/defaults.js
  BLOCK_TYPES: readonly BlockBody['type'][] // порядок меню «Добавить блок»: text, fields, barcode, signatures, separator, spacer, columns, copy_label
  newBlock(type: BlockBody['type']): Block  // copies: 'both'

  // #lib/templates/preview-queue.js
  createPreviewQueue<I, O>(run: (input: I, signal: AbortSignal) => Promise<O>,
    opts: { delayMs: number; onresult: (o: O) => void; onerror: (e: unknown) => void }): { request(input: I): void; dispose(): void }
  ```

Новые блоки (`newBlock`), значения точные:
- `text` — `{ text: 'Текст', size_pt: null, bold: false, align: 'left', one_line: false }`;
- `fields` — `{ rows: [{ label: 'Название', value: '', hide_if_empty: false, bold: false }] }`;
- `barcode` — `{ height_mm: 12, digits: true, align: 'center', fill: false, stretch: false }`;
- `signatures` — `{ left: 'Подпись сервиса', right: 'Подпись клиента' }`;
- `separator` — `{}`;
- `spacer` — `{ mm: 4 }`;
- `columns` — `{ columns: [[], []], gap_mm: 6 }`;
- `copy_label` — `{ align: 'right' }`.

Строки этих текстов — в `ru.ts` (`ru.templates.defaults`).

Очередь превью:
- ждёт `delayMs` после последнего `request`;
- одновременно идёт не больше одного `run`;
- если во время `run` пришли новые запросы, после него идёт ещё один `run` — только с последним входом;
- результат устаревшего `run` (вход уже не последний) в `onresult` не передаётся;
- `dispose` прерывает текущий `run` через `signal` и глушит его результат и ошибку.

В редакторе `delayMs` — 400.

- [ ] **Step 1: Написать падающие тесты** (Vitest, фейковые таймеры, где нужно):
  - `ops.test.ts`:
    - каждая функция на верхнем уровне и в колонке;
    - исходный шаблон не меняется (`structuredClone` до и `toEqual` после);
    - `canMove` запрещает колонки в колонку и `fill`-штрихкод в колонку;
    - `reorder([2,0,1])` на верхнем уровне и в колонке;
    - `flatten` даёт пути и глубину.
  - `history.test.ts`:
    - push/undo/redo;
    - push после undo стирает redo;
    - 101-я запись вытесняет первую;
    - два push с ключом `blocks.0.text` за 500 мс — одна запись, за 1 500 мс — две;
    - разные ключи не сливаются.
  - `defaults.test.ts` — `newBlock` для каждого из `BLOCK_TYPES` равен значениям выше; в `BLOCK_TYPES` нет `calibration_marks`.
  - `preview-queue.test.ts`:
    - `preview_queue_runs_one_at_a_time_with_the_latest` — три `request` за 100 мс дают один `run` с третьим входом; `request` во время `run` даёт второй `run` после первого, только с последним входом, и одновременно идёт не больше одного;
    - `dispose` прерывает `signal`, `onresult` не зовётся.
  - `document-templates.test.ts` — пути, методы и тела запросов по образцу `documents.test.ts`; `exportTemplateFile` берёт имя из `Content-Disposition`.
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/templates src/lib/api/document-templates.test.ts`. Expected: FAIL.
- [ ] **Step 3: Реализовать** модули.
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(ui): данные шаблонов, операции над блоками, история и очередь превью`.

### Task 7: Экран редактора — каркас, блоки, превью, автосохранение, отмена, сброс

**Files:**
- Create:
  - `apps/ui/src/routes/(app)/templates/[kind]/+page.svelte`, `+page.ts`;
  - `apps/ui/src/lib/templates/editor.svelte.ts` + `editor.test.ts`;
  - `apps/ui/src/lib/templates/TemplateEditor.svelte`, `BlockList.svelte`, `TemplatePreview.svelte`, `Properties.svelte`;
  - `apps/ui/src/lib/templates/props/PageProps.svelte`, `props/TextProps.svelte`, `props/CopiesField.svelte`;
  - `apps/ui/e2e/template-editor.spec.ts`
- Modify: `apps/ui/src/routes/(app)/settings/documents/+page.svelte` (кнопка «Изменить»), `apps/ui/src/lib/i18n/ru.ts` (`ru.templates`)

**Interfaces:**
- Consumes: модули задачи 6; `createEntityEditor` (`#lib/app/entity-editor.svelte.js`); `registerHotkey`; `mw.print` (`#lib/print/settings.svelte.js`) и `ACT_DPI` (`#lib/print/print.svelte.js`); `listOrders` для выбора заказа; компоненты `#lib/ui`: `Select`, `Combobox`, `SortableList`, `SegmentedControl`, `DropdownMenu`, `Button`, `Callout`, `Switch`, `Field`.
- Produces:
  ```ts
  // #lib/templates/editor.svelte.js
  createTemplateEditor(initial: DocumentTemplate): {
    readonly template: Template; readonly standard: Template; readonly isStandard: boolean;
    readonly selected: BlockPath | 'page'; select(s: BlockPath | 'page'): void;
    edit(next: Template, mergeKey?: string): void;   // в историю и в автосохранение
    undo(): void; redo(): void; readonly canUndo: boolean; readonly canRedo: boolean;
    resetToStandard(): void;                         // = edit(standard), отменяется
    readonly status: SaveStatus;                     // для индикатора и конфликта
    flush(): Promise<boolean>; leave(ask: () => Promise<boolean>): Promise<boolean>; dispose(): void }
  ```
  `TextProps` и `PageProps` получают `{ editor, path }`. По этому же образцу задача 8 делает остальные панели свойств.

Устройство:
- **Запись.** `createEntityEditor<DocumentTemplate, 'template'>`: `id` — `DocumentTemplate.id`, `load` — `getTemplate`, `save` — `saveTemplate`. Конфликт показывается тем же способом, что в других редакторах записей: «Оставить моё» / «Взять новое». Если после `undo`/`edit` модель совпала с сохранённой, автосохранение ничего не шлёт.
- **Верхняя строка:**
  - `Select` «Документ» по `listTemplates` (без калибровки), переход на `/templates/{kind}`;
  - `Combobox` «Заказ для примера»: первый пункт «Пример» (образец), дальше поиск через `listOrders`; у пустого акта выбора нет;
  - кнопки «Отменить» и «Повторить»;
  - «Сбросить к стандартному» — недоступна, когда `isStandard`;
  - индикатор сохранения.
- **Клавиши.** `Mod+Z` — отменить, `Mod+Shift+Z` и `Mod+Y` — повторить, через `registerHotkey` без `allowInFields`. Регистрация новее, чем у `Toaster`, поэтому, пока открыт редактор, отмена — его.
- **Список блоков (`BlockList`):**
  - первый пункт «Страница», дальше `flatten`: блоки колонок с отступом под своей колонкой («Колонка 1», «Колонка 2»…);
  - перетаскивание — `SortableList` внутри своего контейнера (`reorder`);
  - меню блока: «Дублировать», «Удалить», «В колонку N» (для блоков верхнего уровня, если есть блок колонок, по `canMove`), «Вынести из колонок»;
  - «Добавить блок» — `DropdownMenu` по `BLOCK_TYPES`; новый блок встаёт после выбранного (или в конец) и выбирается;
  - подпись пункта — тип блока и начало текста, например «Текст · Акт приёма №…».
- **Превью (`TemplatePreview`):**
  - `createPreviewQueue` с `delayMs: 400` зовёт `previewTemplate` на каждой правке и смене заказа;
  - у `sticker` — `label_w_mm`/`label_h_mm`, смещение и `printer_dpi` из `mw.print` для принтера наклеек, у актов — `printer_dpi: ACT_DPI`; `dpi` растра не передаётся (110);
  - страницы — картинки в столбик по ширине колонки с подписями «Страница N из M»;
  - ошибка 422 — `Callout` «Шаблон не построен» со списком текстов ошибок по полям; прежние страницы остаются, но приглушены.
- **Свойства (`Properties`)** — по выбранному пункту: `PageProps` и `TextProps` здесь, у остальных типов пока заглушка «Свойства этого блока — в следующей задаче». Задача 8 её убирает.
  - `PageProps`: размер (A4, A5, «Свой» с шириной и высотой), «Альбомная», поля, раскладка с подписями из решения 7 (недопустимые для вида не показываются), базовый размер шрифта.
  - `TextProps`: текст (`Textarea`, `mergeKey` — путь и поле), размер (пусто — базовый), «Жирный», выравнивание (`SegmentedControl`), «В одну строку».
  - `CopiesField` («Во всех экземплярах» / «Только в экземпляре сервиса» / «Только в экземпляре клиента») — у каждого блока, когда раскладка не `one_per_page`.
- **Ширины:**
  - `lg`/`xl` — три колонки 260px / 1fr / 320px;
  - `md` — две: слева 300px блоки над свойствами, справа превью;
  - `xs`/`sm` — `SegmentedControl` «Блоки · Свойства · Превью».
  - Всё через `:global([data-width=…])`.
- **Вход и права:**
  - в «Документы и реквизиты» у каждого вида, кроме калибровки, рядом с «Образец» — кнопка-ссылка «Изменить» (`aria-label` «Изменить шаблон: {имя}»), только с правом `settings_manage`;
  - маршрут без права — `Callout` «Изменять шаблоны может пользователь с правом «{label_ru права}»» без редактора;
  - уход со страницы с несохранённым — `editor.leave(...)`, как в других редакторах.

- [ ] **Step 1: Написать падающие тесты.**
  - `editor.test.ts`, с подменённым API:
    - `edit` → `canUndo`; `undo` возвращает модель и шлёт её на запись;
    - `resetToStandard` → `isStandard`, и его можно отменить;
    - правка и отмена до записи ничего не шлют.
  - Сквозной `template-editor.spec.ts`:
    - вид — `warranty_intake_act` в Chromium и `warranty_issue_act` в WebKit (шаблоны общие для сервера, а проекты могут идти параллельно);
    - `afterEach` возвращает стандартный шаблон через API (`PUT` со `standard` и текущей версией).
  - Тесты в сквозном файле:
    - `opens_from_documents_settings` — «Настройки → Документы и реквизиты» → «Изменить шаблон: …» → видны «Страница», блоки и картинка превью;
    - `adds_text_block_and_saves` — «Добавить блок» → «Текст», вписать `uniq()` → после перезагрузки блок с этим текстом на месте;
    - `undo_hotkey_undoes_template_edit` — после правки фокус вне полей, Ctrl/⌘Z → текста нет, после перезагрузки тоже;
    - `reset_to_standard` — после правки «Сбросить к стандартному» → кнопка недоступна, по API `custom === false`;
    - `no_editor_without_permission` — пользователь без `settings_manage`: в «Документы и реквизиты» нет «Изменить», по прямому адресу — пояснение.
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/templates/editor.test.ts && npm run build && npx playwright test e2e/template-editor.spec.ts`. Expected: FAIL.
- [ ] **Step 3: Реализовать** экран, компоненты и строки `ru.templates`.
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify && npm run test:e2e`. Expected: PASS в Chromium и WebKit.
- [ ] **Step 5: Commit** — `feat(ui): редактор шаблонов — блоки, превью, автосохранение, отмена и сброс`.

### Task 8: Свойства всех блоков, «Вставить поле», файл `.mwtemplate`

**Files:**
- Create:
  - `apps/ui/src/lib/templates/props/FieldsProps.svelte`, `BarcodeProps.svelte`, `SignaturesProps.svelte`, `SpacerProps.svelte`, `ColumnsProps.svelte`, `CopyLabelProps.svelte`;
  - `apps/ui/src/lib/templates/InsertField.svelte`, `apps/ui/src/lib/templates/insert.ts` + `insert.test.ts`;
  - `apps/ui/src/lib/templates/TemplateFileMenu.svelte`
- Modify: `apps/ui/src/lib/templates/Properties.svelte` (убрать заглушку), `props/TextProps.svelte` (поле «Вставить поле»), `TemplateEditor.svelte` (меню файла), `apps/ui/src/lib/i18n/ru.ts`, `apps/ui/e2e/template-editor.spec.ts`

**Interfaces:**
- Consumes: `createTemplateEditor` и образец `TextProps` (задача 7); `documentFields`, `exportTemplateFile`, `importTemplateFile` (задача 6); помощник сохранения файла, которым пользуется «Сохранить PDF» в `#lib/print`.
- Produces: `insertAtCaret(value: string, selection: { start: number; end: number }, insert: string): { value: string; caret: number }` в `insert.ts`.

Панели свойств:
- **`FieldsProps`** — строки `SortableList`. У строки: «Название», «Значение» (с «Вставить поле»), «Скрывать, если пусто», «Жирная»; кнопки «Добавить строку» и «Удалить строку».
- **`BarcodeProps`** — «Высота, мм», «Цифры под штрихкодом», выравнивание, «Растянуть по ширине» (`stretch`), «Растянуть по высоте» (`fill`; в колонке недоступно).
- **`SignaturesProps`** — «Слева» и «Справа» (с «Вставить поле»).
- **`SpacerProps`** — «Отступ, мм».
- **`ColumnsProps`** — «Колонок» (2–4; лишние колонки при уменьшении удаляются вместе с блоками, это можно отменить), «Промежуток, мм».
- **`CopyLabelProps`** — выравнивание.
- **`Separator`** — только `CopiesField`.

Числовые поля шлют правку при допустимом значении, а границы — те же, что в таблице задачи 2.

«Вставить поле» (`InsertField`):
- кнопка рядом с полем ввода открывает `Combobox` с поиском по `label` из `documentFields()` через `normalizeSearch` (`#lib/format/search.js`);
- вставляет `{placeholder}` в позицию курсора (`insertAtCaret`) и возвращает фокус в поле, курсор — после вставки.

Файл (`TemplateFileMenu`, меню «Файл» в верхней строке):
- **«Сохранить в файл…»** — `flush()`, затем `exportTemplateFile`, затем сохранение файла.
- **«Загрузить из файла…»** — `<input type="file" accept=".mwtemplate,application/json">`, затем `file.text()`. Ошибки:
  - `JSON.parse` не удался — уведомление «Это не файл шаблона MyWork»;
  - 422 с полем `format` — то же уведомление;
  - `format_version` — «Файл сделан в более новой версии программы»;
  - прочие поля — `Callout` «Шаблон из файла не подходит этому документу» со списком текстов.

  Успех — `editor.edit(template)`: это отменяемая правка, и уведомление «Шаблон загружен из файла».

- [ ] **Step 1: Написать падающие тесты.**
  - `insert.test.ts`: вставка в середину, в конец, с заменой выделения; `caret` после вставки.
  - Сквозные, дописать в `template-editor.spec.ts`:
    - `edits_every_block_type` — добавить по блоку каждого типа, поменять по одному свойству (высота штрихкода 20, отступ 8, вторая подпись `uniq()`, вторая строка полей) → после перезагрузки значения на месте;
    - `insert_field_into_text` — в тексте «Вставить поле» → «Номер заказа» → в поле `{заказ.номер}` в позиции курсора;
    - `export_and_import_file` — «Сохранить в файл…» скачивает `akt-priema-garantiya.mwtemplate` (Chromium) или `akt-vydachi-garantiya.mwtemplate` (WebKit); загрузка этого файла после правки возвращает модель, а Ctrl/⌘Z откатывает загрузку;
    - `bad_file_leaves_editor_unchanged` — загрузка файла с `{"format":"x"}` → «Это не файл шаблона MyWork», список блоков прежний.
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/templates/insert.test.ts && npm run build && npx playwright test e2e/template-editor.spec.ts`. Expected: FAIL.
- [ ] **Step 3: Реализовать** панели, `InsertField`, `insertAtCaret` и меню файла.
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify && npm run test:e2e`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(ui): свойства блоков, «Вставить поле», шаблон в файл и из файла`.

### Task 9: Остатки печати из плана 6

**Files:**
- Modify: `crates/mw-core/src/documents.rs` (`plain_text`), `apps/ui/src/lib/print/print.svelte.ts`, места печати при приёме (поиск по `afterIntake`/`settleLate`), `apps/ui/src/lib/i18n/ru.ts`, `apps/ui/src/routes/(app)/settings/print/+page.svelte`
- Test: тесты `plain_text` в `crates/mw-core/src/documents.rs`; `apps/ui/src/lib/print/*.test.ts`; сквозной тест печати при приёме, где он уже есть

**Interfaces:**
- Consumes: ничего нового.
- Produces: ничего нового для других задач.

Поведение (по заметкам README «План 7»):
- **`plain_text`:**
  - строка из одних пробелов и табуляций считается пустой: её пробелы выбрасываются, и правило «больше двух переводов строк подряд — два» действует через неё;
  - инструкция `<?…?>` убирается, как комментарий (без `?>` дальше по тексту — это текст);
  - имя тега может содержать `:` (`<o:p>`, `</o:p>` — строчные, убираются);
  - в `HTML_ELEMENTS` добавляются недостающие из `tfoot`, `thead`, `tbody`, `code`, `font`, `span`, `u`, `sup`, `sub`, `small`; строчные они или блочные — по тому же делению, что у соседних (`tfoot`/`thead`/`tbody` — как `table`).
- **Нет принтеров.** Когда система не видит ни одного принтера (`list_printers` пуст), текст — «Принтеры не найдены», а не «Принтер не выбран, а принтера по умолчанию в системе нет».
- **Одно уведомление.** Приём с обоими переключателями в ветке «набран другой заказ» даёт одно уведомление, а не два одинаковых.
- **Поздний ответ (`settleLate`)** после того, как форма приёма закрыта и открыта новая, печатает, но не открывает предпросмотр поверх формы: только уведомление.
- **Подсказка про 300 dpi** в «Настройки → Печать» зависит от системы:
  - Windows — «Акты печатаются в 300 точек на дюйм.»;
  - macOS и Linux — «Акты уходят на принтер в PDF; штрихкод выровнен по сетке 300 точек на дюйм.».

- [ ] **Step 1: Написать падающие тесты.**
  - Rust:
    - `plain_text_collapses_blank_lines` — `"а\n   \n \t \n\nб"` → `"а\n\nб"`;
    - `plain_text_drops_office_tags` — `"<p>Текст<o:p></o:p></p><?xml version=\"1.0\"?><table><tfoot><tr><td>Итог</td></tr></tfoot></table>"` → `"Текст\nИтог"`;
    - `plain_text_keeps_question_text` — `"a <? b"` → `"a <? b"` (нет `?>` — не разметка).
  - Vitest: текст ошибки при пустом списке принтеров; одно уведомление в ветке «другой заказ»; поздний ответ не открывает предпросмотр.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-core plain_text && cd apps/ui && npx vitest run src/lib/print`. Expected: FAIL.
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-core && cd apps/ui && npm run verify && npm run test:e2e`. Expected: PASS.
- [ ] **Step 5: Commit** — `fix: остатки печати плана 6 — plain_text, нет принтеров, одно уведомление, поздний ответ`.

### Task 10: Документация

**Files:**
- Modify:
  - `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` (§5.1 «Как сделано в плане 7.1»);
  - `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (пункт «Редактор шаблонов» — как сделано);
  - `docs/superpowers/plans/README.md` (план 7.1 выполнен; заметки плана 7.2 и остатки);
  - `AGENTS.md`

**Interfaces:** нет.

- [ ] **Step 1: Спека 5.1** — подраздел «Как сделано в плане 7.1»:
  - модель в `mw_domain::document_template`, проверка и её границы кратко;
  - таблица `document_templates`, «стандартный — `NULL`», версия не обнуляется;
  - права `settings_manage`; черновик превью через `export_document`;
  - раскладки и их подписи; запрет `fill` и вложенных колонок;
  - `.mwtemplate` — формат 1, импорт не сохраняет;
  - нечитаемая модель печатается стандартной.
- [ ] **Step 2: Спека интерфейса** — маршрут `/templates/[kind]`, ширины `lg`/`md`/`xs`, очередь превью, клавиши отмены.
- [ ] **Step 3: README** — план 7.1 «выполнен». В «Заметки → План 7.2» — пункты решения 12 и всё, что парковали ревью этого плана. Из заметок «План 7» убрать сделанное.
- [ ] **Step 4: AGENTS.md** — два правила:
  - «Шаблоны документов пишутся только через `Core::save_document_template`; модель проверяет `Template::validate`; новые поля модели — с `#[serde(default)]`, чтобы шаблоны в БД читались»;
  - «Превью несохранённого шаблона — только `DocumentRequest.draft` через `Core::export_document`».
- [ ] **Step 5: Commit** — `docs: план 7.1 выполнен, заметки плана 7.2`.
