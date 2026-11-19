# План 5.4. Настройки и справочники — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** «Настройки» становятся рабочими: общие настройки, настройки заказов, справочники («используется в N»,
архив, порядок, переименование и объединение вместе с заказами), статусы (цвет, вид с предупреждением), точки,
«О программе»; CI перестаёт зависать и предупреждать.

**Architecture:** сервер уже умеет справочники, статусы, точки и настройки; план добавляет архив значений справочника
(новая миграция), счётчики использования, переименование и объединение с правкой заказов (как обычная правка: версия,
`search_text`, история заказа), `GET` следующего номера и «О программе». Интерфейс — подразделы `/settings/*` по
образцу «Внешнего вида» из 5.3, общие компоненты `SortableList` и `ColorSwatches`, автосохранение через `createAutosave`.

**Tech Stack:** Rust (Axum 0.8, SeaORM 2, SQLite + PostgreSQL), SvelteKit 3 SPA, Svelte 5 (руны), Vitest, Playwright.

**Spec:** `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` — разделы 6.7, 6.8, 5 (п. 5 «Настройки»),
10, 11; основная спека `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` — справочники и статусы.

## Global Constraints

- Всё из `AGENTS.md`: запись в БД только через `Core::begin_write()`, права проверяет `mw-core`, тесты с БД — по
  `all_backends()`, выпущенные миграции не меняются, входные типы API с `deny_unknown_fields`, после изменения API —
  `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1` и коммит вместе с кодом;
  любая правка построителей `search_text` увеличивает `SEARCH_INDEX_VERSION`.
- Сравнение значений справочника с полями заказов — без учёта регистра, только через `mw_domain::search::normalize_search`.
- Управление настройками, справочниками, статусами и точками — право `settings_manage`; без него разделы только для
  чтения с подписью «Изменять настройки может администратор» (в режиме без входа права есть у всех).
- Интерфейс: правила `AGENTS.md` («Интерфейс»): токены, руны, `#lib/...`, тексты в `ru.ts`, ширины — `display` и
  `data-width`, автосохранение — `createAutosave`, e2e в Chromium и WebKit на настоящем сервере с `uniq()`.
- Каждый новый подраздел настроек добавляет сцену в обход экранов `e2e/layout.spec.ts` (все случаи, оба движка).
- Перед коммитом зелёное: Rust — `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings &&
  cargo check --workspace && cargo test --workspace` (и с `MW_TEST_PG_URL`); интерфейс — `cd apps/ui && npm run verify
  && npm run test:e2e`.
- Коммиты по-русски `feat(…)/fix(…)/test(…)/docs:`, последняя строка `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Переименование и объединение на смешанных данных: «samsung» в заказе при значении «Samsung»; списки
   («Чехол, зарядка, чехол») — меняются только целые элементы без учёта регистра, повторы схлопываются, остальное и
   порядок не трогаются, нет «, ,» и пробелов по краям.
2. Заказ открыт в панели на другом компьютере, пока справочник переименовывает его поле: версия заказа растёт, панель
   получает обычный спор версий, а не тихую перезапись.
3. Архивное значение пропадает из подсказок и чипов приёма, но заказы со старым значением показывают его как есть;
   «Вернуть из архива» возвращает в подсказки.
4. Пользователь без `settings_manage`: разделы открываются, поля недоступны, автосохранение ничего не отправляет,
   ошибок 403 в уведомлениях нет.
5. Смена вида последнего живого статуса «Выдан» или его архив — сервер отказывает (`last_issued_status`), интерфейс
   говорит понятно, что нужен хотя бы один статус вида «Выдан», и возвращает прежнее значение.

---

### Task 1: CI без зависаний и предупреждений; дорожная карта

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `docs/superpowers/plans/README.md`

CI:
- `actions/checkout@v4` → `@v5`, `actions/setup-node@v4` → `@v5`, `actions/upload-artifact@v4` → `@v5` (Node 24);
- у каждого задания `timeout-minutes`: rust и postgres — 40, ui — 20, ui-e2e — 45;
- установка браузеров в `ui-e2e` — до трёх попыток по 5 минут:
  `for i in 1 2 3; do timeout 300 npx playwright install --with-deps chromium webkit && exit 0; echo "retry $i"; done; exit 1`
  (шаг `shell: bash`, `timeout-minutes: 16`).

Дорожная карта (`README.md`):
- новая строка **5.5 «Доводка заказов, клиентов и фото»** — весь блок заметок «доводка интерфейса», «вложения» и
  «после плана 5.3» из «План 5.4» переезжает в «План 5.5»; в «План 5.4» остаются только «смена вида статуса…»,
  «гарантия: `warrantyUntil`…» и «ширины и порядок колонок, `BlistColor`… из `legacy_rows`» (последнее — тоже в 5.5);
- «Десктоп, первый запуск и импорт» становится **5.6** (строка и заметки); ссылки на «план 5.5» в спеке
  интерфейса (§11 решение 47, §12) поправить на 5.6, где речь о десктопе.

- [ ] **Step 1:** правки `ci.yml`; `npx --yes @action-validator/cli .github/workflows/ci.yml` или, если недоступно,
  `python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/ci.yml'))"` — без ошибок.
- [ ] **Step 2:** правки `README.md` и ссылок в спеке.
- [ ] **Step 3: Commit** `ci: действия на Node 24, ограничения по времени и повтор установки браузеров` (+ `docs:`
  отдельным коммитом для дорожной карты).

---

### Task 2: Сервер — архив значений, счётчики использования, следующий номер, «О программе»

**Files:**
- Create: `crates/mw-db/src/migration/m20261119_000001_dictionary_archive.rs` (+ регистрация в `migration/mod.rs`)
- Modify: `crates/mw-db/src/entity/dictionary_item.rs`, `crates/mw-core/src/dictionaries.rs`,
  `crates/mw-core/src/statuses.rs`, `crates/mw-core/src/service_points.rs`, `crates/mw-core/src/orders/list.rs`
- Create: `crates/mw-core/src/usage.rs` (общий тип и подсчёт)
- Modify: `crates/mw-api/src/routes/catalogs.rs`, `crates/mw-api/src/routes/orders.rs`,
  `crates/mw-api/src/routes/meta.rs`, `crates/mw-api/src/routes/mod.rs`; контракт (`openapi.json`, `bindings/`)
- Modify: места интерфейса, которые вызывают изменённые эндпоинты (`apps/ui/src/lib/api/catalogs.ts` и вызывающие)
- Test: `crates/mw-core/tests/` (по образцу соседних), `crates/mw-api/tests/`

**Interfaces — Produces:**
```rust
// mw-core
pub struct CatalogUsage { pub id: i64, pub count: u64 }               // Serialize, TS, ToSchema
pub struct DictionaryItem { /* прежние поля */ pub archived: bool }
pub struct DictionaryItemPatch { pub value: Option<String>, pub archived: Option<bool> } // patch::present, deny_unknown_fields
impl Core {
  pub async fn list_dictionary(&self, kind: DictionaryKind, include_archived: bool) -> Result<Vec<DictionaryItem>, CoreError>;
  pub async fn update_dictionary_item(&self, actor: &Actor, id: i64, patch: DictionaryItemPatch) -> Result<DictionaryItem, CoreError>;
  pub async fn dictionary_usage(&self, actor: &Actor, kind: DictionaryKind) -> Result<Vec<CatalogUsage>, CoreError>;
  pub async fn status_usage(&self, actor: &Actor) -> Result<Vec<CatalogUsage>, CoreError>;
  pub async fn service_point_usage(&self, actor: &Actor) -> Result<Vec<CatalogUsage>, CoreError>;
  pub async fn next_order_number(&self, actor: &Actor) -> Result<i64, CoreError>;
}
pub struct AboutInfo { pub version: String, pub database: String /* "SQLite" | "PostgreSQL" */,
  pub data_dir: Option<String>, pub orders: u64, pub clients: u64 }   // в mw-api, Serialize, TS, ToSchema
```
HTTP:
- `GET /api/v1/dictionaries/{kind}?archived=true` — с архивными (без параметра — без них, как раньше для приёма);
- `PATCH /api/v1/dictionary-items/{id}` — тело `DictionaryItemPatch` (заменяет прежнее тело переименования);
- `GET /api/v1/dictionaries/{kind}/usage`, `GET /api/v1/statuses/usage`, `GET /api/v1/service-points/usage` →
  `Vec<CatalogUsage>` (право `settings_manage`; значения без использования в ответ не попадают);
- `GET /api/v1/orders/next-number` → `NextOrderNumber` (право `settings_manage`);
- `GET /api/v1/about` → `AboutInfo` (право `settings_manage`).

Поле заказа для каждого вида справочника (`usage.rs`, `fn usage_target(kind) -> UsageTarget`), считаются только
неудалённые записи:

| Вид | Где | Как |
|---|---|---|
| `device_type`, `brand`, `color`, `master` | `orders.<то же имя>` | значение целиком |
| `warranty` | `orders.warranty_text` | значение целиком |
| `completeness` | `orders.completeness` | элементы через «,» |
| `condition` | `orders.device_condition` | элементы через «,» |
| `malfunction` | `orders.malfunction` | элементы через «,» |
| `work_done` | `orders.work_done` | элементы через «,» |
| `client_source` | `clients.source` | значение целиком (считаются клиенты) |

Подсчёт: один запрос `SELECT <поле>, COUNT(*) … GROUP BY <поле>`, дальше в Rust — разбиение на элементы и
сопоставление по `normalize_search` с элементами справочника; элемент, встречающийся в заказе дважды, считает заказ
один раз.

- [ ] **Step 1: Тесты** (`all_backends()`): архив скрывает значение из `list_dictionary(kind, false)` и оставляет в
  `(kind, true)`; `update_dictionary_item` с `archived` и с `value` (дубль → `duplicate`, без изменений — без записи
  в журнал); `dictionary_usage(brand)` при заказах «Samsung», «samsung», «Apple» и удалённом «Samsung» → Samsung 2,
  Apple 1; `completeness` при «Чехол, зарядка» и «чехол» → Чехол 2, Зарядка 1; `client_source` считает клиентов;
  `status_usage` и `service_point_usage` по заказам; `next_order_number` после `set_next_order_number(500)` → 500;
  все пять — `Forbidden` без `settings_manage`. В `mw-api`: коды ответов и форма JSON новых эндпоинтов.
- [ ] **Step 2:** запустить — падают.
- [ ] **Step 3:** миграция `archived BOOLEAN NOT NULL DEFAULT false`, сущность, `usage.rs`, методы, маршруты,
  контракт; прежний вызов переименования в интерфейсе — на новый `PATCH`. Приём и панель продолжают брать
  справочники без архивных.
- [ ] **Step 4:** полные проверки Rust (обе СУБД) и интерфейса — зелёные.
- [ ] **Step 5: Commit** `feat(core,api): архив значений справочника, счётчики использования, номер и «О программе»`.

---

### Task 3: Сервер — переименование и объединение вместе с заказами

**Files:**
- Create: `crates/mw-core/src/dictionaries_rewrite.rs` (замена значений в заказах и клиентах)
- Modify: `crates/mw-core/src/dictionaries.rs`, `crates/mw-core/src/lib.rs`, `crates/mw-api/src/routes/catalogs.rs`,
  `crates/mw-api/src/routes/mod.rs`; контракт
- Test: `crates/mw-core/tests/`, `crates/mw-api/tests/`

**Interfaces:**
- Consumes: `usage_target(kind)` из Task 2.
- Produces:
  ```rust
  pub struct DictionaryRename { pub value: String, pub in_orders: bool }               // deny_unknown_fields
  pub struct DictionaryMerge { pub target_id: i64, pub source_ids: Vec<i64>, pub in_orders: bool }
  pub struct RewriteResult { pub item: DictionaryItem, pub records_changed: u64 }     // заказы, для client_source — клиенты
  impl Core {
    pub async fn rename_dictionary_item_everywhere(&self, actor: &Actor, id: i64, req: DictionaryRename) -> Result<RewriteResult, CoreError>;
    pub async fn merge_dictionary_items(&self, actor: &Actor, kind: DictionaryKind, req: DictionaryMerge) -> Result<RewriteResult, CoreError>;
  }
  ```
  HTTP: `POST /api/v1/dictionary-items/{id}/rename`, `POST /api/v1/dictionaries/{kind}/merge` → `RewriteResult`.

Правила:
- Всё в одной транзакции `begin_write()`; право `settings_manage`.
- Переименование: значение справочника меняется (проверка дубля как раньше); при `in_orders` — в каждой неудалённой
  записи из таблицы Task 2 значение (целиком) или элемент списка, совпавший по `normalize_search` со **старым**
  значением, заменяется новым.
- Объединение: `source_ids` (не пусто, без `target_id`, того же вида) удаляются; значение `target_id` остаётся;
  при `in_orders` совпавшие с любым источником значения и элементы заменяются значением цели.
- В списках после замены повторы схлопываются (первое вхождение остаётся на месте), разделитель — «, ».
- Изменённая запись — как обычная правка: `version + 1`, `updated_at`, `search_text` и ключи (`device_type_key`,
  `brand_key`, `master_key`; у клиента — свои), запись в историю заказа/клиента с полем «до → после» — так же, как
  пишет обычная правка; для гарантии — пересчёт срока тем же кодом, что при правке заказа. Плюс одна запись в
  журнал `dictionary.renamed` / `dictionary.merged` с числом изменённых записей.
- Неизменившаяся запись не трогается.

- [ ] **Step 1: Тесты** (`all_backends()`):
  - бренд «Samsung» → «Samsung Electronics» с `in_orders`: заказы «Samsung» и «samsung» меняются, их `version`
    +1, поиск по «electronics» их находит; «Apple» не тронут; без `in_orders` заказы не меняются;
  - комплектность: объединение «Чехол» ← «чехол», «Чехол силиконовый» нет; заказ «чехол, Зарядка, Чехол» →
    «Чехол, Зарядка»; заказ «Чехол силиконовый» не тронут;
  - `client_source`: «интернет» + «Интернет» → «Интернет», клиенты меняются, `records_changed` = их число;
  - мастер: `master_key` пересчитан; гарантия: срок пересчитан;
  - история заказа содержит запись правки поля с прежним и новым значением;
  - ошибки: `source_ids` пусто, содержит `target_id`, чужой вид → `Validation`; без права → `Forbidden`.
- [ ] **Step 2:** запустить — падают.
- [ ] **Step 3:** реализация; контракт.
- [ ] **Step 4:** полные проверки — зелёные.
- [ ] **Step 5: Commit** `feat(core,api): переименование и объединение значений справочника вместе с заказами`.

---

### Task 4: Компоненты `SortableList` и `ColorSwatches`

**Files:**
- Create: `apps/ui/src/lib/ui/SortableList.svelte`, `apps/ui/src/lib/ui/SortableList.test.ts`,
  `apps/ui/src/lib/ui/ColorSwatches.svelte`, `apps/ui/src/lib/ui/ColorSwatches.test.ts`
- Modify: `apps/ui/src/lib/ui/index.ts`, витрина `src/routes/design/` (раздел для каждого), `src/lib/i18n/ru.ts`

**Interfaces — Produces:**
```ts
// SortableList.svelte
props: { items: T[]; key: (item: T) => number; label: (item: T) => string; row: Snippet<[T]>;
         onreorder: (ids: number[]) => void; disabled?: boolean }
// ColorSwatches.svelte
props: { value: StatusHue; onchange: (hue: StatusHue) => void; label: string; disabled?: boolean }
```
- `SortableList`: у каждой строки ручка (`GripVertical`, кнопка с `aria-label` «Переместить „<label>“»);
  перетаскивание указателем (pointer events, с захватом; строка следует за указателем, место вставки видно);
  с клавиатуры: фокус на ручке, ↑/↓ двигают строку на одну позицию, фокус остаётся на ней, объявление в
  `aria-live` «„<label>“ — место N из M». `onreorder` — один раз на отпускание или нажатие, с новым порядком id.
  Без изменений порядка — не вызывается. `disabled` — ручек нет.
- `ColorSwatches`: `role="radiogroup"` из 12 оттенков `STATUS_HUES` (`#lib/design/status-colors.js`), у каждого
  `aria-label` с русским названием (новые строки в `ru.ts`), стрелки двигают выбор (родной радио-ввод),
  выбранный отмечен не только цветом (галочка). Цвета — только классы `.st-*` из `tokens.css`.

- [ ] **Step 1: Тесты** (Testing Library): ↓ на ручке второй строки → `onreorder([1,3,2])`, фокус на ручке
  перемещённой строки, объявление «… — место 3 из 3»; ↑ на первой — ничего; `disabled` — ручек нет;
  `ColorSwatches`: 12 радио с подписями, стрелка вправо выбирает следующий и вызывает `onchange`.
- [ ] **Step 2:** запустить — падают. **Step 3:** реализация + витрина. **Step 4:** `npm run verify` и
  `npm run test:e2e` — зелёные (e2e `design` при наличии — открыть раздел витрины и перетащить строку мышью).
- [ ] **Step 5: Commit** `feat(ui): SortableList и ColorSwatches`.

---

### Task 5: «Общие», «Заказы», «О программе»

**Files:**
- Create: `apps/ui/src/routes/(app)/settings/general/+page.svelte`, `.../settings/orders/+page.svelte`,
  `.../settings/about/+page.svelte`, `apps/ui/src/lib/settings/settings-editor.svelte.ts` (автосохранение настроек)
- Modify: `apps/ui/src/lib/settings/sections.ts`, `apps/ui/src/lib/api/catalogs.ts`, `src/lib/i18n/ru.ts`,
  `e2e/settings.spec.ts`, `e2e/layout.spec.ts`
- Modify: спека 6.8

**Interfaces:**
- Consumes: `GET/PATCH /settings`, `GET/PUT /orders/next-number`, `GET /about` (Task 2), `app.refreshCatalogs()`,
  `app.can('settings_manage')`, `createAutosave`.
- Produces: `SETTINGS_SECTIONS` в порядке: `general` «Общие», `appearance` «Внешний вид», `orders` «Заказы»,
  `dictionaries` «Справочники», `statuses` «Статусы», `points` «Точки», `about` «О программе» (`dictionaries`,
  `statuses`, `points` добавляют Tasks 6–7). `/settings` на широком экране ведёт на «Общие».

Поля (подпись → поле `AppSettings`):
- «Общие»: «Название сервиса» → `firm_name`; «Валюта» → `currency_symbol`; «Часовой пояс» → `timezone`
  (`Combobox` по `Intl.supportedValuesOf('timeZone')`, сервер проверяет); «Следующий номер заказа» — отдельное поле,
  сохраняется по Enter или уходу из поля через `PUT /orders/next-number`, при отказе сервера — ошибка под полем и
  прежнее значение; «Процент мастера» → `master_percent` (0–100).
- «Заказы»: «Точка по умолчанию» → `default_service_point_id` (`Select`: живые точки и «Не выбрана»); «Мастер по
  умолчанию» → `default_master` (`Combobox` по справочнику мастеров); «Гарантия по умолчанию» → `default_warranty`
  (`Combobox` по справочнику гарантий); «Долгая диагностика, дней» → `diagnostics_overdue_days`; «Давно готов,
  дней» → `ready_stale_days`; «Проверять обязательные поля при приёме» → `intake_required_fields` (`Switch`).
- «О программе»: версия, база данных, папка данных (с кнопкой «Скопировать»), число заказов и клиентов.
- Реквизиты и тексты договоров (`firm_phone`, `firm_details`, `firm_legal_details`, `intake_terms`, `issue_terms`) —
  не здесь: план 6 («Документы и реквизиты»).

Сохранение: `settings-editor.svelte.ts` — одна очередь `createAutosave<SettingsPatch>` на экран, после ответа —
`app.refreshCatalogs()`; ошибка проверки сервера — под полем; без `settings_manage` все поля `disabled` и
`Callout` «Изменять настройки может администратор», запросов записи нет.

- [ ] **Step 1: Тесты** — unit: редактор не отправляет запрос без права; два быстрых изменения → один `PATCH`.
  e2e (`settings.spec.ts`, сервер 4173 и 4174): «Название сервиса» сохраняется и переживает перезагрузку;
  «Долгая диагностика, дней» 0 → ошибка под полем «От 1 до 365» (сервер: `out_of_range`, допустимо 1–365), после
  перезагрузки — прежнее значение; «Валюта» пустая → «Обязательное поле»; «Следующий номер заказа» 900000 → новый заказ через API получает 900000;
  «О программе» показывает версию из `/server-info`; на сервере 4174 под администратором поля доступны.
- [ ] **Step 2:** запустить — падают. **Step 3:** реализация. Сцены `general`, `orders-settings`, `about` в
  `layout.spec.ts` (все случаи).
- [ ] **Step 4:** спека 6.8: что где; «что обязательно при выдаче» — нет правила на сервере, перенесено в заметки
  плана 5.5. Проверки — зелёные.
- [ ] **Step 5: Commit** `feat(ui): настройки «Общие», «Заказы» и «О программе»`.

---

### Task 6: «Справочники»

**Files:**
- Create: `apps/ui/src/routes/(app)/settings/dictionaries/+page.svelte`, `apps/ui/src/lib/settings/CatalogRows.svelte`,
  `apps/ui/src/lib/settings/MergeDialog.svelte`, `apps/ui/src/lib/settings/RenameEverywhereDialog.svelte`,
  `apps/ui/src/lib/settings/catalog.svelte.ts` (загрузка, счётчики, действия)
- Modify: `sections.ts`, `apps/ui/src/lib/api/catalogs.ts`, `ru.ts`, `e2e/settings-dictionaries.spec.ts` (новый),
  `e2e/layout.spec.ts`

**Interfaces:**
- Consumes: Tasks 2–4.
- Produces (`CatalogRows.svelte`, его же берёт Task 7):
  ```ts
  props: { rows: { id: number; label: string; archived: boolean; count: number }[]; countLabel: (n: number) => string;
           selectable?: boolean; selected?: number[] /* bindable */; readonly: boolean;
           onrename: (id: number, label: string) => void; onarchive: (id: number, archived: boolean) => void;
           ondelete?: (id: number) => void; onreorder: (ids: number[]) => void; extra?: Snippet<[number]> }
  ```

Экран:
- Вид справочника — слева список из 10 видов (на `xs`/`sm` — `Select`), в адресе `?kind=<код>`.
- Сверху: поле «Найти» (фильтр на месте) и «Добавить значение» (Enter добавляет; дубль — ошибка под полем).
- Строка: ручка, флажок (для объединения), значение — по щелчку или Enter правится на месте (Enter сохраняет,
  Esc отменяет), «в N заказах» (у «Откуда узнали» — «у N клиентов»), «В архив» / «Вернуть», «Удалить» — только
  при N = 0 (через `ConfirmDialog`). Архивные — внизу под заголовком «Архив», приглушённые, без ручки.
- Переименование значения с N > 0: после Enter — окно «Переименовать „старое“ в „новое“» с флажком «И в N заказах»
  (включён) → `POST …/rename`; уведомление «Переименовано, изменено N заказов».
- «Объединить выбранные» (от двух отмеченных) → окно: выбрать, какое значение оставить (радио, по умолчанию с
  большим N), флажок «Заменить и в заказах» (включён), подтверждение → `POST …/merge`; уведомление с числом.
- После действий — счётчики перечитываются; без права — всё только для чтения.

- [ ] **Step 1: Тесты** — unit `catalog.svelte.ts` (фильтр, порядок, отказ сервера возвращает прежнее значение);
  e2e (`uniq()` значения, оба движка): добавить два бренда «X-<uniq>» и «x-<uniq>» нельзя (дубль) — добавить
  «Alpha-<uniq>» и «Beta-<uniq>», создать заказ с брендом «alpha-<uniq>» через API → счётчик «в 1 заказе»;
  переименовать «Alpha-<uniq>» в «Gamma-<uniq>» с флажком → заказ через API имеет «Gamma-<uniq>»; объединить
  «Gamma-<uniq>» и «Beta-<uniq>», оставить «Beta-<uniq>» → в списке одно значение, заказ — «Beta-<uniq>»;
  в архив → значение пропадает из подсказок бренда в приёме и возвращается после «Вернуть»; перетаскивание мышью
  меняет порядок и он переживает перезагрузку.
- [ ] **Step 2:** запустить — падают. **Step 3:** реализация; сцена `dictionaries` в обходе.
- [ ] **Step 4:** проверки — зелёные. **Step 5: Commit** `feat(ui): настройки «Справочники»`.

---

### Task 7: «Статусы» и «Точки»

**Files:**
- Create: `apps/ui/src/routes/(app)/settings/statuses/+page.svelte`, `.../settings/points/+page.svelte`,
  `apps/ui/src/lib/settings/status-kind.ts` (пояснения и текст предупреждения)
- Modify: `sections.ts`, `apps/ui/src/lib/api/catalogs.ts`, `ru.ts`, `apps/ui/src/lib/ui/StatusPicker.svelte`
  («Ещё ▾»), `e2e/settings-statuses.spec.ts` (новый), `e2e/layout.spec.ts`
- Modify: спека 6.7 (как сделано), README (строка 5.4 — «выполнен» делает контроллер при слиянии)

**Interfaces:**
- Consumes: `CatalogRows` (Task 6), `SortableList`, `ColorSwatches` (Task 4), `status_usage`, `service_point_usage`
  (Task 2), существующие `POST/PATCH /statuses`, `PUT /statuses/order`, то же для точек, `app.refreshCatalogs()`.
- Produces: `export function kindChangeWarning(from: StatusKind, to: StatusKind, count: number): string | null` и
  `export const KIND_HINTS: Record<StatusKind, string>` в `status-kind.ts`.

Статусы: строка `CatalogRows` + `extra`: точка цвета-кнопка (открывает `Popover` с `ColorSwatches`), `Select` вида
с пояснением под ним (`KIND_HINTS`: «Открыт» — «заказ в работе»; «Диагностика» — «идёт диагностика, считается срок
долгой диагностики»; «Готов» — «можно забирать, считается срок „давно готов“»; «Выдан» — «заказ выдан, попадает в
отчёты и гарантию»). Смена вида при N > 0 — `ConfirmDialog` с текстом `kindChangeWarning`:
- в «Выдан»: «N заказов станут выданными и попадут в отчёты»;
- из «Выдан»: «N заказов перестанут считаться выданными и пропадут из отчётов»;
- в «Готов»: «N заказов попадут в „Готовы“ и „Давно готовы“»;
- иначе: «N заказов сменят вид на „<вид>“».
Отказ `last_issued_status` → «Нужен хотя бы один статус вида „Выдан“», значение возвращается. Новый статус:
название, вид, цвет. После любого изменения — `app.refreshCatalogs()` (фильтры и `StatusPicker` видят сразу).

Точки: `CatalogRows` без объединения; добавление; «в N заказах»; ссылка «Точка по умолчанию — в „Заказах“».

`StatusPicker`: при числе живых статусов больше 7 — первые 7 кнопками и «Ещё ▾» (`DropdownMenu`) с остальными.

- [ ] **Step 1: Тесты** — unit `kindChangeWarning` (все четыре ветки, `count` 0 → `null`); `StatusPicker` с 9
  статусами показывает 7 и «Ещё». e2e (оба движка): новый статус «Ждёт клиента-<uniq>» вида «Открыт», цвет «синий»
  → виден в фильтрах «Заказов» без перезагрузки; заказ в нём через API → смена вида на «Выдан» показывает «1 заказ
  станет выданным…», «Отмена» оставляет вид; перетаскивание меняет порядок в `StatusPicker`; точка «Филиал-<uniq>» —
  добавить, в архив, вернуть.
- [ ] **Step 2:** запустить — падают. **Step 3:** реализация; сцены `statuses`, `points` в обходе.
- [ ] **Step 4:** спека 6.7; проверки — зелёные. **Step 5: Commit** `feat(ui): настройки «Статусы» и «Точки»`.
