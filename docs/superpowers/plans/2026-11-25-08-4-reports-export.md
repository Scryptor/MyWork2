# План 8.4. Отчёты и экспорт

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** раздел «Отчёты» и выгрузки:
- отчёт по выданным заказам за период: число заказов, выручка, затраты, прибыль, «без ремонта», оплата мастерам; разрез по мастеру, точке, типу, бренду или источнику; графики и таблица;
- отчёт в Excel;
- отчёт о перемещениях заказов между точками;
- выгрузка заказов и клиентов в Excel;
- контакты для телефона: CSV для Google Контактов и vCard.

**Architecture:**
- Отчёт строит `mw-core` (`reports/`). Один запрос берёт выданные заказы по условиям в БД: период, точка, мастер, тип, бренд. Второй берёт их запчасти. Суммы, «без ремонта», источник без учёта регистра и разрезы считаются в Rust: на обоих бэкендах одинаково, без `SUM` (в PostgreSQL он даёт `numeric`).
- Формулы — чистые функции в `mw-domain::report`: признак «без ремонта» и оплата мастеру.
- Файлы (xlsx, csv, vcf) строит `mw-core::export` под семафором на 2 выгрузки, тяжёлая часть — в `spawn_blocking`. API отдаёт их с `Content-Disposition: attachment` и русским именем (`filename*`).
- Новое право «Выгрузка заказов, клиентов и контактов» (`data_export`) закрывает выгрузки списков. Отчёт и его Excel закрывает уже существующее `reports_view`.
- Интерфейс: экран `/reports` с вкладками «Заказы» и «Перемещения». Новые общие компоненты в `#lib/ui`: `Chart` (ECharts, SVG, цвета из токенов) и `PeriodPicker`. Вид экрана живёт в адресе.

**Tech Stack:** Rust (mw-domain, mw-db/SeaORM, mw-core, mw-api/Axum + utoipa + ts-rs), `rust_xlsxwriter`, `calamine` (только тесты), SvelteKit 3 + Svelte 5 (руны), ECharts, Playwright.

**Spec:**
- `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md`:
  - раздел 7 «Отчёты и графики»;
  - раздел 8 «Импорт/экспорт»;
  - таблица стека: ECharts и `rust_xlsxwriter`.
- `docs/superpowers/specs/2026-11-15-ui-design-system-design.md`, «Отчёты (план 8)»: сверху период и фильтры, ниже плитки с итогами, графики, таблица и «Экспорт в Excel».
- `docs/superpowers/plans/README.md`:
  - строка 8.4;
  - заметки «План 8.4»;
  - идея «Перемещение заказа между точками».

Как было в MyWork 2 (`legacy/MyWork2/Graf.cs`, `Export.cs`): отчёт только по выданным заказам, период по дате выдачи, оплата мастеру = (цена − затраты) × процент, контакты в CSV для Google и в vCard 3.0.

## Решения плана

1. **В отчёт входят заказы, выданные сейчас.**
   - Условие: вид текущего статуса `issued`, `issued_at` в периоде, заказ не удалён.
   - Период — дни «с» и «по» включительно, по местному времени предприятия: `issued_at >= с 00:00` и `issued_at < (по + 1 день) 00:00`. Нет «с» или «по» — граница открыта.
   - Заказ, который выдали и вернули в работу, из отчёта уходит, хотя `issued_at` у него остался.
   - Так отчёт сходится со списком заказов «Выданные» с тем же периодом выдачи.
2. **Формулы по заказу.** Деньги — копейки, `Money`.
   - Выручка = итог − скидка; нет итога — 0 − скидка. В MyWork 3 «К оплате» = итог − предоплата − скидка, поэтому итог берётся до скидки, и выручка — это деньги, которые клиент реально заплатил. В MyWork 2 скидка не вычиталась, потому что там и «К оплате» её не вычитало.
   - Затраты = «Расходы» (`extra_costs`) + Σ цена × количество запчастей заказа. Это та же «Себестоимость», что в карточке заказа.
   - Прибыль = выручка − затраты.
   - Оплата мастеру = прибыль × процент мастера / 100 с округлением до копейки половиной от нуля. Считается для каждого заказа, а итог — сумма по заказам. Заказ без мастера (нет `master_user_id` и пустой `master_key`) даёт 0. Убыточный заказ даёт отрицательную оплату, как в MyWork 2.
   - Процент — текущая настройка `master_percent`, один на всех.
3. **«Без ремонта»** — признак по тексту «Выполненных работ», как в MyWork 2, но устойчивее. Текст делится на части по `,`, `;` и переводам строки. У каждой части через `normalize_search` убираются регистр, `ё`, лишние пробелы и точка в конце. Признак есть, если хоть одна часть равна «без ремонта».
4. **Разрезы и подписи групп:**
   - мастер: ключ `user:<id>`, для заказов без пользователя — `name:<master_key>`, пустой — «Без мастера». Подпись — имя пользователя, для заказов без пользователя — самое частое написание `master`.
   - точка: ключ — id, подпись — название, у архивной « (в архиве)», без точки — «Без точки».
   - тип и бренд: ключ `*_key`, подпись — самое частое написание, пустой — «Не указан».
   - источник клиента: ключ `normalize_search(clients.source)`, подпись — самое частое написание, пустой — «Не указан».
   - Группы идут по выручке, большие сверху, при равенстве — по подписи.
5. **Шкала времени:** по дням, если в периоде не больше 62 дней, иначе по месяцам. «Всё время» — по месяцам от первой до последней выдачи. Пустые дни и месяцы показываются нулями.
6. **Быстрые периоды** — календарные:
   - «Сегодня»;
   - «Неделя» — с понедельника;
   - «Месяц» — с 1-го числа;
   - «Прошлый месяц»;
   - «Год» — с 1 января;
   - «Всё время»;
   - «Свой период».
   По умолчанию — «Месяц». В адресе хранится вид периода, а не даты, поэтому закладка «Месяц» всегда показывает текущий месяц.
7. **Перемещения** — записи журнала `order.moved`, которые пишет план 8.2.
   - Отчёт показывает пары «откуда → куда» с числом переездов и список переездов: последние 500, новые сверху, с общим числом.
   - Права — `reports_view`, а не `audit_view`.
   - Статуса «в пути» нет: переезд в программе мгновенный. Кому нужен такой этап, заводит свой статус вида «в работе» в «Статусах».
8. **Право `data_export`** «Выгрузка заказов, клиентов и контактов» стоит в `Permission` сразу после `ReportsView`.
   - Миграция данных дописывает его каждой роли, у которой есть все 17 прежних прав, то есть «Администратору».
   - «Мастер» и «Приёмщик» его не получают.
   - Импорт MyWork 2: флаг `settings` даёт и `DataExport`, потому что контакты в MyWork 2 выгружались из «Настроек».
   - Отчёт в Excel — под `reports_view`: кто видит отчёт, тот может его сохранить.
9. **Выгрузки списков** берут фильтр и порядок экрана: заказы — фильтры списка заказов, клиенты — поиск и «Похожие».
   - Не больше `EXPORT_ROWS_MAX = 50 000` строк, иначе ошибка `too_many_rows`.
   - Пароль и графический ключ устройства в выгрузки не попадают никогда.
10. **Контакты:**
    - один контакт на клиента, имя — ФИО как записано (регистр не меняется: «ИП», «ООО»);
    - телефоны — все из `client_phones` по порядку;
    - номер приводится к виду для телефонной книги: российский (10 цифр, 11 с 7 или 8 в начале) — `+7XXXXXXXXXX`, остальные — цифры как есть; номер короче 5 цифр пропускается;
    - номер, который уже вышел у клиента выше в файле, повторно не пишется (по `phone_key`); клиент без телефонов пропускается;
    - выгрузить можно всех клиентов из списка или только добавленных с даты; дата прошлой выгрузки запоминается на этом компьютере;
    - CSV — формат Google Контактов, UTF-8 без BOM, CRLF, кавычки по RFC 4180;
    - vCard 3.0 — CRLF, экранирование `\ , ;` и перевода строки, перенос строк длиннее 75 байт.
11. **Несколько платежей по заказу** выносятся в отдельный план 8.7 «Платежи по заказу». Это отдельная подсистема (деньги заказа, «К оплате» в актах, импорт), и отчётам она не нужна: выручка считается по итогу и скидке, как в MyWork 2.
12. **Имена файлов** (дата — `Core::now()`):
    - «Отчёт по заказам 01.11.2026–30.11.2026.xlsx», без периода — «Отчёт по заказам за всё время.xlsx»;
    - «Заказы 25.11.2026.xlsx»;
    - «Клиенты 25.11.2026.xlsx»;
    - «Контакты 25.11.2026.csv» и «Контакты 25.11.2026.vcf».
    Запасные ASCII-имена: `report.xlsx`, `orders.xlsx`, `clients.xlsx`, `contacts.csv`, `contacts.vcf`.

## Global Constraints

- Деньги — только `Money` (копейки, `i64`), без `f64` в расчётах. В ячейку Excel сумма пишется числом рублей (`kopecks as f64 / 100.0`) только при записи файла.
- Время — `Core::now()` (местное время предприятия) для дат в именах файлов. Периоды отчёта — `NaiveDate` местного времени.
- Поиск и сравнение без учёта регистра — только `mw_domain::search` (`normalize_search`). SQL `lower()` не годится: в SQLite он не знает кириллицу. `SEARCH_INDEX_VERSION` не трогается.
- Права проверяет `mw-core` (`Actor::require`): отчёты — `ReportsView`; выгрузки — `DataExport`, у клиентов и контактов ещё `ClientsView`.
- Каждый тест с БД идёт по `all_backends()`. PostgreSQL — `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`.
- Выпущенные миграции не меняются, только добавляются новые. Новое право — только миграцией данных.
- Пароль устройства (`device_password`) и графический ключ (`device_pattern`) не попадают ни в одну выгрузку.
- Типы API:
  - это типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS`, `utoipa::ToSchema`;
  - входные — с `deny_unknown_fields`, `#[ts(export)]` не ставить;
  - serde `flatten` с `deny_unknown_fields` не работает, поэтому общие поля запросов не склеивать через `flatten`.
- Маршруты — функциями по разделам в `crates/mw-api/src/routes/mod.rs` с `#[inline(never)]` (`report_routes`, `export_routes`). Тест `crates/mw-api/tests/stack.rs` зелёный.
- После изменения API: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`, затем `cargo test -p mw-api --test contract` без переменной. `openapi.json` и `bindings/` — в том же коммите.
- Зависимости — только MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode:
  - `rust_xlsxwriter` — в `[workspace.dependencies]`, `{ workspace = true }` в `mw-core`;
  - `calamine` — только `[dev-dependencies]`;
  - `echarts` — в `dependencies` `apps/ui`;
  - после смены зависимостей `crates/*` обновить `apps/desktop/src-tauri/Cargo.lock` (`cargo check` в `apps/desktop/src-tauri`) и прогнать `npm run licenses` в `apps/ui` и `apps/desktop`;
  - если установлен `cargo deny` — `cargo deny check licenses`.
- Ответы-файлы:
  - `Content-Disposition: attachment; filename="<ascii>"; filename*=UTF-8''<percent-encoded>` и `Cache-Control: private, no-store`;
  - типы содержимого: xlsx `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet`, CSV `text/csv; charset=utf-8`, vCard `text/vcard; charset=utf-8`.
- Коды ошибок (с русским текстом в `message_ru` и в шапке списка кодов `crates/mw-domain/src/validation.rs`):
  - `period_reversed` у поля `to`: «Начало периода позже конца»;
  - `too_many_rows` у поля `rows`: «Слишком много строк для одного файла — не больше 50 000, сузьте выборку»;
  - `nothing_to_export` у поля `rows`: «Нет клиентов с телефонами для выгрузки».
- Интерфейс:
  - Svelte 5 на рунах, обработчики — пропсы `on…`;
  - импорты `#lib/...` с расширениями, типы API — `import type` из `$api/...`;
  - цвета — только токены `tokens.css`, в том числе цвета графиков;
  - ширины — только `display` и `:global([data-width=…])`;
  - тексты — в `ru.ts`;
  - иконки `@lucide/svelte` по одной;
  - даты — `#lib/format/date.js`, деньги — `#lib/format/money.js`;
  - `fetch` — только `src/lib/api/client.ts`, файлы — `requestFile` + `downloadBlob`;
  - новые компоненты — в `#lib/ui/` с разделом на витрине `/design`;
  - вид экрана — в адресе;
  - сквозные тесты — Chromium и WebKit, по ролям и подписям, на настоящем `mw-server`, данные через API с `uniq()`.
- Перед коммитом зелёное:
  - `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace --no-fail-fast`;
  - в задачах с интерфейсом ещё `cd apps/ui && npm run verify && npm run test:e2e`;
  - в задачах с новой зависимостью Rust ещё `cd apps/desktop/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test`.
- Коммиты заканчиваются строкой `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Заказ выдали и вернули в работу.** Он не входит в отчёт, хотя `issued_at` остался. Удалённый заказ тоже не входит. Тест: `reopened_and_deleted_orders_leave_the_report` (задача 1).
2. **Границы периода.** Заказ, выданный в 23:59 последнего дня, — внутри, в 00:00 следующего — снаружи; `с == по` — ровно один день; «с» позже «по» — `period_reversed`. Тест: `period_bounds_are_inclusive_days` (задача 1).
3. **Один источник, написанный по-разному** («Авито», «авито », «АВИТО») — одна группа с подписью самого частого написания. Фильтр по источнику ловит все написания. Тест: `sources_group_without_case` (задача 1).
4. **Русское имя файла.** Оно доходит целиком и в Chromium, и в WebKit, и в программе для компьютера сохраняется в «Загрузки» под этим именем. Тесты: `report_xlsx_has_a_russian_file_name` (задача 2), сквозные проверки `suggestedFilename()` (задачи 4, 6, 7).
5. **Имя контакта с запятой, кавычкой, точкой с запятой или переводом строки; один телефон у двух клиентов.** CSV и vCard остаются разбираемыми, телефон пишется один раз. Тесты: `csv_quotes_what_needs_quoting`, `vcard_escapes_and_folds`, `a_phone_goes_to_the_first_client_only` (задача 7).

---

### Task 1: Отчёт по заказам в ядре

**Files:**
- Create: `crates/mw-domain/src/report.rs` (+ `pub mod report;` в `crates/mw-domain/src/lib.rs`)
- Create: `crates/mw-db/src/migration/m20261125_000002_orders_issued_at_index.rs` (+ регистрация в `crates/mw-db/src/migration/mod.rs` после `m20261125_000001_firm_links`): индекс `idx_orders_issued_at` по `orders.issued_at`.
- Create: `crates/mw-core/src/reports/mod.rs`, `crates/mw-core/src/reports/orders.rs` (+ `mod reports;` и `pub use` типов в `crates/mw-core/src/lib.rs`)
- Modify: `crates/mw-domain/src/validation.rs` — код `period_reversed` (текст и шапка списка), `crates/mw-core/tests/api_types.rs` — код в `field_error_messages_are_russian`
- Test: `crates/mw-core/tests/reports.rs`

**Interfaces:**
- Produces (`mw-domain::report`):
  - `pub fn is_without_repair(work_done: &str) -> bool` — решение 3;
  - `pub fn master_pay(profit: Money, percent: u8) -> Money` — решение 2.
- Produces (`mw-core`, всё с `Serialize, Deserialize, ts_rs::TS, utoipa::ToSchema`, `snake_case`):
  - `enum ReportGroupBy { Master (по умолчанию), ServicePoint, DeviceType, Brand, Source }`;
  - `enum ReportStep { Day, Month }`;
  - `struct ReportFilter { from: Option<NaiveDate>, to: Option<NaiveDate>, service_point_id: Option<i64>, master_user_id: Option<i64>, device_type: Option<String>, brand: Option<String>, source: Option<String>, group_by: ReportGroupBy }` (`Default`);
  - `struct ReportTotals { orders: u64, without_repair: u64, revenue: Money, discounts: Money, costs: Money, profit: Money, master_pay: Money }` (`Default`);
  - `struct ReportGroup { key: String, label: String, totals: ReportTotals }`;
  - `struct ReportPoint { start: NaiveDate, totals: ReportTotals }` — день или первое число месяца;
  - `struct OrderReport { totals: ReportTotals, groups: Vec<ReportGroup>, step: ReportStep, timeline: Vec<ReportPoint>, master_percent: u8 }`;
  - `Core::order_report(&self, actor: &Actor, filter: &ReportFilter) -> Result<OrderReport, CoreError>` — требует `Permission::ReportsView`; «с» позже «по» → `CoreError::Validation([FieldError::new("to", "period_reversed")])`.
- Produces для задачи 2 (внутри `mw-core`):
  - `pub(crate) struct ReportOrderRow` — заказ отчёта со всем, что нужно листу «Заказы»: id, принят, выдан, клиент, основной телефон (первый из `client_phones`), тип, бренд, модель, неисправность, работы, мастер, точка, источник, итог, скидка, затраты, прибыль, оплата мастеру, без ремонта;
  - `pub(crate) async fn report_rows(&self, filter: &ReportFilter) -> Result<Vec<ReportOrderRow>, CoreError>` — по возрастанию `issued_at`, затем id;
  - `order_report` строится из тех же строк.

- [ ] **Step 1: Тесты формул** (в `report.rs`):
  - `without_repair_reads_the_work_done_text`: `true` для «Без ремонта», «без ремонта,», «БЕЗ РЕМОНТА\r\n», «Диагностика, без ремонта», «Без ремонта. », «Без ремонта; клиент отказался»; `false` для «», «Ремонт без замены платы», «Замена дисплея».
  - `master_pay_rounds_half_away_from_zero`: `(101, 50) → 51`, `(-101, 50) → -51`, `(1000, 0) → 0`, `(1000, 100) → 1000`, `(333, 33) → 110` (109,89 → 110).
- [ ] **Step 2: Запустить** `cargo test -p mw-domain report` → не собирается.
- [ ] **Step 3: Реализовать** `report.rs`.
- [ ] **Step 4: Тесты ядра** (`crates/mw-core/tests/reports.rs`, все по `all_backends()`). Данные создаются через `Core` (заказы, выдача, запчасти через `order_parts` API ядра), бренд или тип — уникальные в каждом тесте.
  - `report_sums_issued_orders`. Три заказа одного бренда:
    - А: итог 1000 ₽, скидка 100 ₽, расходы 200 ₽, запчасть 2 × 50 ₽, мастер-пользователь, выдан;
    - Б: итог 500 ₽, работы «Без ремонта», без мастера, выдан;
    - В: не выдан.
    Отчёт с `brand` = этот бренд: `orders 2`, `without_repair 1`, `revenue 1400 ₽`, `discounts 100 ₽`, `costs 300 ₽`, `profit 1100 ₽`, `master_pay 300 ₽` (А: (900 − 300) × 50 %; Б без мастера), `master_percent 50`.
  - `reopened_and_deleted_orders_leave_the_report`: выданный заказ переведён в статус вида `open` — его нет (`orders 0`). Выданный, затем удалённый — тоже нет.
  - `period_bounds_are_inclusive_days`: заказы выданы `2026-03-10 23:59` и `2026-03-11 00:00` (дата выдачи правится через ядро, как в тестах правки дат):
    - `с = по = 2026-03-10` → 1 заказ;
    - `с = 2026-03-11` → 1;
    - без границ → 2;
    - `с 2026-03-12, по 2026-03-11` → `[("to", "period_reversed")]`.
  - `sources_group_without_case`: клиенты с источниками «Авито» (2 заказа), «авито » (1), «АВИТО» (1), «Сарафан» (1), «» (1); у всех заказов одинаковый итог. При `group_by: Source`:
    - группы (подпись, заказов) — `[("Авито", 4), ("Сарафан", 1), ("Не указан", 1)]`;
    - фильтр `source: " АВИТО "` → `orders 4`.
  - `groups_by_master_point_type_and_brand`:
    - мастер-пользователь, мастер только текстом (`master_user_id = NULL`, как у старых заказов) и без мастера → три группы с ключами `user:<id>`, `name:<key>`, `""` и подписями «<имя пользователя>», «<текст>», «Без мастера»;
    - архивная точка → подпись «<имя> (в архиве)»;
    - пустой тип → «Не указан»;
    - порядок групп — по выручке.
  - `timeline_steps_by_day_or_month`:
    - период 2026-03-01..2026-03-31 → `Day`, 31 точка, пустые дни нулями;
    - 2026-01-01..2026-12-31 → `Month`, 12 точек с `start` первого числа;
    - «всё время» при выдачах в январе и марте → `Month`, 3 точки (февраль нулевой).
  - `report_needs_reports_view`: актёр без `ReportsView` → `CoreError::Forbidden(Permission::ReportsView)`.
- [ ] **Step 5: Запустить** `cargo test -p mw-core --test reports` → не собирается.
- [ ] **Step 6: Реализовать.**
  - Миграция индекса.
  - `report_rows`: SeaQuery-выборка `orders` ⋈ `statuses` (`kind = 'issued'`) ⋈ `clients`, `deleted_at IS NULL`, условия фильтра в SQL (период по `issued_at`, точка, `master_user_id`, `device_type_key`/`brand_key` = `normalize_search(значение)`). Вторая выборка — `order_parts` тех же заказов с тем же условием. Источник фильтруется в Rust.
  - Суммы — `Money` с проверкой переполнения (`checked_add`, ошибка — `CoreError::Internal`).
  - Подписи и порядок — решения 4 и 5.
- [ ] **Step 7: Запустить** `cargo test -p mw-domain report && cargo test -p mw-core --test reports && cargo test -p mw-core --test api_types` → PASS, затем полный Rust из Global Constraints → PASS.
- [ ] **Step 8: Коммит** `feat(core): отчёт по выданным заказам`.

### Task 2: API отчёта и файл Excel

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`: `rust_xlsxwriter`, `calamine`), `crates/mw-core/Cargo.toml` (`rust_xlsxwriter` в зависимостях, `calamine` в `[dev-dependencies]`), `apps/desktop/src-tauri/Cargo.lock`
- Create: `crates/mw-core/src/export/mod.rs` (`ExportFile`, `EXPORT_ROWS_MAX`, `ensure_rows`, семафор), `crates/mw-core/src/export/xlsx.rs` (запись таблиц), `crates/mw-core/src/export/report.rs` (Excel отчёта)
- Modify: `crates/mw-core/src/core.rs` (семафор выгрузок `Semaphore::new(2)`), `crates/mw-core/src/lib.rs`
- Modify: `crates/mw-domain/src/validation.rs` (`too_many_rows`, `nothing_to_export`), `crates/mw-core/tests/api_types.rs`
- Create: `crates/mw-api/src/routes/reports.rs`; Modify: `crates/mw-api/src/routes/mod.rs` (`mod reports;`, `#[inline(never)] fn report_routes()`, `.merge(report_routes())`), `crates/mw-api/src/dto.rs` (`OrderReportQuery`), `crates/mw-api/src/routes/headers.rs` (`attachment(name, ascii)`, перенос `percent_encode` из `routes/attachments.rs`, вложения пользуются им же)
- Test: `crates/mw-core/tests/report_xlsx.rs`, `crates/mw-api/tests/reports.rs`
- Modify: `crates/mw-api/openapi.json`, `crates/mw-api/bindings/*` (генерация)

**Interfaces:**
- Consumes: `Core::order_report`, `report_rows`, `ReportFilter`, `OrderReport` (задача 1).
- Produces (`mw-core::export`):
  - `pub struct ExportFile { pub name: String, pub ascii_name: &'static str, pub content_type: &'static str, pub bytes: Vec<u8> }`;
  - `pub const EXPORT_ROWS_MAX: usize = 50_000`;
  - `pub(crate) fn ensure_rows(count: usize, max: usize) -> Result<(), CoreError>` — `count > max` → `[("rows", "too_many_rows")]`;
  - `pub(crate) struct Sheet { name: String, columns: Vec<Column>, rows: Vec<Vec<Cell>>, total: Option<Vec<Cell>> }`;
  - `pub(crate) enum Cell { Empty, Text(String), Int(i64), Money(Money), Day(NaiveDate), Time(NaiveDateTime) }`;
  - `pub(crate) fn workbook(sheets: &[Sheet], currency: &str) -> Result<Vec<u8>, CoreError>`. Шапка жирная и закреплена, автофильтр по шапке, ширина колонок по содержимому. Деньги — число в формате `#,##0.00 "<знак валюты>"`, даты — `dd.mm.yyyy`, время — `dd.mm.yyyy hh:mm`. Строка итога жирная;
  - `Core::order_report_xlsx(&self, actor: &Actor, filter: &ReportFilter) -> Result<ExportFile, CoreError>` — `ReportsView`, под семафором, книга строится в `spawn_blocking`.
- Produces (API):
  - `GET /api/v1/reports/orders` (`OrderReportQuery`) → `OrderReport`;
  - `GET /api/v1/reports/orders/xlsx` (`OrderReportQuery`) → файл. Тег «Отчёты»;
  - `OrderReportQuery { from: Option<NaiveDate>, to: Option<NaiveDate>, service_point_id: Option<i64>, master_user_id: Option<i64>, device_type: Option<String>, brand: Option<String>, source: Option<String>, group_by: Option<ReportGroupBy> }` + `fn filter(&self) -> ReportFilter`;
  - пустые строки `device_type`/`brand`/`source` = нет фильтра.

Книга отчёта:
- **«Итоги»** — две колонки «Показатель» и «Значение»:
  - «Период» — «01.11.2026–30.11.2026» или «За всё время»;
  - строки заданных фильтров: «Точка», «Мастер», «Тип устройства», «Бренд», «Источник» — подписями, как в группах;
  - «Заказов», «Из них без ремонта», «Выручка», «Скидки», «Затраты», «Прибыль», «Оплата мастерам», «Процент мастера» («50 %»).
- **«По мастерам» / «По точкам» / «По типам» / «По брендам» / «По источникам»** (по `group_by`):
  - колонки: «Мастер» | «Точка» | «Тип устройства» | «Бренд» | «Источник», затем «Заказов», «Без ремонта», «Выручка», «Затраты», «Прибыль», «Оплата мастеру»;
  - строка итога — «Итого».
- **«Заказы»** — строки `report_rows`, не больше `EXPORT_ROWS_MAX`. Колонки:
  - «№ заказа», «Принят», «Выдан», «Дней в сервисе» (дни между датами выдачи и приёма);
  - «Клиент», «Телефон», «Тип устройства», «Бренд», «Модель», «Неисправность», «Выполненные работы», «Мастер», «Точка», «Источник»;
  - «Итог», «Скидка», «Затраты», «Прибыль», «Оплата мастеру», «Без ремонта» («да» или пусто).

- [ ] **Step 1: Добавить зависимости.**
  - `rust_xlsxwriter` и `calamine` — последние версии с crates.io, которым больше двух недель.
  - Проверить лицензии (`cargo deny check licenses`, если установлен; иначе `cargo metadata` и поле `license` всех новых крейтов) — только разрешённые.
  - Обновить `apps/desktop/src-tauri/Cargo.lock` через `cargo check` в `apps/desktop/src-tauri`.
- [ ] **Step 2: Тесты** (`crates/mw-core/tests/report_xlsx.rs`, по `all_backends()`; книга читается `calamine` из памяти):
  - `report_workbook_has_three_sheets`: данные как в `report_sums_issued_orders`.
    - Листы «Итоги», «По мастерам», «Заказы».
    - В «Итогах» строка «Выручка» = число 1400.
    - В «По мастерам» строка «Итого» = 2 заказа.
    - В «Заказах» 2 строки по порядку выдачи; «Без ремонта» у второй = «да».
    - Колонок пароля и графического ключа нет: ни одна ячейка шапки не содержит «Пароль» или «ключ».
  - `rows_over_the_limit_are_refused`: `ensure_rows(3, 2)` → `[("rows", "too_many_rows")]`, `ensure_rows(2, 2)` → `Ok`.
  - `report_xlsx_needs_reports_view`.
- [ ] **Step 3: Тесты API** (`crates/mw-api/tests/reports.rs`):
  - `report_endpoint_returns_totals`: `GET /api/v1/reports/orders?brand=<бренд>` → 200, `totals.orders == 2`. Без права → 403. `from=2026-03-12&to=2026-03-11` → 422 с `period_reversed`.
  - `report_xlsx_has_a_russian_file_name`:
    - `GET …/reports/orders/xlsx?from=2026-11-01&to=2026-11-30` → 200;
    - тип xlsx;
    - `Content-Disposition` начинается с `attachment; filename="report.xlsx"; filename*=UTF-8''`, а раскодированный `filename*` равен «Отчёт по заказам 01.11.2026–30.11.2026.xlsx» (тире — U+2013);
    - `Cache-Control: private, no-store`;
    - тело начинается с `PK`.
  - `unknown_query_fields_are_refused`: `?brandd=x` → 400 (как у других `Query` с `deny_unknown_fields`).
- [ ] **Step 4: Запустить** `cargo test -p mw-core --test report_xlsx && cargo test -p mw-api --test reports` → не собирается.
- [ ] **Step 5: Реализовать.**
  - `export/`: `ExportFile`, `ensure_rows`, `Sheet`/`Cell`, `workbook`, `order_report_xlsx`.
  - Маршруты в `report_routes`.
  - `attachment(...)` в `headers.rs`; вложения пользуются им же, их тесты зелёные.
  - Если `fileNameOf` в `apps/ui/src/lib/api/client.ts` не читает `filename*`, поправить и покрыть тестом в `client.test.ts` (нужно задачам 4–7).
- [ ] **Step 6: Запустить** тесты задачи → PASS. Обновить контракт. Полный Rust → PASS. `cd apps/desktop/src-tauri && cargo clippy --all-targets -- -D warnings && cargo test` → PASS. `cd apps/desktop && npm run licenses` → PASS.
- [ ] **Step 7: Коммит** `feat(api): отчёт по заказам и его файл Excel`.

### Task 3: Графики и выбор периода в `#lib/ui`

**Files:**
- Modify: `apps/ui/package.json`, `apps/ui/package-lock.json` (`echarts` в `dependencies`)
- Create: `apps/ui/src/lib/ui/chart.ts` (построение опций ECharts — чистая функция), `apps/ui/src/lib/ui/Chart.svelte`
- Create: `apps/ui/src/lib/ui/period.ts` (`PeriodKind`, `periodRange`), `apps/ui/src/lib/ui/PeriodPicker.svelte`
- Modify: `apps/ui/src/lib/ui/index.ts` (экспорт `Chart`, `PeriodPicker`), `apps/ui/src/lib/design/tokens.css` (`--chart-1`, `--chart-2`, `--chart-3` для светлой и тёмной темы), `apps/ui/src/lib/i18n/ru.ts` (`ru.period.*`)
- Create: `apps/ui/src/routes/design/sections/Charts.svelte`; Modify: `apps/ui/src/routes/design/+page.svelte` (раздел «Графики и период»)
- Test: `apps/ui/src/lib/ui/chart.test.ts`, `apps/ui/src/lib/ui/period.test.ts`, `apps/ui/src/lib/ui/PeriodPicker.test.ts`

**Interfaces:**
- Produces:
  - `type ChartSeries = { name: string; values: number[]; kind: 'bar' | 'line'; tone: 1 | 2 | 3 }`;
  - `function chartOption(input: { categories: string[]; series: ChartSeries[]; horizontal?: boolean; format: (value: number) => string }, colors: Record<1 | 2 | 3, string>, text: { color: string; muted: string; grid: string }): EChartsCoreOption`;
  - `Chart.svelte` — пропсы `label: string` (`aria-label` у `<figure role="img">`), `categories`, `series`, `horizontal?`, `format`, `height?: number` (по умолчанию 280).
    - `echarts/core` с `BarChart`, `LineChart`, `GridComponent`, `TooltipComponent`, `LegendComponent` и `SVGRenderer`, без полного `echarts`.
    - Цвета читаются из `--chart-1..3` и текстовых токенов через `getComputedStyle` и перечитываются при смене темы.
    - Размер — по `ResizeObserver`; `dispose` при уничтожении.
  - `type PeriodKind = 'today' | 'week' | 'month' | 'last_month' | 'year' | 'all' | 'custom'`;
  - `function periodRange(kind: Exclude<PeriodKind, 'custom'>, today: string): [string | null, string | null]` — ISO-дни, решение 6;
  - `PeriodPicker.svelte` — пропсы `kind: PeriodKind`, `from: string | null`, `to: string | null`, `onchange(kind, from, to)`.
    - Чипы быстрых периодов — подписи `ru.period`: «Сегодня», «Неделя», «Месяц», «Прошлый месяц», «Год», «Всё время», «Свой период».
    - При «Свой период» — два `DateInput` «С» и «По». «По» раньше «С» подсвечено ошибкой «Начало периода позже конца».

- [ ] **Step 1: Добавить `echarts`** (последняя версия, которой больше двух недель). `npm run licenses` → PASS (ECharts — Apache-2.0, `zrender` — BSD-3-Clause, `tslib` — 0BSD).
- [ ] **Step 2: Тесты:**
  - `period.test.ts` — `periodRange`:
    - `('week', '2026-11-25')` (среда) → `['2026-11-23', '2026-11-25']`;
    - `('week', '2026-11-23')` (понедельник) → `['2026-11-23', '2026-11-23']`;
    - `('month', '2026-11-25')` → `['2026-11-01', '2026-11-25']`;
    - `('last_month', '2026-03-15')` → `['2026-02-01', '2026-02-28']`;
    - `('last_month', '2028-03-10')` → `['2028-02-01', '2028-02-29']`;
    - `('last_month', '2026-01-05')` → `['2025-12-01', '2025-12-31']`;
    - `('year', '2026-11-25')` → `['2026-01-01', '2026-11-25']`;
    - `('today', d)` → `[d, d]`;
    - `('all', d)` → `[null, null]`.
  - `chart.test.ts`:
    - серия `bar` и серия `line` дают ECharts-серии этих типов с цветами `colors[tone]`;
    - `horizontal: true` меняет оси (категории по оси Y);
    - подсказка по наведению показывает значения через `format`;
    - в опции нет ни одного литерала цвета, кроме переданных.
  - `PeriodPicker.test.ts`:
    - щелчок по «Прошлый месяц» зовёт `onchange('last_month', null, null)`;
    - «Свой период» показывает поля «С» и «По»;
    - выбранный вариант отмечен выбранным для чтения с экрана (роль и состояние — те, что даёт компонент выбора).
  - Быстрые периоды строятся на том из `ChipGroup` и `SegmentedControl`, что даёт одиночный выбор с переносом на новую строку.
- [ ] **Step 3: Запустить** `cd apps/ui && npx vitest run src/lib/ui/period.test.ts src/lib/ui/chart.test.ts src/lib/ui/PeriodPicker.test.ts` → FAIL.
- [ ] **Step 4: Реализовать.**
  - Токены `--chart-1` (выручка), `--chart-2` (затраты), `--chart-3` (прибыль) — в обеих темах, с достаточным контрастом к фону.
  - Витрина: столбики с линией и горизонтальные столбики на вымышленных данных, `PeriodPicker`.
- [ ] **Step 5: Запустить** `cd apps/ui && npm run verify` → PASS: `no-raw-colors`, `no-width-media`, проверка размера сборки, если есть.
- [ ] **Step 6: Коммит** `feat(ui): графики ECharts и выбор периода`.

### Task 4: Экран «Отчёты»

**Files:**
- Create: `apps/ui/src/routes/(app)/reports/+page.svelte`
- Create: `apps/ui/src/lib/reports/view.ts` (вид в адресе и запрос к API), `apps/ui/src/lib/reports/OrdersReport.svelte`, `apps/ui/src/lib/reports/ReportFilters.svelte`
- Create: `apps/ui/src/lib/api/reports.ts`
- Modify: `apps/ui/src/lib/app/AppSidebar.svelte` (пункт «Отчёты» после «Склада»), `apps/ui/src/lib/i18n/ru.ts` (`ru.app.reports`, `ru.reports.*`; вторая строка `ru.help.general.masterPercent`)
- Test: `apps/ui/src/lib/reports/view.test.ts`, `apps/ui/src/lib/reports/OrdersReport.test.ts`, `apps/ui/e2e/reports.spec.ts`

**Interfaces:**
- Consumes: `GET /api/v1/reports/orders[/xlsx]`, типы `$api/OrderReport`, `$api/ReportGroupBy`, `$api/OrderReportQuery` (задача 2); `Chart`, `PeriodPicker`, `periodRange` (задача 3).
- Produces:
  - `type ReportView = { tab: 'orders' | 'moves'; period: PeriodKind; from: string | null; to: string | null; groupBy: ReportGroupBy; masterUserId?: number; servicePointId?: number; deviceType?: string; brand?: string; source?: string }`;
  - `reportViewFromSearch(params: URLSearchParams): ReportView` и `reportViewToSearch(view: ReportView): string`. Параметры:
    - `tab` — только `moves`;
    - `period` — по умолчанию `month`, не пишется;
    - `from` и `to` — только при `custom`;
    - `by` — `point` | `type` | `brand` | `source`, по умолчанию мастер;
    - `m`, `p` — положительные числа;
    - `t`, `b`, `s` — текст;
    - неверные значения отбрасываются;
  - `reportQuery(view: ReportView, today: string): OrderReportQuery`;
  - `orderReport(query, signal)`, `downloadOrderReport(query)` в `#lib/api/reports.ts`.

Экран (вкладка «Заказы»):
- **Верх:** `PeriodPicker`, фильтры «Мастер» (мастера), «Точка» (живые точки), «Тип устройства» и «Бренд» (подсказки — тот же источник, что у фильтров списка заказов), «Источник» (справочник «Откуда узнали»), «Группировать»: «По мастерам», «По точкам», «По типам», «По брендам», «По источникам». Кнопка «В Excel» (`FileSpreadsheet`).
- **Плитки `StatTile`:**
  - «Заказов» — «без ремонта: N»;
  - «Выручка» — «скидки: X»;
  - «Затраты» — «расходы и запчасти»;
  - «Прибыль» — «в среднем X с заказа»;
  - «Оплата мастерам» — «N % прибыли».
- **Графики:**
  - «По времени»: столбики «Выручка» и «Затраты», линия «Прибыль». Подписи — «25.11» по дням или «ноя 2026» по месяцам, через `#lib/format/date.js`;
  - «По мастерам» (по разрезу): горизонтальные столбики «Выручка», «Затраты», «Прибыль» первых 10 групп, остальные — одной группой «Остальные».
- **Таблица** по группам: подпись, «Заказов», «Без ремонта», «Выручка», «Затраты», «Прибыль», «Мастеру», последняя строка «Итого». На узком экране таблица листается внутри своего блока, страница не листается вбок.
- **Пусто:** «За этот период выданных заказов нет». Пояснение у плитки «Заказов»: «Считаются заказы, выданные в этот период. Без ремонта — у кого в «Выполненных работах» стоит «Без ремонта»».
- **Данные:** запрос при каждом изменении вида; прежний запрос отменяется (`AbortController`). Ошибка сервера показывается над плитками, ошибка периода — у поля «По».
- **Боковая панель:** «Отчёты» (`ChartColumn`) видна при `app.can('reports_view')`. Без права адрес `/reports` пишет «Отчёты может смотреть пользователь с правом «Отчёты»».
- **Подсказка процента мастера**, вторая строка: «Оплата мастеру в «Отчётах» — этот процент от прибыли заказа: итог − скидка − затраты».

- [ ] **Step 1: Тесты:**
  - `view.test.ts`:
    - вид по умолчанию ↔ пустая строка;
    - `?period=custom&from=2026-11-01&to=2026-11-30&by=source&p=3&b=Apple` туда и обратно;
    - `?period=custom` без дат ↔ `custom` с `null`;
    - `?by=bad&m=-1` → по умолчанию;
    - `reportQuery` с `period: 'month'` и `today = '2026-11-25'` → `from: '2026-11-01'`, `to: '2026-11-25'`, `group_by: 'master'`.
  - `OrdersReport.test.ts` (ответ API — фикстура):
    - плитки показывают суммы через `formatMoney`;
    - таблица — группы и строку «Итого»;
    - пустой ответ — текст пустого отчёта;
    - 12 групп → в графике 11 категорий, последняя «Остальные».
- [ ] **Step 2: Запустить** `npx vitest run src/lib/reports` → FAIL.
- [ ] **Step 3: Реализовать** экран, пункт панели, строки `ru.ts`.
- [ ] **Step 4: Сквозной тест** `e2e/reports.spec.ts`. Данные через API, бренд `uniq('Бренд')`: заказы А, Б, В как в `report_sums_issued_orders`, мастер — новый пользователь-мастер.
  - Открыть `/orders`, перейти в «Отчёты» из боковой панели. Выбрать «Всё время», ввести бренд в фильтр «Бренд».
  - Плитки: «Заказов» 2, «без ремонта: 1», «Выручка» 1 400 ₽, «Затраты» 300 ₽. «Оплата мастерам» — от `master_percent` из ответа API (другие тесты могут менять настройку).
  - Таблица: строки мастера, «Без мастера» и «Итого».
  - «Группировать» → «По брендам»: одна строка бренда.
  - Адрес содержит `period=all` и `b=`; перезагрузка показывает тот же вид.
  - «В Excel» → загрузка с `suggestedFilename()` «Отчёт по заказам за всё время.xlsx», файл не пустой.
- [ ] **Step 5: Запустить** `cd apps/ui && npm run verify && npm run test:e2e` → PASS.
- [ ] **Step 6: Коммит** `feat(ui): экран «Отчёты»`.

### Task 5: Перемещения между точками

**Files:**
- Create: `crates/mw-core/src/reports/moves.rs`; Modify: `crates/mw-core/src/reports/mod.rs`, `crates/mw-core/src/lib.rs`
- Modify: `crates/mw-api/src/routes/reports.rs`, `crates/mw-api/src/routes/mod.rs` (`report_routes`), `crates/mw-api/src/dto.rs` (`MovesReportQuery`)
- Create: `apps/ui/src/lib/reports/MovesReport.svelte`; Modify: `apps/ui/src/routes/(app)/reports/+page.svelte` (вкладки «Заказы» и «Перемещения»), `apps/ui/src/lib/api/reports.ts`, `apps/ui/src/lib/i18n/ru.ts`
- Test: `crates/mw-core/tests/reports_moves.rs`, `crates/mw-api/tests/reports.rs`, `apps/ui/src/lib/reports/MovesReport.test.ts`, `apps/ui/e2e/reports.spec.ts`
- Modify: `crates/mw-api/openapi.json`, `bindings/*`

**Interfaces:**
- Consumes: записи `audit_log` с `action = "order.moved"` и `details = {"from": i64|null, "to": i64|null}` (план 8.2, `crates/mw-core/src/orders/update.rs:470-487`); `ReportView.tab` (задача 4).
- Produces:
  - `struct MovesFilter { from: Option<NaiveDate>, to: Option<NaiveDate>, service_point_id: Option<i64> }` — точка как «откуда» или «куда»;
  - `struct MoveRoute { from: Option<i64>, from_name: String, to: Option<i64>, to_name: String, count: u64 }` — по `count`, большие сверху;
  - `struct MoveItem { at: NaiveDateTime, order_id: i64, user_name: String, from: Option<i64>, from_name: String, to: Option<i64>, to_name: String }`;
  - `struct MovesReport { routes: Vec<MoveRoute>, items: Vec<MoveItem>, total: u64 }` — `items` до 500, новые сверху;
  - `Core::moves_report(&self, actor: &Actor, filter: &MovesFilter) -> Result<MovesReport, CoreError>` — требует `ReportsView`, `period_reversed` как в отчёте;
  - имена точек — текущие названия, архивная « (в архиве)», `null` — «Без точки», id без точки — «Точка № <id> (удалена)»;
  - запись с повреждёнными `details` пропускается с `warn`, как в `list_audit`;
  - `GET /api/v1/reports/moves` (`MovesReportQuery { from, to, service_point_id }`) → `MovesReport`.

- [ ] **Step 1: Тесты ядра** (`reports_moves.rs`, по `all_backends()`):
  - `moves_report_counts_routes`. Точки А, Б, В; заказ 1: А → Б → В; заказ 2: А → Б.
    - `routes` = `[(А, Б, 2), (Б, В, 1)]`, `total 3`, `items` новые сверху.
    - Фильтр точки В → только `(Б, В, 1)`.
  - `moves_report_names_archived_deleted_and_missing_points`: переезд «без точки» → А, затем А архивирована. Подписи «Без точки», «А (в архиве)». Запись с id несуществующей точки (вставка журнала в тесте) → «Точка № <id> (удалена)».
  - `moves_report_needs_reports_view`; `moves_period_reversed`.
- [ ] **Step 2: Тест API** `moves_endpoint`: 200 и `total`; 403 без права.
- [ ] **Step 3: Тест интерфейса** `MovesReport.test.ts`:
  - таблица «Откуда → Куда · Сколько раз»;
  - список «Когда», «Заказ», «Откуда», «Куда», «Кто»;
  - `total > items.length` → «Показаны последние 500 из N»;
  - пусто → «За этот период заказы между точками не перемещались».
- [ ] **Step 4: Запустить** новые тесты → FAIL.
- [ ] **Step 5: Реализовать.**
  - Вкладки `Tabs` «Заказы» и «Перемещения». Вкладка «Перемещения» берёт период вида и фильтр «Точка», остальные фильтры скрыты.
  - Номер заказа — та же ссылка, что у номеров в отчёте импорта (`ReportView.svelte`): открывает панель заказа.
- [ ] **Step 6: Сквозной тест** в `reports.spec.ts`:
  - через API две точки `uniq('Точка')`, заказ в первой, затем смена точки на вторую;
  - «Отчёты» → «Перемещения» → «Всё время», точка — первая;
  - строка «<первая> → <вторая>» с «1», в списке номер заказа;
  - щелчок открывает панель заказа;
  - адрес содержит `tab=moves`.
- [ ] **Step 7: Запустить** полный Rust, `npm run verify && npm run test:e2e` → PASS. Обновить контракт.
- [ ] **Step 8: Коммит** `feat: отчёт о перемещениях заказов между точками`.

### Task 6: Право «Выгрузка» и выгрузка заказов и клиентов в Excel

**Files:**
- Modify: `crates/mw-domain/src/permission.rs`:
  - `DataExport` после `ReportsView`;
  - `ALL`, `label_ru` «Выгрузка заказов, клиентов и контактов»;
  - тест числа прав → 18.
- Create: `crates/mw-db/src/migration/m20261125_000003_data_export_permission.rs` (+ регистрация). Миграция дописывает `"data_export"` каждой роли, чьи права как множество содержат все 17 прав замороженного литерала (копия `SEED_ADMIN_PERMISSIONS` из `m20261111_000001_init.rs` в самой миграции).
- Modify: `crates/mw-legacy/src/convert/access.rs` (`settings` → `+ Permission::DataExport`, тест)
- Modify: `apps/ui/src/lib/settings/permissions.ts` (группа «Отчёты»: `reports_view`, `data_export`), `apps/ui/src/lib/settings/permissions.test.ts`, `ru.ts`
- Create: `crates/mw-core/src/export/orders.rs`, `crates/mw-core/src/export/clients.rs`
- Create: `crates/mw-api/src/routes/exports.rs`; Modify: `crates/mw-api/src/routes/mod.rs` (`#[inline(never)] fn export_routes()`), `crates/mw-api/src/dto.rs` (`OrderExportQuery`, `ClientExportQuery`)
- Create: `apps/ui/src/lib/api/exports.ts`; Modify: страница списка заказов `apps/ui/src/routes/(app)/orders/+page.svelte` (или её панель инструментов), страница клиентов `apps/ui/src/routes/(app)/clients/+page.svelte`
- Test: тест миграции (`all_backends_up_to`) — там же, где тест `m20261124_000002_stock_data`; `crates/mw-core/tests/exports.rs`, `crates/mw-api/tests/exports.rs`, компонентный тест кнопок, `apps/ui/e2e/exports.spec.ts`
- Modify: `crates/mw-api/openapi.json`, `bindings/*`

**Interfaces:**
- Consumes: `ExportFile`, `Sheet`, `Cell`, `workbook`, `ensure_rows`, семафор, `attachment(...)` (задача 2).
- Produces:
  - `Permission::DataExport` (`"data_export"`);
  - `Core::export_orders_xlsx(&self, actor: &Actor, filter: &OrderFilter, sort: <тип порядка Core::list_orders>) -> Result<ExportFile, CoreError>` — требует `DataExport`. Строки и порядок — как у `Core::list_orders` с тем же фильтром и порядком, без постраничности. Условия — общая функция списка, не копия;
  - `Core::export_clients_xlsx(&self, actor: &Actor, filter: &ClientFilter) -> Result<ExportFile, CoreError>` — требует `ClientsView` и `DataExport`, строки и порядок — как у `Core::list_clients`;
  - `GET /api/v1/exports/orders` (`OrderExportQuery` — поля фильтра и порядка `OrderListQuery` без `limit`/`offset`) → «Заказы <дата>.xlsx»;
  - `GET /api/v1/exports/clients` (`ClientExportQuery` — поля `ClientListQuery` без `limit`/`offset`) → «Клиенты <дата>.xlsx». Тег «Выгрузки»;
  - в интерфейсе — `downloadOrders(query)`, `downloadClients(query)`; запрос строится той же функцией, что запрос списка.

Колонки листа «Заказы»:
- «№ заказа», «Принят», «Статус», «Выдан», «Точка», «Клиент», «Телефон» (основной, `format_phone`);
- «Тип устройства», «Бренд», «Модель», «Серийный номер», «Неисправность», «Выполненные работы», «Мастер»;
- «Оценка от», «Оценка до», «Итог», «Скидка», «Предоплата», «Расходы», «Запчасти» (Σ цена × количество), «К оплате» (итог − предоплата − скидка; без итога — пусто);
- «Гарантия» — текст гарантии, как в актах;
- «Штрихкод».

Колонки листа «Клиенты»:
- «№ клиента», «ФИО», «Телефоны» (все, `format_phone`, через «, »);
- «Адрес», «Откуда узнал», «Чёрный список» («да» или пусто), «Заказов», «Примечание», «Добавлен».

- [ ] **Step 1: Тесты права:**
  - `permission.rs`: 18 прав, `label_ru(DataExport)`;
  - тест миграции:
    - роль «Администратор» из начальных данных получает `data_export`;
    - роль с 17 правами под другим именем — тоже;
    - «Мастер», «Приёмщик» и роль без `audit_view` — нет;
  - `access.rs`: группа с `settings = 1` получает `DataExport`;
  - `permissions.test.ts`: группа «Отчёты» = `reports_view`, `data_export`.
- [ ] **Step 2: Тесты ядра** (`exports.rs`, по `all_backends()`, книга читается `calamine`):
  - `orders_export_follows_the_list_filter`:
    - три заказа с `uniq` брендом, у одного пароль «1234» и графический ключ;
    - выгрузка с фильтром бренда → 3 строки в порядке `list_orders`;
    - колонки как выше;
    - ни в одной ячейке нет «1234» и нет колонок «Пароль» или «Графический ключ»;
    - «К оплате» = итог − предоплата − скидка;
  - `clients_export_follows_the_search`:
    - клиент с двумя телефонами → «+7 921 123-45-67, +7 921 765-43-21»;
    - поиск по `uniq` имени → 1 строка;
  - `exports_need_data_export`:
    - без `DataExport` → `Forbidden(DataExport)`;
    - клиенты без `ClientsView` → `Forbidden(ClientsView)`.
- [ ] **Step 3: Тесты API** (`crates/mw-api/tests/exports.rs`):
  - `GET /api/v1/exports/orders?brand=…` → 200, xlsx, `filename*` с «Заказы»;
  - `?limit=10` → 400;
  - без права → 403.
- [ ] **Step 4: Запустить** новые тесты → FAIL.
- [ ] **Step 5: Реализовать** право, миграцию, импорт, выгрузки, маршруты в `export_routes`.
  - Интерфейс: кнопка «В Excel» (`FileSpreadsheet`) на панели списка заказов при `app.can('data_export')` — выгружает текущий фильтр и порядок.
  - На странице клиентов меню «Выгрузить» с пунктом «В Excel» при `data_export`; пункт «Контакты для телефона…» добавит задача 7.
  - Ошибка сервера — тостом с текстом сервера.
- [ ] **Step 6: Тесты интерфейса:**
  - компонентный: без `data_export` кнопки «В Excel» нет, с правом — есть;
  - `e2e/exports.spec.ts`: заказы с `uniq` брендом → поиск или фильтр в списке → «В Excel» → `suggestedFilename()` начинается с «Заказы » и кончается «.xlsx», файл не пустой. То же для клиентов через «Выгрузить» → «В Excel».
- [ ] **Step 7: Запустить** полный Rust, `npm run verify && npm run test:e2e` → PASS. Обновить контракт.
- [ ] **Step 8: Коммит** `feat: право «Выгрузка» и выгрузка заказов и клиентов в Excel`.

### Task 7: Контакты для телефона и документы плана

**Files:**
- Create: `crates/mw-domain/src/contacts.rs` (+ `pub mod contacts;`)
- Create: `crates/mw-core/src/export/contacts.rs`
- Modify: `crates/mw-api/src/routes/exports.rs`, `crates/mw-api/src/dto.rs` (`ContactsExportQuery`)
- Create: `apps/ui/src/lib/clients/ContactsExportDialog.svelte`; Modify: `apps/ui/src/routes/(app)/clients/+page.svelte` (пункт «Контакты для телефона…»), `apps/ui/src/lib/api/exports.ts`, `ru.ts`
- Modify: `docs/superpowers/plans/README.md`, этот план (все `- [ ]` → `- [x]`)
- Test: `crates/mw-domain/src/contacts.rs` (модульные), `crates/mw-core/tests/exports.rs`, `crates/mw-api/tests/exports.rs`, `apps/ui/src/lib/clients/ContactsExportDialog.test.ts`, `apps/ui/e2e/exports.spec.ts`
- Modify: `crates/mw-api/openapi.json`, `bindings/*`

**Interfaces:**
- Consumes: `ExportFile`, `attachment`, `Permission::DataExport`, меню «Выгрузить» (задача 6).
- Produces (`mw-domain::contacts`):
  - `pub struct Contact { pub name: String, pub phones: Vec<String> }`;
  - `pub fn contact_phone(stored: &str) -> Option<String>` — решение 10;
  - `pub fn contacts_csv(contacts: &[Contact]) -> String`:
    - шапка `Name,Given Name,Group Membership,Phone 1 - Type,Phone 1 - Value` и далее пары до наибольшего числа телефонов;
    - строка: имя, имя, `* myContacts`, `Mobile`, номер…;
    - CRLF, кавычки по RFC 4180 для `,` `"` CR LF;
  - `pub fn contacts_vcard(contacts: &[Contact]) -> String` — на контакт `BEGIN:VCARD`, `VERSION:3.0`, `FN:<имя>`, `N:;<имя>;;;`, `TEL;TYPE=CELL:<номер>` на каждый номер, `END:VCARD`; экранирование и перенос строк по решению 10.
- Produces (`mw-core`, API):
  - `enum ContactsFormat { Csv, Vcard }` (`snake_case`, `TS`, `ToSchema`);
  - `Core::export_contacts(&self, actor: &Actor, filter: &ClientFilter, created_from: Option<NaiveDate>, format: ContactsFormat) -> Result<ExportFile, CoreError>`:
    - требует `ClientsView` и `DataExport`;
    - клиенты и порядок — как у `list_clients`; `created_from` — `clients.created_at >= с 00:00`;
    - повторный номер (по `phone_key`) у следующего клиента выбрасывается; клиент без номеров пропускается;
    - ничего не осталось → `[("rows", "nothing_to_export")]`;
    - больше `EXPORT_ROWS_MAX` клиентов → `too_many_rows`;
  - `GET /api/v1/exports/contacts` (`ContactsExportQuery` = поля `ClientExportQuery` + `format: ContactsFormat` + `created_from: Option<NaiveDate>`) → «Контакты <дата>.csv» или «.vcf».

Диалог «Контакты для телефона»:
- «Формат»: «Google Контакты (CSV)» или «vCard (iPhone, Android)».
- «Клиенты»: «Все из списка» или «Добавленные с» + `DateInput`.
- Кнопка «Выгрузить», Ctrl/⌘+Enter — `submitShortcut`.
- После успешной выгрузки сегодняшний день запоминается в `localStorage` под ключом `mw.contactsExportedOn` (чтение и запись — в `try/catch`). Если день сохранён, диалог открывается с «Добавленные с <этот день>», иначе — «Все из списка».
- Ошибка сервера (`nothing_to_export` и др.) — под формой, текстом сервера.

- [ ] **Step 1: Тесты домена** (`contacts.rs`):
  - `contact_phones_for_a_phone_book`:
    - «8 (921) 123-45-67» → `+79211234567`;
    - «9211234567» → `+79211234567`;
    - «+7 921 123 45 67» → `+79211234567`;
    - «123-45-67» → `1234567`;
    - «12» → `None`;
  - `csv_quotes_what_needs_quoting`: имя `Иванов, "Ваня"` → поле `"Иванов, ""Ваня"""`; строки через `\r\n`; два контакта с 1 и 2 номерами → шапка до `Phone 2 - Value`, у первого пустые поля второго номера;
  - `vcard_escapes_and_folds`:
    - имя `Петров; ООО, "Север"\nсклад` → `FN:Петров\; ООО\, "Север"\nсклад`;
    - строка длиннее 75 байт кириллицей переносится на `\r\n ` без разрыва символа UTF-8;
    - каждая строка кончается `\r\n`.
- [ ] **Step 2: Тесты ядра** (`exports.rs`):
  - `a_phone_goes_to_the_first_client_only`: два клиента с одним номером → номер только у первого; второй без других номеров пропущен;
  - `contacts_since_a_day`: клиент, добавленный вчера, и сегодняшний; `created_from = сегодня` → только второй;
  - `no_contacts_is_an_error`: клиент без телефонов → `nothing_to_export`;
  - `contacts_need_data_export`.
- [ ] **Step 3: Тест API**: `GET /api/v1/exports/contacts?format=vcard&query=<uniq>` → 200, `text/vcard; charset=utf-8`, тело начинается с `BEGIN:VCARD\r\n`, `filename*` с «Контакты»; `format=pdf` → 400.
- [ ] **Step 4: Запустить** новые тесты → FAIL.
- [ ] **Step 5: Реализовать** домен, ядро, маршрут, диалог.
- [ ] **Step 6: Тесты интерфейса:**
  - `ContactsExportDialog.test.ts`:
    - без сохранённого дня — «Все из списка»;
    - с `mw.contactsExportedOn = '2026-11-20'` — «Добавленные с» и дата 20.11.2026;
    - `localStorage` бросает исключение → диалог работает;
  - `e2e/exports.spec.ts`:
    - клиент `uniq('Контакт')` с телефоном через API;
    - «Клиенты» → поиск → «Выгрузить» → «Контакты для телефона…» → «Google Контакты (CSV)» → «Выгрузить»;
    - `suggestedFilename()` кончается «.csv»;
    - текст файла содержит имя и `+7…`.
    - То же для vCard (`BEGIN:VCARD`).
- [ ] **Step 7: Документы:**
  - `README.md`:
    - строка 8.4 — «выполнен»;
    - новая строка `| 8.7 | Платежи по заказу | несколько платежей: день, сумма и кто принял; «К оплате» в актах и карточке по сумме платежей; перенос предоплаты | 8.4 | — |`, добавленная в коммите плана, остаётся;
    - заметку «несколько платежей по заказу…» перенести из «План 8.4» в новую «План 8.7 (платежи по заказу)»;
    - пометить сделанными «отчёты по источнику — без учёта регистра» и «подсказка о проценте мастера»;
    - идея «Перемещение заказа между точками» → `[x]` с «Сделано в плане 8.4: отчёт «Перемещения»; статуса «в пути» нет — переезд мгновенный, свой этап можно завести статусом вида «в работе»»;
  - в этом плане все `- [ ]` → `- [x]`.
- [ ] **Step 8: Запустить** полный Rust, `npm run verify && npm run test:e2e` → PASS. Обновить контракт.
- [ ] **Step 9: Коммит** `feat: контакты для телефона (CSV и vCard); план 8.4 выполнен`.

## Самопроверка

- **Спецификация:**
  - раздел 7: период и фильтры мастер/точка/тип/бренд/источник — задачи 1, 4; число, выручка, затраты, без ремонта, оплата мастерам — задача 1; отчёт в Excel — задача 2;
  - раздел 8: заказы и клиенты в Excel — задача 6; контакты CSV — задача 7, vCard из строки дорожной карты — тоже задача 7;
  - экран из спецификации интерфейса: период и фильтры, плитки, графики, таблица, «В Excel» — задачи 3, 4;
  - заметки README: источник без учёта регистра — задача 1; подсказка процента — задача 4; перемещения и «в пути» — задача 5 и решение 7; несколько платежей — решение 11, план 8.7.
- **Типы:** `ReportFilter`/`OrderReport` (задача 1) → `OrderReportQuery` (задача 2) → `$api/...` (задачи 4, 5). `ExportFile`/`Sheet`/`Cell`/`workbook` (задача 2) → задачи 6, 7. `Permission::DataExport` (задача 6) → задача 7.
- **Review Focus:** 1–3 — тесты задачи 1; 4 — задача 2 и сквозные задач 4, 6, 7; 5 — задача 7.
- **Что не покрыто намеренно:** платежи (план 8.7); мобильная раскладка отчётов (план 10, здесь — только без бокового листания страницы); шаблоны отчётов и печать отчёта.
