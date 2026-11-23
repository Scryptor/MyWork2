# План 7.2 «Массовая печать, логотип, штрихкоды и шрифты» — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Цель:** из списка заказов печатаются акты и наклейки сразу для выбранных заказов. На актах есть логотип фирмы. Штрихкоды MyWork 2, которые не подходят под UPC-A, печатаются кодом Code 128. Знаки, которых нет в Libertinus, печатаются запасными шрифтами. Свой текст старых шаблонов MyWork 2 можно вставить в редактор. Доводка редактора по итогам плана 7.1 закрыта.

**Архитектура:**
- Штрихкод: `mw-docs` выбирает символику по значению — UPC-A, если код верный, иначе Code 128. Модуль по-прежнему в целых точках принтера.
- Шрифты: встроенный Libertinus, затем шрифты из папки `fonts` в папке данных, затем шрифты системы. Typst сам ищет недостающий знак во всех шрифтах книги.
- Логотип: одна картинка фирмы. Хранится как вложение вида `logo` через `mw-core::attachments` и попадает в Typst виртуальным файлом `logo.png`. Новый блок модели `Logo`.
- Пачка документов: каждый заказ компилируется отдельно, страницы склеиваются в один `PagedDocument` (`PagedDocument::new` в `typst-layout` 0.15.1 сам строит интроспектор). Программа для компьютера печатает пачку заданиями по одному заказу, PDF пачки отдаёт новый API.
- Старые шаблоны: текст `Shablon_*.htm` из `legacy_rows` с метками MyWork 2, заменёнными на поля, показывается в редакторе и вставляется блоком текста.

**Tech Stack:** Rust (Typst 0.15.1, `fontdb`, `image`), Axum 0.8 + utoipa + ts-rs, SvelteKit 3 + Svelte 5, Tauri 2.

**Spec:** `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` (§5.1, §5.2) и спека интерфейса `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (раздел 7); заметки «План 7.2» в `docs/superpowers/plans/README.md`.

## Global Constraints

- Код и идентификаторы на английском; тексты для людей и документация на русском.
- `mw-domain` без ввода-вывода. К БД обращаться только через `mw-db` и `mw-core`; права проверяет `mw-core`. Любая запись в БД — только через `Core::begin_write()`.
- Сначала тест, потом код. Каждый тест с БД идёт по `all_backends()`.
- Перед коммитом зелёным должно быть: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`. Для интерфейса: `cd apps/ui && npm run verify && npm run test:e2e`.
- Время — только `Core::now()` и `Core::now_utc()`.
- Выпущенные миграции не меняются, только добавляются новые.
- Типы API — это типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS` и `utoipa::ToSchema`. Входные типы — с `deny_unknown_fields`. `#[ts(export)]` не ставить.
- `crates/mw-api/openapi.json` и `crates/mw-api/bindings/` генерируются: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`. Затем `cargo test -p mw-api --test contract`; коммит вместе с кодом.
- Документы: данные заказа никогда не становятся разметкой Typst — только `data.json` и виртуальные файлы (`barcode.svg`, с этого плана ещё `logo.png`). Рендерит только `Core::export_document` (и новый `Core::export_documents`) под семафором; `mw-docs` из API и интерфейса не зовут.
- Вложения: файлы — только через `mw-core::attachments`, пути — через `store::resolve`. Загрузка — телом `application/octet-stream`; `multipart/form-data` и прочие «простые» типы запрещены.
- Модель шаблона: новые поля — с `#[serde(default)]`; каждое изменение модели повышает `TEMPLATE_FILE_VERSION`; `Template::validate` не сужать.
- Зависимости — только MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode; OFL-1.1 — только шрифты. После изменения зависимостей в `crates/*` обновить `apps/desktop/src-tauri/Cargo.lock`.
- Правила интерфейса (`AGENTS.md`):
  - ширины раскладки — только `display` и `:global([data-width=…])`; цвета и размеры — только из `tokens.css`;
  - Svelte 5 только на рунах, обработчики — пропсы `on…`;
  - импорты — `#lib/...` с явными расширениями, типы API — `import type` из `$api/...`;
  - компоненты — из `#lib/ui/...`; тексты — в `#lib/i18n/ru.ts`; иконки — `@lucide/svelte`, по одной;
  - `fetch` — только в `src/lib/api/client.ts`; картинки — только через `attachmentUrl` или такой же `blob:`, не прямыми ссылками на API;
  - сквозные тесты — Chromium и WebKit, запросы по ролям и подписям, данные через API с `uniq()`.
- Окружение для каждой команды:
  ```
  export PATH="/opt/homebrew/opt/node@24/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:/opt/homebrew/bin:$PATH"
  export CARGO_TARGET_DIR=/Users/ruf/dev/MyWork2/target
  export MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres
  ```
  `docker start mw-pg` (при таймаутах пула — `docker restart mw-pg`). `rustc`/`cargo` не запускать вне worktree: rustup сам поставит лишний stable. Программу для компьютера не запускать.

## Решения плана

1. **«Свой Typst-код» уходит в план 7.3.** В Typst 0.15.1 компиляцию нельзя прервать изнутри процесса: у `World` нет отмены. Встроенных пределов два: 10 000 шагов цикла `while` и глубина вызовов 80, а `for` по `range(…)` не ограничен ничем. Настоящий предел времени даёт только отдельный процесс рендера, который можно убить. Это отдельная работа, и её проще проверить в своём плане. В дорожной карте появится строка 7.3.
2. **Code 128 для кодов не в UPC-A.**
   - Верный 12-значный UPC-A печатается как раньше.
   - Любое другое значение из печатных знаков ASCII 32–126 длиной до 40 знаков печатается кодом Code 128:
     - набор C, если значение — только цифры чётной длины не меньше 4;
     - иначе набор B, без переключений внутри кода.
   - Всё остальное (кириллица, управляющие знаки, длиннее 40) печатается без штрихкода, как сейчас любой неверный код.
   - Модуль — целое число точек принтера (те же `fit_module_mm` и `stretch_module_mm`). Тихая зона — 10 модулей с каждой стороны. Цифры под кодом — значение как есть.
   - Поиск по отсканированному коду уже ищет точное значение (`normalize_scanned` только обрезает пробелы), поэтому Code 128 находит заказ без правок.
3. **Запасные шрифты.**
   - Порядок в книге шрифтов:
     1. встроенный Libertinus;
     2. файлы `.ttf`/`.otf`/`.ttc` из `<папка данных>/fonts` (только верхний уровень);
     3. шрифты системы (`fontdb::Database::load_system_fonts`).
   - Typst с `fallback: true` (по умолчанию) сам найдёт знак в другом шрифте. Документ, в котором все знаки есть в Libertinus, выглядит как раньше на любом компьютере.
   - Книга строится один раз, при первом рендере; сервер заранее строит её в фоне после старта. Файлы шрифтов читаются, только когда шрифт понадобился.
   - Повреждённый файл пропускается с `tracing::warn!`.
   - Замена «₽» → «руб.» остаётся: это решение плана 6 про вид документа, а не про шрифт.
   - В Docker без шрифтов системы запасной шрифт — папка `fonts`; это пишется в документации.
4. **Логотип — одна картинка фирмы**, а не библиотека картинок.
   - Загружается в «Документы и реквизиты».
   - Принимаются PNG, JPEG и WebP до 5 МиБ. Картинка разбирается, уменьшается так, чтобы длинная сторона была не больше 1200 px, и пересохраняется в PNG без метаданных; прозрачность сохраняется.
   - Хранится строкой `attachments` вида `logo` (`order_id` и `draft_key` — `NULL`, поэтому очистка черновиков её не трогает) и файлом `attachments/logo/<id>.png`.
   - Новая загрузка заменяет старую. Удаление — отдельной кнопкой.
   - Отдельный блок «Картинка» с своей библиотекой — потом, когда попросят.
5. **Блок `Logo`** (`width_mm` 5–200, `height_mm` 5–100, `align`).
   - Картинка вписывается в прямоугольник с сохранением пропорций.
   - Без логотипа блок не печатает ничего и не занимает места.
   - Стандартные акты (четыре акта и пустой акт приёма) получают `Logo` 40 × 15 мм слева первым блоком шапки. Без логотипа они печатаются ровно как раньше; это проверяется байтовым совпадением PNG.
   - В файле `.mwtemplate` блок ссылается на логотип фирмы, а не несёт картинку, поэтому файл остаётся маленьким и переносимым. Модель меняется, поэтому `TEMPLATE_FILE_VERSION` становится 2.
6. **Массовая печать.**
   - Программа для компьютера печатает выбранные заказы заданиями по одному заказу, друг за другом, тем же `send`, что и одиночная печать. Windows получает растр по одному заказу, поэтому огромных ответов с сотней страниц PNG нет.
   - PDF пачки (кнопка «Сохранить PDF», а в браузере — единственный путь) строит новый `Core::export_documents`:
     - до 100 заказов;
     - каждый заказ — своя компиляция с прежним пределом в 50 страниц, всего не больше 200 страниц;
     - вся пачка под одним местом семафора;
     - шаблон берётся один раз на пачку.
   - Если заказа нет, падает вся пачка с ошибкой «Заказ № N не найден»: интерфейс скажет, какой снять.
7. **Старые шаблоны MyWork 2 переносятся текстом, а не раскладкой.**
   - Разобрать вёрстку Word в блоки надёжно нельзя, а свой текст в старом шаблоне — это то, что жалко потерять.
   - Ядро отдаёт текст `Shablon_*.htm` (`plain_text`) с метками MyWork 2, заменёнными на поля (таблица — в задаче 8). Редактор показывает его и вставляет блоком «Текст».
   - `Shablon_Priem.htm` в коде MyWork 2 не используется и не показывается.
   - `[ACTS] BarcodeH/BarcodeW` не переносятся: это пиксели GIF из BarcodeLib на экранной сетке GDI. Новый штрихкод считается в точках принтера. Значения остаются в `legacy_rows`.
8. **Поля наклейки.** Сервер не знает размер наклейки рабочего места: он хранится локально. Поэтому свойства страницы наклейки в редакторе предупреждают, если на наклейке этого рабочего места (из «Настройки → Печать») поля оставляют меньше минимальной области. Обрезки и перестановки полей при печати нет.
9. **Гонка «отмена после 409»** исправляется по схеме из итогового ревью 7.1:
   - `send()` помечает сброшенную запись, если запись шаблона ещё в пути;
   - `catchUp` пересохраняет только для ответа своей записи;
   - перечитанная после конфликта запись принимается как есть (`follow`).

## Review Focus

1. Значение штрихкода, которое не кодируется ни UPC-A, ни Code 128 (кириллица, перевод строки, 41 знак): документ печатается без штрихкода и без ошибки. Тест — в задаче 1.
2. Массовая печать, в которой один заказ падает посередине (удалён, нет прав, слишком много страниц): остальные напечатаны, упавший назван номером, повторное нажатие во время печати ничего не задваивает, «Остановить» останавливает после текущего. Тест — в задаче 7.
3. Загрузка логотипа:
   - не картинка, обрезанный PNG, 6 МиБ — понятная ошибка 422 на русском;
   - прозрачный PNG, 16-битный PNG, JPEG в оттенках серого — сохранённый PNG, прозрачность цела.

   Тест — в задаче 3.
4. Блок «Логотип» без загруженного логотипа и после удаления логотипа: документ печатается без пустого места и без ошибки. Стандартный акт без логотипа совпадает с прежним байт в байт. Тест — в задаче 4.
5. В папке `fonts` лежит повреждённый файл или не шрифт: он пропущен с предупреждением, рендер работает. Знака нет ни в одном шрифте: документ строится без паники. Тест — в задаче 2.

---

### Task 1: Code 128 для штрихкодов не в UPC-A

**Files:**
- Modify: `crates/mw-docs/src/barcode.rs`, `crates/mw-docs/src/typst_gen/mod.rs`, `crates/mw-docs/src/typst_gen/blocks.rs`, `crates/mw-docs/src/lib.rs`
- Test: тесты в `barcode.rs`, `crates/mw-docs/src/layout_tests.rs`, `crates/mw-core/tests/documents.rs`

**Interfaces:**
- Consumes: `fit_module_mm`, `stretch_module_mm`, `upc_a_svg`, `upc_a_size_mm` (план 6 и 7.1).
- Produces:
  - `pub enum Symbology { UpcA, Code128 }`;
  - `pub fn symbology(value: &str) -> Option<Symbology>` — решение 2;
  - `pub(crate) fn code128_symbols(value: &str) -> Option<Vec<u8>>` — значения символов: старт, данные, контрольный, стоп `106`;
  - `pub fn code128_svg(value: &str, module_mm: f64, height_mm: f64) -> Option<String>`;
  - `pub(crate) fn barcode_size_mm(value: &str, module_mm: f64, height_mm: f64) -> Option<(f64, f64)>` — общий размер для обеих символик; генератор берёт его вместо `upc_a_size_mm`;
  - `DocumentData.barcode` теперь значение любой символики (комментарий поправить).

Code 128: шаблоны штрихов — стандартная таблица из 107 символов (каждый — 6 ширин на 11 модулей, стоп — 7 ширин на 13 модулей). Контрольный символ = (старт + Σ значение × позиция) mod 103, позиции с 1. Набор B: значение = код ASCII − 32. Набор C: пара цифр = число 00–99.

- [ ] **Step 1: Написать падающие тесты** в `barcode.rs`:
  ```rust
  #[test] fn symbology_by_value() // "036000291452" → UpcA; "036000291453" (неверная контрольная) → Code128; "MW-1" → Code128;
                                  // "Заказ" → None; "a\nb" → None; 40 × "7" → Code128; 41 × "7" → None; "" → None
  #[test] fn code128_symbols_b_and_c() // "A" → [104, 33, 34, 106]; "1234" → [105, 12, 34, 82, 106]; "123" → набор B: [104, 17, 18, 19, 8, 106]
  #[test] fn code128_patterns_are_well_formed() // все 107 шаблонов разные; 0–105 — 6 ширин с суммой 11; стоп «2331112» — сумма 13;
                                                // старт A «211412», старт B «211214», старт C «211232»
  #[test] fn code128_svg_round_trips() // декодер в тесте читает ширины полос из SVG и получает исходные символы для "MW-1" и "1234"
  #[test] fn code128_width_is_whole_dots() // при dpi 203 ширина SVG = (11 × символов + 2 + 20) × модуль, где символов — вместе со стартом,
                                           // контрольным и стопом; модуль = 3 точки 203 dpi
  ```
  В `layout_tests.rs`:
  ```rust
  #[test] fn sticker_prints_code128_for_legacy_value() // стандартная наклейка, barcode "A-1053", dpi 203 → в документе есть barcode.svg, ширина ≤ ширины наклейки
  #[test] fn unencodable_value_prints_without_barcode() // barcode "Заказ 5" → документ строится, штрихкода нет (Review Focus 1)
  ```
  В `crates/mw-core/tests/documents.rs` (по `all_backends()`): `legacy_barcode_is_printed_and_found` — заказ со штрихкодом `"A-1053"` (как после импорта): наклейка PDF строится, а поиск по отсканированному `"A-1053\n"` находит этот заказ.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-docs && cargo test -p mw-core --test documents legacy_barcode`. Expected: FAIL (нет `symbology`/`code128_*`).
- [ ] **Step 3: Реализовать** символику, SVG Code 128 (полосы прямоугольниками той же высоты, что у UPC, цифры — значение как есть) и выбор символики в `render`/генераторе.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-docs && cargo test -p mw-core --test documents`. Expected: PASS. Документы с верным UPC-A не изменились (существующие тесты штрихкода зелёные без правок).
- [ ] **Step 5: Commit** — `feat(docs): Code 128 для штрихкодов не в UPC-A`.

### Task 2: Запасные шрифты

**Files:**
- Modify: `crates/mw-docs/Cargo.toml`, `crates/mw-docs/src/fonts.rs`, `crates/mw-docs/src/world.rs`, `crates/mw-docs/src/render.rs`, `crates/mw-docs/src/typst_gen/mod.rs`, `crates/mw-docs/src/lib.rs`, `crates/mw-server/src/main.rs`, `apps/desktop/src-tauri/src/server.rs` (или место, где программа для компьютера запускает сервер), `apps/desktop/src-tauri/Cargo.lock`
- Test: `crates/mw-docs/tests/fonts.rs`

**Interfaces:**
- Produces:
  - `pub struct FontSources { pub dirs: Vec<PathBuf>, pub system: bool }`;
  - `pub fn configure_fonts(sources: FontSources)` — запоминает источники (дёшево, до первого рендера; повторный вызов после построения книги ничего не меняет и пишет `warn!`);
  - `pub fn warm_fonts()` — строит книгу сейчас (сервер зовёт в `spawn_blocking` после старта);
  - `pub struct FontSet` и `FontSet::new(&FontSources) -> FontSet`;
  - `pub fn render_with_fonts(template, data, opts, fonts: &FontSet) -> Result<Rendered, DocsError>` — `render` зовёт его с общей книгой;
  - `font_covers` проверяет всю общую книгу.
- `mw-server` и программа для компьютера зовут `configure_fonts(FontSources { dirs: vec![<папка данных>/fonts], system: true })` до запуска сервера и `warm_fonts()` в фоне. Без вызова (тесты) — только встроенные шрифты.

Лучше один раз собрать `FontInfo` всех лиц (`fontdb` даёт пути и индексы лиц; `FontInfo` — из `typst::text::FontInfo::new`/`Font::new` над прочитанным файлом), а сам `Font` держать в слоте, который читает файл при первом `World::font(i)`.

- [ ] **Step 1: Написать падающие тесты** в `crates/mw-docs/tests/fonts.rs`. Dev-dependency: `typst-assets = { version = "0.15.1", features = ["fonts"] }` — только для тестов, в программу не попадает. Знак для проверки — тот, которого нет в Libertinus и есть в DejaVu Sans Mono (например, «█» U+2588; проверить `font_covers` встроенного набора в самом тесте).
  ```rust
  #[test] fn fallback_font_from_dir_prints_missing_glyph() // во временной папке DejaVuSansMono.ttf; FontSet с этой папкой, system: false;
                                                           // текст «█» → в PDF есть шрифт DejaVuSansMono; со встроенным набором — нет
  #[test] fn broken_font_file_is_skipped()   // в папке «broken.ttf» из 100 случайных байт и «notes.txt» → FontSet строится, рендер работает (Review Focus 5)
  #[test] fn missing_glyph_everywhere_does_not_panic() // знак U+10FFFD без шрифта → документ строится
  #[test] fn covered_text_looks_the_same()   // стандартный акт приёма на примерных данных: PNG со встроенным набором и с набором + DejaVu совпадают байт в байт
  ```
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-docs --test fonts`. Expected: FAIL (нет `FontSet`).
- [ ] **Step 3: Реализовать** `FontSet` (встроенные лица первыми, затем папки, затем система), ленивые слоты, `configure_fonts`/`warm_fonts`, вызовы в `mw-server` и программе для компьютера. Зависимость `fontdb` (MIT) — та версия, что уже тянет `usvg`, если она есть в дереве. Обновить `apps/desktop/src-tauri/Cargo.lock`.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-docs && cargo test -p mw-core --test documents && cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(docs): запасные шрифты из папки данных и системы`.

### Task 3: Логотип фирмы — хранение и API

**Files:**
- Create: `crates/mw-core/src/attachments/logo.rs`, `crates/mw-api/src/routes/logo.rs`
- Modify: `crates/mw-core/src/attachments/mod.rs` (списки вложений заказа не видят вид `logo`), `crates/mw-core/src/attachments/store.rs`, `crates/mw-domain/src/validation.rs`, `crates/mw-api/src/routes/mod.rs`, `crates/mw-api/tests/contract.rs`
- Test: `crates/mw-core/tests/logo.rs`, `crates/mw-api/tests/logo.rs`

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, TS, ToSchema)] pub struct FirmLogo { pub width: i32, pub height: i32, pub size: i64 }`;
  - `Core::firm_logo(&self, actor) -> Result<Option<FirmLogo>, CoreError>` — любой вошедший;
  - `Core::firm_logo_file(&self, actor) -> Result<Option<AttachmentFile>, CoreError>` — любой вошедший;
  - `Core::set_firm_logo(&self, actor, bytes: Vec<u8>) -> Result<FirmLogo, CoreError>` — `SettingsManage`, через `begin_write()`, старая строка и файл удаляются, журнал `settings.logo_updated`;
  - `Core::remove_firm_logo(&self, actor) -> Result<(), CoreError>` — `SettingsManage`, журнал `settings.logo_removed`; без логотипа — `Ok`, без записи в журнал;
  - `pub(crate) async fn firm_logo_png(&self) -> Option<Arc<[u8]>>` — байты для рендера (задача 4);
  - API:
    - `GET /api/v1/settings/logo` — `image/png` или 404;
    - `GET /api/v1/settings/logo/info` — `FirmLogo` или 404;
    - `PUT /api/v1/settings/logo` — тело `application/octet-stream`, ответ `FirmLogo`; предел тела маршрута — 6 МиБ (больше — 413 от Axum), а 5 МиБ проверяет ядро (422);
    - `DELETE /api/v1/settings/logo` — 204.
- Ошибки проверки (тексты — в `FieldError::message_ru`):
  - `logo`/`unsupported` — «Логотип — картинка PNG, JPEG или WebP»;
  - `logo`/`too_large` — уже есть: «Файл слишком большой».

- [ ] **Step 1: Написать падающие тесты.** `crates/mw-core/tests/logo.rs`, все по `all_backends()`, картинки генерировать в тесте (`image`):
  ```rust
  #[tokio::test] async fn upload_replaces_and_keeps_alpha()   // прозрачный PNG 64×32 → FirmLogo{64,32}; файл — PNG с альфой; второй загрузкой JPEG → старый файл удалён, строка одна
  #[tokio::test] async fn big_logo_is_downscaled()            // PNG 3000×1000 → 1200×400
  #[tokio::test] async fn odd_images_become_png()             // 16-битный PNG (Rgba16) и JPEG в оттенках серого → PNG 8 бит (Review Focus 3);
                                                              // CMYK JPEG `image` в тесте не пишет — его разбор остаётся на `image`
  #[tokio::test] async fn not_an_image_is_rejected()          // b"hello", обрезанный PNG → Validation logo/unsupported; 5 МиБ + 1 байт → logo/too_large
  #[tokio::test] async fn logo_needs_settings_manage()        // без права → Forbidden; чтение — любой вошедший
  #[tokio::test] async fn logo_is_not_an_order_attachment()   // логотип не в list_order_attachments, cleanup_drafts его не удаляет
  #[tokio::test] async fn remove_logo_is_idempotent()         // удалить дважды → Ok, запись журнала одна
  ```
  `crates/mw-api/tests/logo.rs`: PUT с `application/octet-stream` → 200; PUT с `multipart/form-data` → 415; GET без логотипа → 404; GET после загрузки → `image/png`; 5 МиБ + 1 байт → 422; 7 МиБ → 413.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-core --test logo && cargo test -p mw-api --test logo`. Expected: FAIL.
- [ ] **Step 3: Реализовать** обработку картинки (`image::guess_format` по содержимому, только PNG/JPEG/WebP, уменьшение до 1200 px по длинной стороне, PNG без метаданных), хранение через `store::resolve`, маршруты и предел тела. Перегенерировать контракт.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-core && cargo test -p mw-api`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): логотип фирмы — загрузка, хранение, API`.

### Task 4: Блок «Логотип» в модели, генераторе и стандартных актах

**Files:**
- Modify: `crates/mw-domain/src/document_template.rs`, `crates/mw-docs/src/typst_gen/{mod,blocks}.rs`, `crates/mw-docs/src/standard.rs`, `crates/mw-core/src/documents.rs`, `crates/mw-api/tests/contract.rs`
- Test: тесты в `document_template.rs`, `crates/mw-docs/src/layout_tests.rs`, `crates/mw-core/tests/documents.rs`

**Interfaces:**
- Consumes: `Core::firm_logo_png` (задача 3).
- Produces:
  - `BlockBody::Logo { width_mm: f32, height_mm: f32, align: Align }` (`type: "logo"`), проверка `out_of_range` для `width_mm` вне 5–200 и `height_mm` вне 5–100;
  - `TEMPLATE_FILE_VERSION = 2` (файл версии 1 читается как раньше);
  - `DocumentData.logo: Option<Arc<[u8]>>` (PNG); в `data.json` — `"logo": true|false`; виртуальный файл `logo.png` — только когда логотип есть и в шаблоне есть блок `Logo`;
  - `Core` кладёт логотип в `DocumentData` при каждом рендере.

Вывод блока: `#if data.logo { align(<align>, image("logo.png", width: <w>mm, height: <h>mm, fit: "contain")) }`. Без логотипа `if` не даёт содержимого, и лишнего отступа блока нет.

- [ ] **Step 1: Написать падающие тесты.**
  В `document_template.rs`:
  - `logo_block_limits` — 4,9 и 200,1 по ширине, 4,9 и 100,1 по высоте дают `out_of_range`;
  - `version_1_file_still_reads` — файл с `format_version: 1` без блоков логотипа читается;
  - `logo_round_trips` — JSON ↔ модель.

  В `layout_tests.rs`:
  ```rust
  #[test] fn logo_fits_its_box()                // логотип 400×100 px в блоке 40×15 → картинка 40×10 мм (contain), слева
  #[test] fn missing_logo_takes_no_space()      // шаблон [Text, Logo, Text] без логотипа → те же страницы PNG, что у [Text, Text] (Review Focus 4)
  #[test] fn standard_act_without_logo_is_unchanged() // стандартный акт приёма на примерных данных без логотипа: PNG байт в байт равен PNG шаблона без блока Logo
  #[test] fn standard_acts_start_with_logo()    // у четырёх актов и пустого акта приёма первый блок шапки — Logo 40×15, align Left; у наклейки Logo нет
  ```
  В `crates/mw-core/tests/documents.rs` (по `all_backends()`): `act_prints_uploaded_logo` (после `set_firm_logo` PDF акта приёма содержит картинку; после `remove_firm_logo` — без картинки и без ошибки).
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-domain document_template && cargo test -p mw-docs && cargo test -p mw-core --test documents`. Expected: FAIL.
- [ ] **Step 3: Реализовать** вариант модели, вывод, виртуальный файл, правку стандартных актов и подачу логотипа из ядра. Перегенерировать контракт (`BlockBody.ts`).
- [ ] **Step 4: Прогнать.** Run: `cargo test --workspace`. Expected: PASS. Документы без логотипа совпадают с прежними.
- [ ] **Step 5: Commit** — `feat(docs): блок «Логотип» и логотип в шапке стандартных актов`.

### Task 5: Логотип в интерфейсе — «Документы и реквизиты» и свойства блока

**Files:**
- Create: `apps/ui/src/lib/api/logo.ts`, `apps/ui/src/lib/templates/props/LogoProps.svelte`, `apps/ui/src/lib/settings/FirmLogo.svelte` (или рядом с остальными частями страницы «Документы и реквизиты»)
- Modify: `apps/ui/src/routes/(app)/settings/documents/+page.svelte`, `apps/ui/src/lib/templates/{defaults.ts,Properties.svelte,BlockList.svelte}`, `apps/ui/src/lib/i18n/ru.ts`
- Test: `apps/ui/src/lib/api/logo.test.ts`, `apps/ui/src/lib/templates/props/LogoProps.test.ts`, `apps/ui/e2e/documents-settings.spec.ts` (или существующая спека страницы «Документы и реквизиты»)

**Interfaces:**
- Consumes: API задачи 3, `BlockBody` с `logo` (задача 4).
- Produces:
  - `getLogoInfo(): Promise<FirmLogo | null>`;
  - `logoUrl(): Promise<string | null>` — `blob:` по образцу `attachmentUrl`, освобождается при замене;
  - `uploadLogo(file: Blob): Promise<FirmLogo>` (`application/octet-stream`);
  - `removeLogo(): Promise<void>`.

Поведение:
- **«Документы и реквизиты»**, раздел «Логотип» (только с правом настроек):
  - превью картинки;
  - «Загрузить…» (выбор файла `image/png,image/jpeg,image/webp`) и «Удалить» с подтверждением;
  - ошибка 422 — текстом ошибки;
  - без права — только картинка или «Логотип не загружен».
- **Редактор шаблонов:**
  - блок «Логотип» в списке «Добавить блок» (`defaults.ts`: 40 × 15 мм, слева);
  - свойства — ширина, высота, выравнивание;
  - если логотип не загружен, в свойствах подсказка «Логотип не загружен — блок не печатается» со ссылкой на «Документы и реквизиты».

- [ ] **Step 1: Написать падающие тесты** (vitest):
  - `logo.test.ts` — запросы и освобождение `blob:`;
  - `LogoProps.test.ts`:
    - правка ширины даёт `edit` с новым значением;
    - без логотипа видна подсказка со ссылкой.

  e2e (Chromium и WebKit): `uploads_and_removes_logo`:
  1. загрузить PNG (сгенерировать в тесте);
  2. на странице видна картинка;
  3. предпросмотр акта приёма по кнопке страницы строится;
  4. «Удалить» → «Логотип не загружен».

  В `afterEach` логотип удалить через API.
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/api/logo.test.ts src/lib/templates/props/LogoProps.test.ts`. Expected: FAIL.
- [ ] **Step 3: Реализовать** модуль API, раздел страницы, блок и свойства; раздел витрины `/design` не нужен (новых общих компонентов нет — если появится, добавить раздел).
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify && npx playwright test e2e/documents-settings.spec.ts e2e/template-editor.spec.ts e2e/layout.spec.ts`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(ui): логотип в «Документы и реквизиты» и блок «Логотип» в редакторе`.

### Task 6: Документ пачкой заказов — ядро и API

**Files:**
- Modify: `crates/mw-docs/src/render.rs` (склейка), `crates/mw-docs/src/lib.rs`, `crates/mw-core/src/documents.rs`, `crates/mw-domain/src/validation.rs`, `crates/mw-api/src/routes/documents.rs`, `crates/mw-api/src/dto.rs`, `crates/mw-api/src/routes/mod.rs`
- Test: `crates/mw-docs/tests/merge.rs`, `crates/mw-core/tests/documents.rs`, `crates/mw-api/tests/documents.rs`

**Interfaces:**
- Produces:
  - `pub fn merge(docs: Vec<Rendered>) -> Rendered` в `mw-docs` — страницы по порядку, `PagedDocument::new(pages, info первого)`;
  - `pub struct DocumentBatchRequest { pub kind: DocumentKind, pub order_ids: Vec<i64>, pub options: RenderOptions }` в `mw-core`;
  - `Core::export_documents(&self, actor, req: DocumentBatchRequest) -> Result<DocumentFile, CoreError>` — только PDF;
  - API: `POST /api/v1/documents/{kind}/batch`:
    - тело `DocumentBatch { order_ids: Vec<i64>, label_w_mm, label_h_mm, offset_x_mm, offset_y_mm, printer_dpi }` (все, кроме `order_ids`, необязательны, как в `DocumentQuery`), `deny_unknown_fields`;
    - ответ `application/pdf`, `Content-Disposition: inline; filename="<вид>-<число заказов>.pdf"`.
- Правила (решение 6):
  - вид должен быть `for_order()`, иначе `kind`/`not_allowed`;
  - `order_ids` пусто → `order_ids`/`required`; больше 100 → `order_ids`/`too_many`;
  - повторы убираются с сохранением порядка;
  - заказа нет → `NotFound` с текстом «Заказ № N не найден»;
  - права — как у документа одного заказа;
  - больше 50 страниц у одного заказа или больше 200 всего → `template`/`too_many_pages`;
  - вся пачка — одно задание `render_gate`.

- [ ] **Step 1: Написать падающие тесты.**
  - `crates/mw-docs/tests/merge.rs` — `merged_pdf_has_all_pages`: три рендера наклейки по 1 странице → PDF с 3 страницами (`/Type /Page` встречается 3 раза или разбор через тот же способ, что в существующих тестах PDF); у PNG склейки 3 страницы.
  - `crates/mw-core/tests/documents.rs` (по `all_backends()`):
    ```rust
    #[tokio::test] async fn batch_pdf_has_a_document_per_order()  // 3 заказа, наклейка → 3 страницы; акт приёма «2 на лист A4» → 3 страницы
    #[tokio::test] async fn batch_limits()                         // 0 заказов → order_ids/required; 101 → order_ids/too_many; калибровка → kind/not_allowed
    #[tokio::test] async fn batch_drops_repeats()                  // [a, b, a] → 2 страницы наклейки
    #[tokio::test] async fn batch_missing_order_names_it()         // [a, 999999] → NotFound, текст содержит «999999»
    #[tokio::test] async fn batch_total_page_cap()                 // сохранить шаблон наклейки, который даёт 3 страницы на заказ (длинный текст на наклейке 58×40),
                                                                   // и 67 заказов → 201 страница → template/too_many_pages
    ```
  - `crates/mw-api/tests/documents.rs`: POST batch → 200 `application/pdf`; лишнее поле в теле → 400; без входа → 401.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-docs --test merge && cargo test -p mw-core --test documents batch && cargo test -p mw-api --test documents batch`. Expected: FAIL.
- [ ] **Step 3: Реализовать** склейку, `export_documents` (данные всех заказов собираются до семафора, шаблон — один раз) и маршрут. Перегенерировать контракт.
- [ ] **Step 4: Прогнать.** Run: `cargo test -p mw-docs && cargo test -p mw-core && cargo test -p mw-api`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(core): документ PDF пачкой заказов`.

### Task 7: Массовая печать из списка заказов

**Files:**
- Modify: `apps/ui/src/lib/print/print.svelte.ts`, `apps/ui/src/lib/api/documents.ts`, `apps/ui/src/routes/(app)/orders/+page.svelte`, `apps/ui/src/lib/i18n/ru.ts`
- Create: `apps/ui/src/lib/print/BulkPrintMenu.svelte`
- Test: `apps/ui/src/lib/print/print.test.ts`, `apps/ui/e2e/bulk-print.spec.ts`

**Interfaces:**
- Consumes: API пачки (задача 6), `send`, `printerName`, `canPrintDirectly`, `showNoPrinter` из `print.svelte.ts`.
- Produces:
  - `documentBatchPdf(kind: DocumentKind, orderIds: number[], opts: PdfOptions): Promise<Blob>` в `#lib/api/documents.ts`;
  - `printBatch(kind, orders: { id: number; number: string }[], opts: { onprogress?: (done: number, total: number) => void; signal?: AbortSignal }): Promise<BatchPrintResult>`;
  - `interface BatchPrintResult { sent: number; failed: { number: string; text: string }[]; stopped: boolean; printer: string | null }`;
  - `savePdfBatch(kind, orderIds): Promise<void>`;
  - `BULK_PRINT_LIMIT = 100`.

Поведение (решение 6):
- **Меню «Печать ▾».**
  - В панели выбора, рядом с массовыми действиями; есть и без права правки заказов.
  - Меню из двух разделов: «Печать» (только при прямой печати) и «Сохранить PDF». В каждом — пять документов заказа: акт приёма, акт выдачи, два гарантийных акта, наклейка.
- **Больше 100 выбранных** — пункты недоступны с подсказкой «Не больше 100 заказов за раз».
- **«Печать».**
  - Принтер выбирается один раз; затем заказы идут по одному через `send`.
  - Во время печати панель выбора заблокирована, в ней текст «Печать: 3 из 12» и кнопка «Остановить» — она останавливает после текущего заказа.
  - Итог:
    - всё ушло — уведомление «Отправлено на «<принтер>»: 12»;
    - были сбои — уведомление об опасности «Не напечатаны: № 15, № 18» (до пяти номеров и «и ещё N»).
  - Нет принтера — то же уведомление, что у одиночной печати, один раз.
  - Повторное нажатие во время печати ничего не делает.
- **«Сохранить PDF»** — `savePdfBatch`, как у одиночного `savePdf`. При ошибке 404 текст ошибки говорит номер заказа.

- [ ] **Step 1: Написать падающие тесты** (vitest, команды Tauri и API подменены, как в существующем `print.test.ts`):
  ```ts
  test('printBatch sends each order once, in order')          // 3 заказа → 3 вызова печати с orderId по порядку, sent = 3
  test('printBatch goes on after a failure')                  // второй падает → sent = 2, failed = [{ number: '№ второго', … }] (Review Focus 2)
  test('printBatch stops after the current order')            // abort после первого → sent = 1, stopped = true
  test('printBatch without a printer tells it once')          // listPrinters → [] → ни одного send, одно уведомление
  test('a second printBatch while one runs is dropped')       // два вызова подряд → печать один раз
  ```
  e2e `bulk-print.spec.ts` (Chromium и WebKit; в браузере прямой печати нет):
  1. создать 3 заказа с меткой `uniq()`, найти их в списке, выбрать;
  2. «Печать ▾» → «Наклейка» → «Сохранить PDF»;
  3. скачанный файл начинается с `%PDF` и содержит 3 страницы.
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/print/print.test.ts`. Expected: FAIL.
- [ ] **Step 3: Реализовать** функции печати пачкой, меню и подключение в `orders/+page.svelte` (убрать комментарий «comes with plan 6»).
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify && npx playwright test e2e/bulk-print.spec.ts e2e/orders.spec.ts e2e/layout.spec.ts`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat(ui): массовая печать и PDF выбранных заказов`.

### Task 8: Текст старых шаблонов MyWork 2 в редакторе

**Files:**
- Create: `crates/mw-core/src/legacy_templates.rs`
- Modify: `crates/mw-legacy/src/fixture.rs`, `crates/mw-core/src/lib.rs`, `crates/mw-api/src/routes/document_templates.rs`, `apps/ui/src/lib/api/document-templates.ts`, `apps/ui/src/lib/templates/{TemplateFileMenu.svelte,TemplateEditor.svelte}`, `apps/ui/src/lib/i18n/ru.ts`
- Create: `apps/ui/src/lib/templates/LegacyTemplateDialog.svelte`
- Test: `crates/mw-legacy` (тест фикстуры), `crates/mw-core/tests/legacy_templates.rs`, `crates/mw-api/tests/document_templates.rs`, `apps/ui/src/lib/templates/LegacyTemplateDialog.test.ts`

**Interfaces:**
- Consumes: `mw_core::documents::plain_text`, `legacy_rows` (`source = "file"`, `source_key` — имя файла, `raw.text` — текст).
- Produces:
  - `#[derive(Serialize, Deserialize, TS, ToSchema)] pub struct LegacyTemplate { pub file: String, pub text: String }`;
  - `Core::legacy_document_template(&self, actor, kind) -> Result<Option<LegacyTemplate>, CoreError>` — любой вошедший;
  - `GET /api/v1/document-templates/{kind}/legacy` — 200 или 404;
  - в интерфейсе `legacyTemplate(kind): Promise<LegacyTemplate | null>`.

Соответствие файлов и видов:

| Файл | Вид |
|---|---|
| `Shablon_Act_Priema.htm` | `intake_act` |
| `Shablon_Act_vidachi.htm` | `issue_act` |
| `Shablon_act_priemaGarantii.htm` | `warranty_intake_act` |
| `Shablon_act_vidachiGarantii.htm` | `warranty_issue_act` |

Метки MyWork 2 заменяются целыми словами, длинные раньше коротких. Подстановка — `{<русское имя поля>}` из `mw-docs` `fields`.

| Метка | Поле |
|---|---|
| `NUMZAKAZPRINT` | `order.number` |
| `DATAPRIEMAPRINT` | `order.received` |
| `DATAVIDACHIPRINT` | `order.issued` |
| `FIOPRINT` | `client.name` |
| `PHONEPRINT` | `client.phone` |
| `ADRESSCLIENTAIFNOTNULLPRINT` | `client.address` |
| `TYPEUSTRPRINT` | `order.device` |
| `SERIALNPIRNT` | `order.serial` |
| `KOMPLEKTNOSTPRINT` | `order.completeness` |
| `VNESHVIPRINT` | `order.condition` |
| `NEISPRAVNOSTPRINT` | `order.malfunction` |
| `VIPRABOTPRINT` | `order.work_done` |
| `PREDOPLATAPRINT` | `order.prepayment` |
| `PREDSTOIMOSTPRINT` | `order.estimate` |
| `GARANTYPRINT` | `order.warranty` |
| `FIRMNAMEPRINT` | `firm.name` |
| `FIRMTELPRINT` | `firm.phone` |
| `DANNIEOFIRMEPRINT` | `firm.details` |
| `URDANNIEPRINT` | `firm.legal` |
| `DOGOVORPRIEMPRINT` | `firm.intake_terms` |
| `DOGOVORVIDACHAPRINT` | `firm.issue_terms` |
| `PRICEFULLPRINT` | две строки: «Сумма предоплаты: {предоплата}» и «К оплате: {к оплате}» |
| `SKIDKAPRINT` | «Скидка: {скидка}» |

Остальные слова, похожие на метку, остаются как есть.

- [ ] **Step 1: Написать падающие тесты.**
  - Фикстура `mw_legacy::fixture` получает синтетический `Shablon_Act_Priema.htm` (cp1251, HTML как из Word) с `<o:p>`, метками `NUMZAKAZPRINT`, `FIOPRINT`, `PRICEFULLPRINT` и своей фразой «Гарантия на чистку — 14 дней». Тест импорта: строка `legacy_rows` с этим файлом есть.
  - `crates/mw-core/tests/legacy_templates.rs` (по `all_backends()`, база из фикстуры):
    ```rust
    #[tokio::test] async fn legacy_intake_template_is_text_with_fields() // текст содержит «{заказ.номер}», «{клиент.фио}», «Гарантия на чистку — 14 дней»,
                                                                       // нет «NUMZAKAZPRINT», нет «<», нет «o:p»
    #[tokio::test] async fn price_and_discount_expand()                // PRICEFULLPRINT → две строки с {заказ.предоплата} и {заказ.к_оплате}
    #[tokio::test] async fn no_legacy_template_is_none()               // вид без файла (sticker, issue_act в фикстуре без файла) → None
    ```
  - API: GET `…/intake_act/legacy` → 200, `…/sticker/legacy` → 404.
  - `LegacyTemplateDialog.test.ts`: «Вставить текстом» зовёт `oninsert` с текстом; «Закрыть» — ничего.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-legacy && cargo test -p mw-core --test legacy_templates`. Expected: FAIL.
- [ ] **Step 3: Реализовать** чтение из `legacy_rows`, замену меток, маршрут (контракт перегенерировать), меню «Шаблон из MyWork 2…» в файловом меню редактора (только если `legacyTemplate` вернул текст). Диалог показывает текст с переносами строк, только для чтения, и кнопку «Вставить текстом»: она добавляет блок «Текст» после выбранного (или в конец) одним шагом отмены.
- [ ] **Step 4: Прогнать.** Run: `cargo test --workspace && cd apps/ui && npm run verify`. Expected: PASS.
- [ ] **Step 5: Commit** — `feat: текст шаблонов MyWork 2 в редакторе`.

### Task 9: Доводка редактора шаблонов

**Files:**
- Modify: `apps/ui/src/lib/templates/{editor.svelte.ts,ops.ts,keys.ts,preview-queue.ts,order-search.svelte.ts,BlockList.svelte,TemplateEditor.svelte}`, `apps/ui/src/lib/templates/props/{FieldsProps.svelte,PageProps.svelte}`, `apps/ui/src/routes/(app)/orders/[id]/+page.svelte` (стоп-лист ухода), `apps/ui/src/lib/i18n/ru.ts`
- Create: общий помощник ухода со страницы, например `apps/ui/src/lib/navigation/leave-guard.svelte.ts`
- Test: соответствующие `*.test.ts`

Что сделать (заметки «План 7.2» в README и итоговое ревью 7.1):
1. **Гонка «отмена после 409»** — решение 9. Тест в `editor.test.ts`:
   1. правка;
   2. сохранение отклоняется 409;
   3. отмена, пока идёт `load`;
   4. `load` возвращает более новую модель.

   Ожидается: `saveTemplate` больше не зовётся, показана новая модель.
2. **Поля наклейки** — решение 8. `PageProps` у вида `sticker` берёт размер наклейки из настроек печати рабочего места и, если область содержимого меньше минимума, показывает предупреждение «На наклейке этого рабочего места (W × H мм) поля не оставляют места для содержимого». Тест компонента.
3. **Помощник ухода со страницы.** Стоп-лист ухода со страницы заказа и из редактора — один помощник; оба экрана берут его.
4. **Форматирование миллиметров** — одно, `formatMm` (убрать `ru.templates.mm` или свести к нему).
5. **Таблица полей:**
   - значение строки — многострочное поле;
   - при 50 строках кнопка «Добавить строку» недоступна с подсказкой «Не больше 50 строк»;
   - `flush()`, вернувший `false`, показывает уведомление «Не сохранено» с повтором.
6. **Ключи строк** списка блоков едут за блоком при перестановке и в списке той же длины (тест в `keys.test.ts`).
7. **Операции и очередь превью:**
   - `reorder` с повторяющимися индексами — без изменений;
   - `newBlock` не создаёт `calibration_marks`;
   - если превью бросает исключение сразу, очередь не остаётся «занятой»;
   - тесты сброса очереди, отказа устаревшего запуска и границы `mergeMs`.
8. **Без записи состояния внутри `$derived`** в `FieldsProps` и `of()`: перевести на `$state` + явное обновление.
9. **Поиск заказа для примера** не копит повторы в `known`.

- [ ] **Step 1: Написать падающие тесты** — по пункту на каждый тест выше (vitest).
- [ ] **Step 2: Запустить.** Run: `cd apps/ui && npx vitest run src/lib/templates`. Expected: FAIL на новых тестах.
- [ ] **Step 3: Реализовать** пункты 1–9.
- [ ] **Step 4: Прогнать.** Run: `cd apps/ui && npm run verify && npx playwright test e2e/template-editor.spec.ts e2e/orders.spec.ts e2e/layout.spec.ts`. Expected: PASS.
- [ ] **Step 5: Commit** — `fix(ui): доводка редактора шаблонов по итогам плана 7.1`.

### Task 10: Доводка ядра, API и сквозных тестов печати

**Files:**
- Modify: `crates/mw-docs/src/typst_gen/blocks.rs`, `crates/mw-core/src/document_templates.rs`, `crates/mw-api/src/routes/{documents,document_templates}.rs`, `crates/mw-api/tests/document_templates.rs`, `apps/ui/e2e/{print.spec.ts,template-editor.spec.ts}`, `apps/ui/src/lib/print/print.svelte.ts`, `apps/ui/src/routes/(app)/settings/print/+page.svelte`, `apps/ui/src/lib/intake/IntakeSheet.svelte` (тесты)
- Test: те же файлы и их тесты

Что сделать:
1. **Цифры растянутого штрихкода** — не крупнее 14 pt (тест в `layout_tests.rs`: акт A4 с растянутым штрихкодом на всю ширину → кегль цифр ≤ 14 pt).
2. **`warn!` о повреждённом шаблоне** — один раз на вид и его `version` за время работы процесса. Помощник `fn first_warning(kind, version) -> bool` над `Mutex<HashSet<…>>` и модульный тест: первый вызов `true`, второй с тем же `(kind, version)` `false`, с новой `version` снова `true`.
3. **Тесты API шаблонов:**
   - отказ по правам важнее ошибки проверки: PUT неверной модели без права → 403, а не 422;
   - 400 на неизвестный вид;
   - несовпадение вида в превью;
   - 403 на PUT и на загрузку файла без права;
   - превью с `order_id`;
   - превью черновика, отличного от стандартного (число страниц или размер отличаются).
4. **Код заголовков** вложения и `Cache-Control` в маршрутах документов и шаблонов — один помощник.
5. **Сквозные тесты печати и редактора не делят виды документов.** `print.spec` перестаёт трогать гарантийные акты, которые правит `template-editor.spec`, или спеки идут по разным видам — так, чтобы параллельный запуск не мешал.
6. **`choose()` в `template-editor.spec`.** Найти причину сдвига свойств под щелчком и убрать повтор. Если за час работы причина не найдена — повтор оставить, найденное записать в отчёт.
7. **Подсказка о 300 dpi** на Windows не показывает слов про PDF, пока формат печати неизвестен: до ответа — нейтральный текст. Тест страницы «Печать» для `png`.
8. **`IntakeSheet`:**
   - тест позднего ответа принтера (форма открыта — печать без предпросмотра);
   - тест, что неудачный список принтеров не пишет «Принтеры не найдены».

- [ ] **Step 1: Написать падающие тесты** по пунктам 1–3, 7, 8.
- [ ] **Step 2: Запустить.** Run: `cargo test -p mw-docs && cargo test -p mw-api --test document_templates && cd apps/ui && npx vitest run src/lib/print src/lib/intake`. Expected: FAIL на новых тестах.
- [ ] **Step 3: Реализовать** пункты 1–8.
- [ ] **Step 4: Прогнать.** Run: `cargo test --workspace && cd apps/ui && npm run verify && npm run test:e2e`. Expected: PASS; `template-editor.spec` трижды подряд в Chromium без повтора щелчка (если п. 6 сделан).
- [ ] **Step 5: Commit** — `fix: доводка печати и тестов по итогам плана 7.1`.

### Task 11: Документация

**Files:**
- Modify: `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` (§5.1, §5.2), `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (раздел 7), `docs/superpowers/plans/README.md`, `AGENTS.md`

- [ ] **Step 1: Спека §5.1** — подраздел «Как сделано в плане 7.2»:
  - Code 128 (правила решения 2);
  - запасные шрифты и папка `fonts`;
  - логотип и блок `Logo`;
  - `TEMPLATE_FILE_VERSION` 2;
  - пачка документов и её пределы;
  - текст старых шаблонов;
  - почему `[ACTS] BarcodeH/BarcodeW` не переносятся.

  Строку «Остаётся плану 7.2» заменить на «Остаётся плану 7.3: свой Typst-код с пределом времени компиляции (отдельный процесс рендера)». §5.2 — массовая печать.
- [ ] **Step 2: Спека интерфейса, раздел 7** — «Печать ▾» в панели выбора списка заказов, раздел «Логотип» в «Документы и реквизиты», «Шаблон из MyWork 2…» в редакторе.
- [ ] **Step 3: README:**
  - план 7.2 — «выполнен»;
  - новая строка 7.3 «Свой Typst-код» (отдельный процесс рендера с пределом времени, блок «свой Typst-код», редактор кода);
  - заметки «План 7.2» — убрать сделанное, остаток перенести в «План 7.3» (свой Typst-код) или в доводку, если что-то осталось.
- [ ] **Step 4: `AGENTS.md`:**
  - правило про документы: «только `data.json` и виртуальные `barcode.svg` и `logo.png`»;
  - новое правило: «Логотип фирмы — только через `Core::set_firm_logo` (вложение вида `logo`); в документ он попадает виртуальным `logo.png`».
- [ ] **Step 5: Commit** — `docs: план 7.2 выполнен, план 7.3 «Свой Typst-код»`.
