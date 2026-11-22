# План 6. Документы и печать — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** из заказа печатаются акт приёма, акт выдачи, гарантийные акты, пустой акт приёма и наклейка со штрихкодом —
сразу на принтер этого компьютера, в предпросмотр и в PDF; реквизиты и тексты договоров правятся в настройках.

**Architecture:** новый крейт `mw-docs` (без ввода-вывода, зависит только от `mw-domain`) хранит модель шаблона,
стандартные шаблоны, реестр полей подстановки и превращает шаблон в разметку Typst; данные заказа приходят в Typst
отдельным JSON-файлом и в разметку не попадают. `mw-core` собирает данные документа из заказа, клиента и настроек и
рендерит в фоне; `mw-api` отдаёт PDF или страницы PNG. Программа для компьютера получает документ от сервера и
печатает локально: на macOS и Linux — PDF через CUPS (`lp`), на Windows — страницы PNG в DPI принтера через GDI.
Модель шаблона хранится в коде (стандартные шаблоны); таблица шаблонов и редактор — план 7.

**Tech Stack:** Rust (Typst 0.15.1: `typst`, `typst-pdf`, `typst-render`, `typst-assets`; Axum 0.8; SeaORM 2),
Tauri 2 (крейт `windows` для GDI), SvelteKit 3 + Svelte 5, Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` — раздел 5 п. 6 «Документы», 5.1
«Редактор актов и наклеек» (модель шаблона, стандартные шаблоны), 5.2 «Печать»;
`docs/superpowers/specs/2026-11-15-ui-design-system-design.md` — раздел 7 «Печать (план 6)», раздел 8 (Ctrl/⌘P), 6.4
(переключатели «Акт приёма», «Наклейка», «Печатать акт выдачи»); заметки «План 6» в `docs/superpowers/plans/README.md`.

## Global Constraints

- Всё из `AGENTS.md`: `mw-domain` без ввода-вывода, права проверяет `mw-core`, `all_backends()`, контракт API
  перегенерируется (`MW_UPDATE_CONTRACT=1 …`), входные типы с `deny_unknown_fields`, `#[ts(export)]` не ставить; интерфейс —
  токены, `display`/`data-width`, руны, `#lib/...`, тексты в `ru.ts`, `fetch` только в `src/lib/api/client.ts`,
  `registerHotkey`, `createSettingsEditor`, подсказки `HelpHint`/проп `help`, e2e в Chromium и WebKit на настоящем `mw-server`.
- Лицензии: Typst и его зависимости — MIT/Apache/BSD/ISC/Zlib/Unicode (проверено 2026-11-22: 300 пакетов, исключений нет).
  Шрифт документов — **только Libertinus Serif** (OFL-1.1, файлы из `typst-assets` 0.15.1): DejaVu (лицензия Bitstream
  Vera) и New Computer Modern (GUST) не берём — фича `fonts` у `typst-assets` выключена, нужные `.otf` лежат в
  `crates/mw-docs/fonts/` вместе с текстом OFL из `NOTICE`.
- В Libertinus нет знака `₽`: в документах сумма пишется «1 500 руб.», если знака валюты нет в шрифте (таблица замен
  `₽ → руб.`, `₸ → тг`, `₴ → грн`; проверяется тестом покрытия, а не вручную).
- Данные пользователя (поля заказа, клиента, реквизиты, тексты договоров) **никогда не становятся разметкой Typst**:
  они читаются из `data.json` внутри документа; текст шаблона вставляется как строковый литерал Typst с экранированием.
- Печать всегда на том компьютере, где нажали кнопку; настройки печати (принтеры, размер наклейки, смещение, DPI,
  копии, переключатели) хранятся на устройстве (`#lib/design/storage.ts`), не на сервере.
- Принтер и файлы передаются в `lp` и `lpstat` отдельными аргументами (`std::process::Command`, без оболочки), с
  `LANG=C`/`LC_ALL=C` для разбора вывода.
- Рендер — в `tokio::task::spawn_blocking`, не больше двух одновременно (семафор в `Core`), после каждого рендера
  `comemo::evict(30)`.
- Рабочая ветка `plan-6-documents-printing`, worktree `.superpowers/worktrees/plan-6`; `CARGO_TARGET_DIR=/Users/ruf/dev/MyWork2/target`.
- Каждый закрытый пункт удаляется из «Заметки → План 6» в README в той же задаче.
- Коммиты по-русски, последняя строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Специальные символы Typst в данных**: неисправность или ФИО с `#set page(width: 1mm)`, `$x$`, `*жирно*`, `_`,
   `[`, `]`, `\`, `"`, `<a>`, `@ref`, `// комментарий`, `/*` печатаются как есть и не ломают документ — тест в Task 3.
2. **Очень длинный текст в «2 на лист»** (неисправность в 40 строк, договор на страницу): экземпляры не налезают друг
   на друга; если экземпляр не помещается в половину A4, каждый печатается на своём листе — тест в Task 3.
3. **Пустые поля** (нет адреса, оценки, мастера, телефона, серийного номера): «Не указан» / «Не оговорена» как в
   MyWork 2 или строка скрыта по шаблону; в документе не остаётся `{заказ.…}` — тест в Task 4.
4. **HTML из MyWork 2 в реквизитах и договорах** (`<br>`, `<b>`, `&nbsp;`, `&quot;`, `&#8470;`): печатается
   простым текстом с переносами строк, теги не видны — тест в Task 5.
5. **Принтер пропал или `lp` вернул ошибку, принтер не выбран, имя принтера с пробелами и кириллицей**: понятное
   уведомление с текстом ошибки, без молчания; имя передаётся одним аргументом — тесты в Task 7 (разбор и сборка
   аргументов) и Task 9 (уведомление).

---

### Task 1: Крейт `mw-docs` — Typst внутри процесса, шрифты, PDF и PNG

**Files:** `Cargo.toml` (workspace: `typst`, `typst-pdf`, `typst-render`, `typst-assets` 0.15.1 без фичи `fonts`),
новый `crates/mw-docs/{Cargo.toml,src/lib.rs,src/world.rs,src/render.rs,src/fonts.rs}`, `crates/mw-docs/fonts/`
(`LibertinusSerif-{Regular,Italic,Semibold,Bold}.otf`, `OFL.txt`), `deny.toml`, `.github/workflows/ci.yml` (задание
`cargo deny check licenses`, установка через `taiki-e/install-action`), `apps/desktop/src-tauri/Cargo.lock`
(обновится в Task 6, здесь не трогать).

**Interfaces:**
- Produces: `mw_docs::Rendered` — `pub fn page_count(&self) -> usize`, `pub fn page_size_mm(&self) -> (f64, f64)`
  (первая страница), `pub fn pdf(&self) -> Result<Vec<u8>, DocsError>`, `pub fn png_pages(&self, dpi: u32) ->
  Result<Vec<Vec<u8>>, DocsError>`, `pub fn text(&self) -> String` (весь текст по страницам, для тестов и проверок).
- Produces: `pub(crate) fn compile(source: String, files: Vec<(String, Vec<u8>)>) -> Result<Rendered, DocsError>` —
  `World` с одним главным файлом, виртуальными файлами (`data.json`, `barcode.svg`), шрифтами из `fonts.rs`
  (`include_bytes!`), `today() = None`; ошибки Typst собираются в `DocsError::Compile(Vec<String>)`.
- Produces: `pub fn font_covers(text: &str) -> Vec<char>` — знаки, которых нет ни в одном начертании Libertinus.

Пункты (тесты в `crates/mw-docs/tests/render.rs`):
1. Документ «Привет, №1 «тест» — ёлка» компилируется, `pdf()` начинается с `%PDF-`, `text()` содержит строку целиком.
2. Страница `#set page(width: 58mm, height: 40mm)`: `page_size_mm()` ≈ (58, 40) с точностью 0,01; `png_pages(203)` даёт
   PNG 463×319 (±1) пикселей.
3. Ошибка в разметке → `DocsError::Compile` с текстом ошибки, без паники.
4. `font_covers("АБВ…Яа…яЁё№«»—–…·×°")` пуст, `font_covers("₽")` == `['₽']`.
5. Сто рендеров подряд не наращивают память без предела: после каждого `comemo::evict(30)` (проверка — что вызов есть
   в `render.rs`, отдельного теста памяти не нужно).

- [ ] Step 1: тесты → падают. Step 2: крейт, шрифты, `World`, рендер. Step 3: `cargo deny check licenses` локально
  (если `cargo-deny` нет — один раз `cargo install cargo-deny --locked`) и полные проверки Rust; закрыть в README
  «Общее → добавить в CI `cargo-deny`».
- [ ] Step 4: Commit `feat(docs): крейт mw-docs — Typst в процессе, Libertinus, PDF и PNG; cargo-deny в CI`.

---

### Task 2: Штрихкод UPC-A в SVG

**Files:** `crates/mw-docs/src/barcode.rs`, тесты там же.

**Interfaces:**
- Consumes: `mw_domain::barcode::is_valid_upc_a`.
- Produces: `pub fn upc_a_svg(code: &str, module_mm: f64, height_mm: f64) -> Option<String>` — только полосы
  (95 модулей, охранные полосы длиннее на 5 модулей, тихая зона 9 модулей с каждой стороны), цифры печатает Typst;
  `shape-rendering="crispEdges"`; `None` для неверного кода.
- Produces: `pub fn module_mm(dpi: Option<u32>) -> f64` — без DPI 0,33 мм; с DPI — целое число точек
  `ceil(0,33 мм × dpi / 25,4)`, переведённое обратно в мм (203 dpi → 3 точки → 0,3754 мм).

Пункты:
1. `036000291452` даёт известную последовательность модулей (таблица L/R-кодов UPC-A; ожидаемая строка из 95 нулей и
   единиц — в тесте).
2. Ширина SVG = (95 + 18) × `module_mm`; неверная контрольная цифра и 11 цифр → `None`.
3. `module_mm(Some(203))`, `module_mm(Some(300))` (5 точек), `module_mm(Some(600))` (8 точек), `module_mm(None)`.

- [ ] Step 1: тесты → падают. Step 2: реализация. Step 3: `cargo test -p mw-docs`.
- [ ] Step 4: Commit `feat(docs): штрихкод UPC-A в SVG с шагом под DPI принтера`.

---

### Task 3: Модель шаблона, реестр полей и генератор Typst

**Files:** `crates/mw-docs/src/{model.rs,fields.rs,typst_gen.rs}`, `crates/mw-domain/src/documents.rs` (+ `lib.rs`),
тесты `crates/mw-docs/tests/generator.rs`.

**Interfaces:**
- Produces (`mw-domain`, типы API с `Serialize`/`Deserialize`/`TS`/`ToSchema`, `snake_case`):
  `DocumentKind { IntakeAct, IssueAct, WarrantyIntakeAct, WarrantyIssueAct, BlankIntakeAct, Sticker, Calibration }`;
  `DocumentKind::for_order(self) -> bool` (все, кроме `BlankIntakeAct` и `Calibration`).
- Produces (`mw-docs::model`, `Serialize`/`Deserialize` — это формат `.mwtemplate` плана 7):
  `Template { kind: DocumentKind, page: PageSetup, layout: Layout, font_size_pt: f32, blocks: Vec<Block> }`;
  `PageSetup { size: PageSize, landscape: bool, margins_mm: Margins }`; `PageSize { A4, A5, Custom { width_mm: f32,
  height_mm: f32 } }`; `Layout { OnePerPage, TwoPerA4 }`;
  `Block { copies: CopyFilter, body: BlockBody }`, `CopyFilter { Both, Service, Client }`;
  `BlockBody::{ Text { text: String, size_pt: Option<f32>, bold: bool, align: Align }, Fields { rows: Vec<FieldRow> },
  Barcode { height_mm: f32, digits: bool, align: Align }, Signatures { left: String, right: String }, Separator,
  Spacer { mm: f32 }, Columns { columns: Vec<Vec<Block>>, gap_mm: f32 }, CopyLabel }`;
  `FieldRow { label: String, value: String, hide_if_empty: bool }` (`value` — текст с подстановками).
- Produces (`mw-docs::fields`): `FieldDef { key: &'static str, placeholder: &'static str, label: &'static str,
  sample: &'static str, empty: &'static str }` и `pub fn fields() -> &'static [FieldDef]`; поиск подстановки — без
  учёта регистра, «ё» = «е». Набор (ключ → `{подстановка}` → пусто):
  `order.number` → `заказ.номер`; `order.barcode` → `заказ.штрихкод`; `order.received` → `заказ.дата_приёма`;
  `order.issued` → `заказ.дата_выдачи`; `order.device` → `заказ.устройство` (тип, бренд и модель через пробел);
  `order.device_type`, `order.brand`, `order.model`, `order.serial` → `заказ.серийный_номер` («Не указан»),
  `order.color`, `order.completeness` → `заказ.комплектность`, `order.condition` → `заказ.состояние`,
  `order.malfunction` → `заказ.неисправность`, `order.work_done` → `заказ.работы`, `order.master` → `заказ.мастер`,
  `order.estimate` → `заказ.оценка` («Не оговорена»), `order.prepayment` → `заказ.предоплата`, `order.discount` →
  `заказ.скидка`, `order.price` → `заказ.стоимость`, `order.due` → `заказ.к_оплате`, `order.warranty` →
  `заказ.гарантия`; `client.name` → `клиент.фио`, `client.phone` → `клиент.телефон`, `client.phones` →
  `клиент.телефоны`, `client.address` → `клиент.адрес` («Не указан»); `firm.name` → `фирма.название`, `firm.phone`,
  `firm.details` → `фирма.сведения`, `firm.legal` → `фирма.реквизиты`, `firm.intake_terms` → `фирма.договор_приёма`,
  `firm.issue_terms` → `фирма.договор_выдачи`; `point.name` → `точка.название`; `doc.date` → `документ.дата`.
- Produces (`mw-docs`): `DocumentData { values: BTreeMap<String, String>, barcode: Option<String>, blank: bool }`
  (ключи — `key` из реестра); `RenderOptions { label_mm: Option<(f32, f32)>, offset_mm: (f32, f32), dpi: Option<u32> }`;
  `pub fn to_typst(template: &Template, opts: &RenderOptions) -> String`;
  `pub fn render(template: &Template, data: &DocumentData, opts: &RenderOptions) -> Result<Rendered, DocsError>`.

Генератор (решения, которые реализация не выбирает сама):
- Текст шаблона режется на литералы и `{подстановки}`; литерал — строковый литерал Typst (`\` и `"` экранируются),
  подстановка — вызов `v("order.number")`, который читает `data.json` и превращает переводы строк в `linebreak()`.
  Неизвестная подстановка печатается как написана, с фигурными скобками.
- Пустое значение: если `blank` — линия для записи от руки; иначе текст `empty` из реестра, а при его отсутствии —
  пусто; строка `FieldRow` с `hide_if_empty` и пустым значением не выводится.
- `TwoPerA4`: две половины A4 (сервиса сверху, клиента снизу), между ними пунктир «линия отреза»; `CopyLabel` пишет
  «Экземпляр сервиса» / «Экземпляр клиента». Экземпляр измеряется (`measure` в `context`); если он выше половины
  листа минус отступ, каждый экземпляр печатается на своём листе A4.
- `label_mm` заменяет размер страницы шаблона (наклейка, калибровка); `offset_mm` сдвигает содержимое страницы.

Пункты:
1. Данные с `#set page(width: 1mm)`, `$x$`, `*жирно*`, `_a_`, `[`, `]`, `\`, `"`, `<a>`, `@ref`, `// к`, `/*` —
   в `text()` ровно эти строки, страница прежнего размера. То же — в литерале текста шаблона.
2. `{заказ.номер}`, `{ЗАКАЗ.НОМЕР}`, `{заказ.дата_приема}` подставляются; `{заказ.нет_такого}` остаётся как есть.
3. Многострочная неисправность — строки в `text()` разделены переводом строки.
4. `TwoPerA4` с коротким содержимым — 1 страница A4, «Экземпляр сервиса» и «Экземпляр клиента» по разу; блок с
   `CopyFilter::Service` — только один раз. С неисправностью в 40 строк — 2 страницы.
5. `hide_if_empty`, `empty`, `blank` — по тесту на каждый.
6. `label_mm: Some((40.0, 30.0))` — страница 40×30 мм.

- [ ] Step 1: тесты → падают. Step 2: модель, реестр, генератор. Step 3: `cargo test -p mw-docs -p mw-domain`, clippy.
- [ ] Step 4: Commit `feat(docs): модель шаблона, поля подстановки и генератор Typst`.

---

### Task 4: Стандартные шаблоны

**Files:** `crates/mw-docs/src/standard.rs`, тесты `crates/mw-docs/tests/standard.rs`.

**Interfaces:**
- Produces: `pub fn standard_template(kind: DocumentKind) -> Template`; `pub fn sample_data(kind: DocumentKind) ->
  DocumentData` (из `FieldDef::sample`; для `BlankIntakeAct` — `blank: true`, для `Calibration` — пустые значения).

Содержимое (шрифт Libertinus Serif, основной текст 9 pt, договор 7,5 pt):
- **Акт приёма** (`TwoPerA4`, поля 10 мм): шапка в две колонки — слева `{фирма.название}` жирно 13 pt, телефон,
  `{фирма.сведения}`; справа штрихкод 12 мм с цифрами и `CopyLabel`. Заголовок «Акт приёма № {заказ.номер} от
  {заказ.дата_приёма}». Таблица: Клиент, Телефон, Адрес, Устройство, Серийный номер, Цвет (скрывать пустое),
  Комплектность, Внешний вид, Неисправность, Предварительная стоимость, Предоплата (скрывать пустое). Затем
  `{фирма.договор_приёма}` 7,5 pt, подписи «Сдал» / «Принял», внизу `{фирма.реквизиты}` 7 pt.
- **Акт выдачи**: та же шапка, «Акт выдачи № … от {заказ.дата_выдачи}»; таблица: Клиент, Телефон, Устройство,
  Серийный номер, Неисправность, Выполненные работы, Мастер (скрывать пустое), Гарантия, Стоимость, Скидка
  (скрывать пустое), Предоплата (скрывать пустое), К оплате; `{фирма.договор_выдачи}`; подписи «Получил» / «Выдал».
- **Гарантийные**: как приём и выдача, заголовки «Акт приёма по гарантии № …» и «Акт выдачи после гарантийного
  ремонта № …», строка «Гарантия» в обоих.
- **Пустой акт приёма**: акт приёма с `blank` (линии вместо значений, без номера и штрихкода).
- **Наклейка** (`OnePerPage`, по умолчанию 58×40 мм, поля 1,5 мм, Semibold): штрихкод на всю ширину (высота ~45 %
  наклейки) с цифрами, «№ {заказ.номер} · {заказ.дата_приёма}», `{клиент.фио}`, `{клиент.телефон}`,
  `{заказ.устройство}` — каждая строка в одну строку с обрезкой (как в MyWork 2: код, ФИО, телефон, номер).
- **Калибровка**: рамка на 0,3 мм внутри края, перекрестие по центру, подпись «Ш × В мм · N dpi · смещение X/Y».

Пункты (каждый шаблон на `sample_data`):
1. Компилируется; число страниц: акты — 1, наклейка 58×40 и 40×30 — 1 соответствующего размера.
2. В `text()` нет `{` подстановок; есть номер, ФИО и заголовок своего вида.
3. `font_covers(text())` пуст для всех шаблонов.
4. Пустой акт: нет цифр номера заказа и штрихкода, есть «Акт приёма».

- [ ] Step 1: тесты → падают. Step 2: шаблоны. Step 3: `cargo test -p mw-docs`; PNG каждого шаблона при 110 dpi
  сохранить в `target/docs-preview/` и посмотреть глазами (исправить налезания, мелкий шрифт, пустые места).
- [ ] Step 4: Commit `feat(docs): стандартные акты «2 на листе», наклейка и калибровка`.

---

### Task 5: Ядро — данные документа и рендер

**Files:** `crates/mw-core/Cargo.toml` (+ `mw-docs`), новый `crates/mw-core/src/documents.rs` (+ `lib.rs`),
`crates/mw-domain/src/phone.rs` и `money.rs` (форматирование), тесты `crates/mw-core/tests/documents.rs`.

**Interfaces:**
- Consumes: `mw_docs::{standard_template, sample_data, render, RenderOptions, DocumentData}`, `Core::get_order`.
- Produces: `mw_domain::phone::format_phone(digits: &str) -> String` — то же, что `formatPhone` в
  `apps/ui/src/lib/format/phone.ts` (общий набор примеров в тестах обеих сторон); `mw_domain::money::format_money_doc(m:
  Money, symbol: &str) -> String` — как `formatMoney` в UI, с заменой знака, которого нет в шрифте (Global Constraints).
- Produces: `pub fn plain_text(html_or_text: &str) -> String` (`mw-core::documents`): `<br>`, `<br/>`, `</p>`, `</div>`,
  `</li>` → перевод строки; остальные теги убираются; `&nbsp; &amp; &lt; &gt; &quot; &#39; &#NNN; &#xHH;` раскрываются;
  больше двух переводов строк подряд → два; края обрезаются. Применяется при печати, сохранённые настройки не меняются.
- Produces: `DocumentRequest { kind: DocumentKind, order_id: Option<i64>, sample: bool, options: RenderOptions }`;
  `Core::render_document(&self, actor: &Actor, req: DocumentRequest) -> Result<mw_docs::Rendered, CoreError>`:
  заказ — те же права, что у `get_order`; для видов `for_order()` нужен `order_id` или `sample = true` (образец на
  `sample_data`), иначе `CoreError::Validation`; `BlankIntakeAct` и `Calibration` — всегда без заказа; рендер в
  `spawn_blocking` под семафором на 2.
- Даты — местные, «22 ноября 2026» (родительный падеж месяцев); `order.due` = как «К оплате» в `OrderFields.svelte`
  (повторить ту же формулу, сослаться на неё комментарием); `order.estimate` — «от X до Y», «X» или пусто;
  `client.phone` — первый номер, `client.phones` — все через запятую; мастер — имя текстом из заказа.

Пункты (`all_backends()`):
1. Акт приёма настоящего заказа: в `text()` номер, ФИО, отформатированный телефон, «Не указан» для пустого адреса,
   «Не оговорена» для пустой оценки, сумма предоплаты «1 500 руб.» при знаке «₽».
2. `plain_text`: таблица примеров из Review Focus п. 4 и обычный текст без изменений.
3. Реквизиты с `<br>` в настройках → в документе строки разделены, `<br>` нет.
4. Акт выдачи невыданного заказа печатается: дата выдачи пустая.
5. `IssueAct` без `order_id` и без `sample` → `Validation`; несуществующий заказ → `NotFound`.
6. `format_phone` и `format_money_doc` — общие примеры с UI.

- [ ] Step 1: тесты → падают. Step 2: реализация. Step 3: полные проверки Rust на обеих СУБД.
- [ ] Step 4: Commit `feat(core): данные документа из заказа и настроек, рендер в фоне`.

---

### Task 6: API документов

**Files:** `crates/mw-api/src/routes/documents.rs` (+ `routes/mod.rs`), `crates/mw-api/src/dto.rs`, `contract.rs`,
контракт (`openapi.json`, `bindings/`), тесты `crates/mw-api/tests/documents.rs`; `apps/desktop/src-tauri/Cargo.lock`
(`cargo check` в `apps/desktop/src-tauri` после новых зависимостей `crates/*`).

**Interfaces:**
- Produces: `GET /api/v1/orders/{id}/documents/{kind}` и `GET /api/v1/documents/{kind}` (без заказа: пустой акт,
  калибровка, остальные виды — образец на `sample_data`); запрос `DocumentQuery { format: DocumentFormat (pdf | png,
  по умолчанию pdf), dpi: Option<u32> (72..=600; для `png` по умолчанию 110; передаётся и с `pdf` — тогда шаг
  штрихкода подгоняется под DPI принтера, без него 0,33 мм), label_w_mm, label_h_mm: Option<f32> (15..=200, только
  парой), offset_x_mm, offset_y_mm: Option<f32> (−10..=10) }` с `deny_unknown_fields`.
- Produces: ответ `pdf` — `application/pdf`, `Content-Disposition: inline; filename="<вид>-<номер>.pdf"` (имена:
  `akt-priema`, `akt-vydachi`, `akt-priema-garantiya`, `akt-vydachi-garantiya`, `akt-priema-pustoj`, `naklejka`,
  `kalibrovka`); ответ `png` — JSON `DocumentPages { dpi: u32, width_mm: f32, height_mm: f32, pages: Vec<String> }`
  (PNG в base64).
- Ошибки — обычные ответы API (400 для неверных параметров с текстом по-русски, 404 для заказа).

Пункты (`all_backends()`):
1. PDF акта приёма: 200, `application/pdf`, тело с `%PDF-`, имя файла с номером заказа.
2. PNG наклейки 58×40 при 203 dpi: одна страница, размеры в ответе.
3. `dpi=1000`, `label_w_mm` без `label_h_mm`, лишний параметр → 400.
4. `GET /documents/issue_act` → образец; `GET /documents/calibration?label_w_mm=40&label_h_mm=30` → 40×30.
5. Без входа (когда пользователи есть) → 401, как у заказов.

- [ ] Step 1: тесты → падают. Step 2: маршруты, контракт (`MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract
  -- --test-threads=1`). Step 3: полные проверки Rust; `cargo check` и `npm run licenses` в `apps/desktop`.
- [ ] Step 4: Commit `feat(api): документы заказа в PDF и PNG`.

---

### Task 7: Программа для компьютера — принтеры и печать

**Files:** `apps/desktop/src-tauri/src/print/{mod.rs,cups.rs,windows.rs}`, `src/lib.rs` (`invoke_handler`),
`build.rs` (`tauri_build::Attributes` с `AppManifest::commands`), `capabilities/default.json` и тест-закрепка списка
разрешений в `lib.rs`, `Cargo.toml` (`windows` с фичами `Win32_Graphics_Gdi`, `Win32_Graphics_Printing`,
`Win32_Storage_Xps`, `Win32_Foundation` — только `cfg(windows)`; `image` для PNG).

**Interfaces:**
- Produces (команды Tauri): `list_printers() -> Result<Vec<PrinterInfo { name: String, is_default: bool }>, String>`;
  `print_format() -> &'static str` (`"png"` на Windows, иначе `"pdf"`); `print_pdf(printer: String, pdf_base64: String,
  copies: u32, media_mm: Option<(f32, f32)>) -> Result<(), String>` (macOS/Linux); `print_pages(printer: String,
  pages_base64: Vec<String>, dpi: u32, width_mm: f32, height_mm: f32, copies: u32) -> Result<(), String>` (Windows).
  Ошибки — строки по-русски для уведомления («Принтер «X» не найден», «Ошибка печати: …» с выводом `lp`).
- CUPS: `lpstat -e` (имена), `lpstat -d` (по умолчанию), `LC_ALL=C`; `lp -d <имя> -n <копии> [-o media=Custom.<Ш>x<В>mm]
  <файл>`; PDF — во временный файл с уникальным именем, удаляется после выхода `lp`. Чистые функции
  `parse_lpstat_e`, `parse_lpstat_d`, `lp_args(printer, copies, media, path) -> Vec<OsString>` — под тестами.
- Windows: `EnumPrintersW` (локальные и подключения, уровень 4), `GetDefaultPrinterW`; печать — `CreateDCW`,
  `StartDocW`, на каждую страницу и копию `StartPage` → PNG в 24-битный DIB → `StretchDIBits` в физический размер
  страницы по `LOGPIXELSX/Y` за вычетом `PHYSICALOFFSETX/Y` → `EndPage`, `EndDoc`, `DeleteDC`. Чистая функция
  `target_rect(width_mm, height_mm, dpi_x, dpi_y, offset_x, offset_y) -> (i32, i32, i32, i32)` — под тестом.

Пункты:
1. `parse_lpstat_e` на выводе с пробелами в имени не бывает (CUPS их не разрешает), с кириллицей и пустым выводом.
2. `parse_lpstat_d`: «system default destination: X», «no system default destination».
3. `lp_args` — имя принтера одним аргументом, копии, медиа только для наклеек.
4. `target_rect` для 203 и 600 dpi со смещением полей.
5. Тест-закрепка разрешений: добавлены ровно `allow-list-printers`, `allow-print-format`, `allow-print-pdf`,
   `allow-print-pages`.
6. Сборка на 3 ОС — задание CI `desktop`.

- [ ] Step 1: тесты → падают. Step 2: модули печати и команды. Step 3: `cargo test`, `cargo clippy` в
  `apps/desktop/src-tauri`; `npm run licenses` в `apps/desktop`; ручная проверка на этом Mac: `list_printers` в
  отладочной сборке (если принтеров нет — пустой список без ошибки).
- [ ] Step 4: Commit `feat(desktop): список принтеров и печать — CUPS и Windows GDI`.

---

### Task 8: Интерфейс — «Документы и реквизиты» и «Печать»

**Files:** `apps/ui/src/lib/settings/sections.ts`, новые `apps/ui/src/routes/(app)/settings/documents/+page.svelte`
и `settings/print/+page.svelte`, `apps/ui/src/lib/api/documents.ts`, `apps/ui/src/lib/print/{settings.svelte.ts,
platform.ts}`, `apps/ui/src/lib/i18n/ru.ts`, тесты рядом, e2e `apps/ui/e2e/settings-documents.spec.ts`.

**Interfaces:**
- Produces: `documentPdf(kind: DocumentKind, target: { orderId?: number }, opts?: DocumentOptions): Promise<Blob>` и
  `documentPages(kind, target, opts): Promise<DocumentPages>` в `#lib/api/documents.js`
  (`DocumentOptions = { dpi?: number; labelMm?: [number, number]; offsetMm?: [number, number] }`).
- Produces: `printSettings` (`#lib/print/settings.svelte.js`) — `{ actPrinter: string | null; labelPrinter: string |
  null; labelMm: [number, number]; offsetMm: [number, number]; dpi: 203 | 300 | 600; actCopies: number; labelCopies:
  number; onIntake: { act: boolean; sticker: boolean }; onIssue: { act: boolean } }`, ключ `mw.print`, по умолчанию
  58×40, 0/0, 203, 1, 1, акт при приёме — да, наклейка — нет, акт выдачи — да; разбор с откатом к умолчаниям.
- Produces: `#lib/print/platform.js` — `canPrintDirectly(): boolean` (`isTauri()`), `listPrinters()`, `printFormat()`,
  `printPdf(...)`, `printPages(...)` — обёртки `invoke` (динамический импорт `@tauri-apps/api/core`).
- «Документы и реквизиты» (право `settings_manage`, иначе только чтение): `firm_phone`, `firm_details`,
  `firm_legal_details`, `intake_terms`, `issue_terms` (многострочные), подсказки; список документов с кнопкой
  «Образец» (предпросмотр Task 9 на `sample`). Поправить подсказки `ru.help.general.firmName` и `currency`.
- «Печать» (этот компьютер): в браузере — пояснение «Печать из браузера — через PDF»; в программе — принтеры актов и
  наклеек (`Select` из `listPrinters`, «Принтер по умолчанию»), размер наклейки (`SegmentedControl` 58×40, 40×30,
  «Свой» с двумя полями в мм), смещение X/Y, DPI, копии, «Пробная наклейка» (калибровка на принтер наклеек).

Пункты:
1. Vitest: разбор `mw.print` — мусор, старые версии и пустое хранилище дают умолчания; размеры вне 15..=200 отклоняются.
2. Vitest: `DocumentOptions` → параметры запроса (`label_w_mm`, …).
3. e2e: реквизиты сохраняются и видны после перезагрузки; без `settings_manage` поля только для чтения.
4. e2e: «Печать» в браузере показывает пояснение и сохраняет размер наклейки на устройстве.

- [ ] Step 1: тесты → падают. Step 2: страницы и модули. Step 3: `npm run verify`, e2e двух спек в обоих движках.
- [ ] Step 4: Commit `feat(ui): настройки «Документы и реквизиты» и «Печать»`.

---

### Task 9: Интерфейс — печать из заказа, приёма и выдачи, предпросмотр

**Files:** `apps/ui/src/lib/print/{print.svelte.ts,DocumentPreview.svelte,PrintMenu.svelte}`,
`apps/ui/src/lib/orders/{OrderPanel.svelte,IssueDialog.svelte,WarrantyCaseDialog.svelte}`,
`apps/ui/src/routes/(app)/orders/[id]/+page.svelte`, `apps/ui/src/lib/intake/{IntakeAside.svelte,intake.svelte.ts}`,
команды Ctrl K (`apps/ui/src/lib/ui/commands.ts` или место регистрации команд приложения), `ru.ts`, тесты рядом,
e2e `apps/ui/e2e/print.spec.ts`, витрина `/design` (раздел для `PrintMenu` и `DocumentPreview`).

**Interfaces:**
- Consumes: Task 8 (`documentPdf`, `documentPages`, `printSettings`, `#lib/print/platform.js`).
- Produces: `printDocument(kind: DocumentKind, target: { orderId?: number }): Promise<void>` — в программе с выбранным
  принтером (акты — `actPrinter`, наклейка и калибровка — `labelPrinter`): формат по `printFormat()`, печать, уведомление
  «Отправлено на <принтер>»; ошибка — уведомление с текстом ошибки и кнопкой «Предпросмотр»; без принтера или в
  браузере — открывает предпросмотр. `openPreview(kind, target)`.
- `DocumentPreview` — `Sheet`: страницы картинками (PNG 110 dpi, `blob:`-адреса, освобождаются при закрытии), кнопки
  «Печать» (только в программе) и «Сохранить PDF» (скачивание `<a download>` с именем из ответа; в программе его
  ловит существующий `on_download`).
- `PrintMenu` — «Печать ▾» в панели и полной карточке: «Акт приёма», «Акт выдачи», «Наклейка», разделитель, «Акт
  приёма по гарантии» и «Акт выдачи после гарантии» (только у заказа с датой выдачи), «Предпросмотр…».
- Ctrl/⌘P в панели и карточке: невыданный заказ — акт приёма, выданный — акт выдачи; браузерная печать страницы
  не открывается.
- Приём: переключатели «Акт приёма» и «Наклейка» (`printSettings.onIntake`) — после сохранения заказа печатаются
  выбранные документы; выдача: «Печатать акт выдачи» (`onIssue.act`); приём по гарантии: «Печатать акт приёма по
  гарантии» (тот же `onIntake.act`). Ошибка печати не отменяет сохранённый заказ.
- Ctrl K: «Пустой акт приёма» → `printDocument('blank_intake_act', {})`.

Пункты:
1. Vitest: выбор действия Ctrl/⌘P по состоянию заказа; выбор принтера по виду документа; без принтера → предпросмотр.
2. Vitest: `printDocument` при ошибке `printPdf` показывает уведомление с текстом ошибки (платформа подменена).
3. e2e: «Печать ▾ → Предпросмотр» открывает страницы; «Сохранить PDF» скачивает `akt-priema-<номер>.pdf`.
4. e2e: Ctrl/⌘P в панели открывает предпросмотр акта приёма (в браузере) и не открывает печать браузера.
5. e2e: приём с включённым «Акт приёма» открывает предпросмотр после сохранения; выключенный — нет.
6. Обход экранов `layout.spec` не ломается (новая кнопка в панели при 1280×720 и на телефоне).

- [ ] Step 1: тесты → падают. Step 2: модули, компоненты, места вызова. Step 3: `npm run verify`, `npm run test:e2e`.
- [ ] Step 4: Commit `feat(ui): печать из заказа, приёма и выдачи, предпросмотр и PDF`.

---

### Task 10: Знак валюты по всей программе

**Files:** `apps/ui/src/lib/format/money.ts` и места вызова `formatMoney`/`MoneyInput`, хранилище настроек приложения
(где интерфейс держит `AppSettings`), `ru.ts` (подсказки `ru.help.general.currency`, доля мастера), тесты рядом.

**Interfaces:**
- Consumes: `AppSettings.currency_symbol` (уже приходит с сервера).
- Produces: суммы в интерфейсе — со знаком из настроек; `formatMoney(k)` без `currency` берёт знак из настроек
  приложения (по умолчанию «₽», пока настройки не загружены).

Пункты:
1. Vitest: смена знака в настройках меняет вывод `formatMoney` и `MoneyInput`.
2. e2e: «Общие → Валюта = $» → в панели заказа суммы с «$»; вернуть «₽».
3. Подсказки больше не говорят «только хранится»; закрыть в README «Заметки → План 6» пункт о знаке валюты.

- [ ] Step 1: тесты → падают. Step 2: реализация. Step 3: `npm run verify`, затронутые e2e.
- [ ] Step 4: Commit `feat(ui): знак валюты из настроек во всей программе`.

---

### Task 11: Документы, итоговая проверка, слияние

**Files:** `docs/superpowers/plans/README.md` (строка плана 6 — «выполнен», «Заметки → План 6» — убрать закрытое,
добавить отложенное в «План 7»/«План 10»), `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` (5.1 —
шрифт Libertinus Serif и «руб.» вместо «₽», что модель шаблона пока в коде), `docs/superpowers/specs/2026-11-15-ui-design-system-design.md`
(раздел 7 — как сделано), `AGENTS.md` (правила: данные не в разметку Typst, шрифты документов только OFL).

- [ ] Step 1: Полные проверки: Rust на обеих СУБД, `cargo deny check licenses`, `npm run verify` и `npm run test:e2e`
  в `apps/ui`, `cargo test` и `npm run licenses` в `apps/desktop`; релизная сборка программы для компьютера.
- [ ] Step 2: Итоговое ревью всей ветки (субагент `opus`) и волна исправлений.
- [ ] Step 3: Push ветки, CI на GitHub зелёный во всех заданиях (смотреть самому: `gh run list --branch …`).
- [ ] Step 4: Перемотка `main`, `git push origin main`, удалить ветку и worktree; запустить «Сборку установщиков»
  (`gh workflow run desktop-release.yml`) и проверить, что все три установщика собрались.
- [ ] Step 5: Commit `docs: план 6 выполнен` (до перемотки, в ветке).

## Решения, принятые при планировании

- **Шрифт — Libertinus Serif.** Inter (шрифт интерфейса) есть в проекте только в woff2, а Typst читает TTF/OTF; в
  `typst-assets` под OFL лишь Libertinus. Серифный шрифт привычен для актов. Если автор захочет Inter — положить
  официальные TTF из релиза rsms/inter (OFL) рядом и добавить в `fonts.rs`.
- **Знак «₽» → «руб.»** в документах (в шрифте нет глифа); в интерфейсе остаётся «₽».
- **Модель шаблона в коде, без таблицы.** Пользовательские шаблоны, `.mwtemplate`, «свой Typst-код» — план 7.
- **Данные — отдельным JSON**, а не подстановкой в разметку: защищает от «внедрения» разметки через поля заказа.
- **Windows печатает растром от сервера** (`format=png` в DPI принтера), macOS/Linux — PDF в CUPS: не нужен свой
  растеризатор PDF на клиенте.
- **Массовая печать из списка заказов — не в этом плане** (у автора ~200 заказов в год): переносится в заметки плана 7.
- **QR-код на документах — не в этом плане**: ему нужна ссылка на статус заказа (план 9); переносится в заметки.
- **Гарантийные акты** — в меню у заказа с датой выдачи; признак «был гарантийный случай» не хранится отдельно
  (только запись журнала), отдельный признак не вводим.
