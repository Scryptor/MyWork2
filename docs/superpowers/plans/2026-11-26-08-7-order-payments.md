# План 8.7. Платежи по заказу

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** у заказа есть список платежей: день, сумма и кто принял. Карточка и панель показывают «Оплачено» и «Остаток» по этому списку. «Выдать» сразу принимает остаток. Акты печатают то же, что раньше: «Предоплата» — внесённое до выдачи, «К оплате» — сколько клиент платит при выдаче. Прежние предоплаты стали платежами.

**Architecture:**
- Платёж — строка `order_payments`: заказ, день (дата без времени), сумма, кто принял (текст), этап, кто и когда записал, `legacy`, ключ повтора. Этапы и пределы — `mw_domain::payment`.
- `orders.prepayment` остаётся сохранённым итогом: это сумма платежей `before_issue`. Он пересчитывается в той же транзакции `begin_write()`, что и запись платежа. Поэтому формула «К оплате» в актах, списке, Excel и интерфейсе не меняется.
- Ядро — `mw-core/src/orders/payments.rs`. Список, приём и удаление устроены как запчасти заказа (`stock/parts.rs`): версия заказа не меняется, повтор запроса — по `request_key`, у удалённого заказа платежи только читаются.
- Выдача с оплатой — `Core::issue_order_with` и тело `IssueRequest` у `POST /orders/{id}/issue`. Платёж `at_issue` пишется в транзакции выдачи. При каждом переходе между видом «выдан» и другим видом все платежи заказа становятся `before_issue`.
- Перенос: миграция данных `m20261126_000002_order_payments_from_prepayment` и импорт MyWork 2 создают по одному платежу `legacy` из предоплаты.
- API — раздел `order_payment_routes()`: 3 новые операции (128 → 131).
- Интерфейс:
  - блок «Платежи» в панели и карточке вместо поля «Предоплата»;
  - кнопка «Принять оплату…» в блоке и её диалог «Оплата по заказу № N»;
  - в «Выдать» — переключатель «Принять оплату» и рядом поле «Сумма оплаты»;
  - скрытые столбцы «Оплачено» и «Остаток» в списке;
  - поля документов `{заказ.оплачено}` и `{заказ.остаток}`.

**Tech Stack:** Rust (mw-domain, mw-db/SeaORM + SeaQuery-миграции, mw-core, mw-docs, mw-api/Axum + utoipa + ts-rs), SvelteKit 3 + Svelte 5 (руны), Vitest, Playwright. Новых зависимостей нет.

**Spec:** `docs/superpowers/specs/2026-11-25-order-payments-design.md` — проект решения. Он кладётся в репозиторий вместе с этим планом (`docs/superpowers/plans/2026-11-26-08-7-order-payments.md`) отдельным коммитом до задачи 1. Проект решения кладётся уже согласованным с планом: правок в этом коммите нет. Согласованы:
- решения 1–5, 7 и 10 (день, этапы, «кто принял», поля правки, выдача, права, `prepaid_at` не пишется и импортом);
- журнал приёма в решении 8.

Проект опирается на:
- `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` §5.1 (в акте выдачи «К оплате» выделено жирным);
- `docs/superpowers/plans/2026-11-24-08-1-stock.md` (запчасти заказа — образец таблицы у заказа, которая не меняет его версию).

## Решения плана

В проекте решения этого нет или там написано иначе; решено здесь.

1. **Права.** Права `orders_view` в программе нет.
   - Список платежей читает любой вошедший, как сам заказ (`get_order`) и его запчасти.
   - Добавить и удалить платёж — `orders_edit`.
   - Предоплата при приёме — `orders_create`.
   - Выдача с оплатой — `orders_edit`: отдельного права выдачи нет, `issue_order` требует его.
2. **«Кто принял» без поля** (приём, выдача).
   - Сервер пишет `actor.name()`, если у `actor` есть `user_id()`, иначе пусто: режим без входа («Без входа»), импорт, консоль. Помощник — `received_by_of(actor)`.
   - В диалоге «Принять оплату…» поле есть. По умолчанию — `app.me.open_mode ? '' : app.me.name`. Подсказки — активные мастера (`listMasters`), свой текст разрешён.
   - `added_by_name` — снимок `actor.name()`, как у запчастей. `added_by_id` без внешнего ключа.
3. **Этапы при смене вида статуса** (вместо «этап не меняется» в проекте).
   - При каждом переходе между видом «выдан» и другим видом в `change_status_in` все платежи заказа становятся `before_issue`, затем пересчитывается `prepayment`. Это выдача, «Вернуть в работу», «Принять по гарантии», откат выдачи и массовая смена.
   - Платёж при выдаче пишется уже после этого — `at_issue`.
   - Иначе после возврата в работу повторная выдача показала бы «К оплате» без уже внесённого, и клиент заплатил бы дважды.
   - Инвариант: у невыданного заказа `prepayment` = «Оплачено».
   - Платёж, принятый заказом в виде «выдан», — `after_issue`.
4. **Выдача с оплатой** — обязательное тело `IssueRequest { status_id?, payment? }` у `POST /api/v1/orders/{id}/issue`. `{}` — выдача как раньше.
   - `status_id` — выдающий статус, выбранный в переключателе статусов. Не вида «выдан» или архивный → `status_id`/`invalid`.
   - `payment` вне 1…`MAX_PAYMENT` → `payment`/`out_of_range`.
   - Диалог «Выдать» всегда идёт через `/issue`; `/status` из диалога больше не зовётся.
   - `Core::issue_order` остаётся обёрткой `issue_order_with(…, IssueRequest::default())`: вызовы в тестах не меняются.
5. **Повтор выдачи.** Если заказ уже в этом статусе, ответ — заказ без изменений, платёж не пишется. Так повтор запроса после потери связи денег второй раз не берёт.
6. **Журнал выдачи с оплатой** — одна запись `order.status_changed`.
   - Сводка: «Статус: «Готов» → «Выдан»; принята оплата 3 000 ₽».
   - В подробностях, кроме `from` и `to`, — `payment`.
   - Отдельной `order.payment_added` нет.
7. **«Отменить» после выдачи** возвращает статус, а платёж при выдаче остаётся: по решению 3 он становится `before_issue`. Уведомление после отмены: «Платёж 3 000 ₽ остался в заказе — удалите его в «Платежах», если он ошибочный». Деньги молча не удаляются.
8. **Предоплата при приёме.**
   - Создаёт платёж `before_issue` с сегодняшним днём; «кто принял» — по решению 2.
   - Запись журнала одна — `order.created`, сводка «Заказ № N принят, предоплата 2 000 ₽», в подробностях `payment`.
   - `prepayment` больше `MAX_PAYMENT` → `prepayment`/`out_of_range`.
   - `orders.prepaid_at` больше не пишется нигде. Колонка в БД остаётся, `Order.prepaid_at` уходит из API в задаче 6.
9. **Предел суммы** `MAX_PAYMENT` = 99 999 999 999 копеек, как `MAX_MONEY_KOPECKS` интерфейса (`apps/ui/src/lib/format/money.ts`). Общего предела денег в Rust нет, константа живёт в `mw_domain::payment`.
10. **День платежа** — не позже сегодня (`Core::now().date()`) и не раньше дня приёма заказа. Нижней границы в проекте нет: платёж до приёма бессмыслен, чаще всего это опечатка в годе.
    - Новый код `payment_day` у поля `day`.
    - Перенесённые платежи (миграция, импорт) не проверяются.
11. **Ключ повтора** `request_key` у `NewOrderPayment` — как у `NewOrderPart`.
    - Повтор с тем же ключом, содержимым (`day`, `amount`, `received_by`) и `added_by_id` → прежний ответ без новой строки.
    - Иначе → `request_key`/`duplicate`.
    - В БД — UNIQUE (`order_id`, `request_key`).
12. **Ответ** — `OrderPayments { payments, paid, prepayment }`.
    - `prepayment` (сумма `before_issue`) нужен, чтобы список заказов обновил строку без перечитывания.
    - Порядок платежей: день, затем номер.
13. **Суммы считаются в Rust**, не `SUM()` в SQL: у PostgreSQL `SUM(bigint)` — `numeric`, у SQLite — целое.
    - У `Money` нет арифметики: копейки складываются в `i64` через `checked_add`, результат — `Money::from_kopecks`; переполнение — `CoreError::Internal`.
    - Один помощник `sum_kopecks` на всё: `paid_by_order`, `order_payments`, `recount_prepayment`, список.
14. **Пересчёт** `recount_prepayment` меняет только `orders.prepayment`: ни `version`, ни `updated_at`. Платёж не правка заказа: правило «правка без изменений не создаёт версию» его не касается.
15. **Удалённый заказ.**
    - Платежи читаются; добавить и удалить нельзя (`id`/`deleted`), как запчасти.
    - Заказы удаляются только мягко (`deleted_at`), восстановления нет.
    - Жёсткое удаление — только `wipe()` импорта: он удаляет `order_payments` раньше `orders`.
16. **Перенос** (миграция данных).
    - Какие заказы: `prepayment > 0` и ни одного платежа, включая удалённые.
    - День и `added_at` — из `prepaid_at`, иначе из `received_at`.
    - Поля: `legacy = true`, `before_issue`, «кто принял» и `added_by_name` пусты.
    - Записи журнала нет.
    - Импорт MyWork 2 делает то же из `Predoplata`/`Data_predoplaty` и пишет `orders.prepaid_at = NULL`. Поэтому импорт до и после этой версии даёт одно и то же.
17. **Список.**
    - `OrderListItem.paid` (все платежи) считается пачкой по номерам страницы (`paid_by_order`); сортировки по нему нет.
    - Столбцы «Оплачено» и «Остаток» стоят после «Предоплата», скрыты по умолчанию, без сортировки.
    - После платежа строку обновляет событие `emitOrderPaid`. Список клиента перечитывается при открытии.
18. **Excel.**
    - «Оплачено» и «Остаток» встают после «К оплате»: было 24 столбца, станет 26.
    - «Остаток» без итога — пусто; отрицательный — как есть.
19. **Документы.**
    - `order.paid` («заказ.оплачено»): пусто при 0.
    - `order.remaining` («заказ.остаток»): без итога пусто, отрицательный — с минусом, как `order.due`.
    - В реестре полей было 39, станет 41.
    - `TEMPLATE_FILE_VERSION` не меняется: модель шаблона та же, в реестре только новые ключи. Стандартные акты не меняются.
20. **Палитра «Недавние».** Строку списка она строит из `Order` (`listItemOf`). Поле `paid` берёт из `o.prepayment`: сумм палитра не показывает.
21. **«Остаток» в блоке «Платежи».** Меньше нуля — «Переплата N». Без итога — «Итог не указан». Итог и скидка берутся живые, из редактора, ещё до сохранения.

## Global Constraints

- `mw-domain` без ввода-вывода (и без `chrono`): проверка дня — в `mw-core`. К БД — только через `mw-db` и `mw-core`; права проверяет `mw-core`.
- Любая запись в БД — только через `Core::begin_write()`. Платёж, пересчёт `prepayment` и запись журнала идут в одной такой транзакции; выдача с оплатой — тоже одна транзакция.
- Каждый тест с БД идёт по `all_backends()` (`cores()` в `crates/mw-core/tests/common/mod.rs`, часы 2026-11-11 09:00 UTC).
  - PostgreSQL: `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`.
  - Исключение — замена базы при импорте: она бывает только у файловой SQLite (`file_sqlite_core`).
- Деньги — только `Money`. Арифметики у него нет: считать в копейках (`kopecks()`, `i64`) через `checked_add`/`checked_sub`/`saturating_sub`, обратно — `Money::from_kopecks`.
- Время — только `Core::now()`, день — `Core::now().date()`.
- Инвариант «`prepayment` = сумма `before_issue`» держится, начиная с задачи 6. Промежуточные коммиты ветки не выпускаются, ветка вливается целиком.
- Выпущенные миграции не меняются. Новые — `m20261126_000001_order_payments` (схема) и `m20261126_000002_order_payments_from_prepayment` (данные), последними в `Migrator` (21-я и 22-я). Литералы миграции данных заморожены: константы `mw-domain` в ней не используются. Новых прав нет.
- `SEARCH_INDEX_VERSION` не меняется: `search_text`, `normalize_search`, `index_text` и `mw_domain::phone` не трогать.
- `Actor::for_tests` и `Core::db()` — только в тестах.
- Типы API:
  - `Serialize`/`Deserialize`, `ts_rs::TS`, `utoipa::ToSchema`, без `#[ts(export)]`;
  - входные — с `deny_unknown_fields`; необязательные поля — `patch::present` и `#[ts(optional_fields)]`;
  - `Option` в ответах — с `#[schema(required = true)]`;
  - `NewOrderPayment` проверяет `check_new`, `IssueRequest` — `check_patch` (`crates/mw-core/tests/api_types.rs`);
  - корни — в `crates/mw-api/src/contract.rs`;
  - имена уникальны: `OrderPayment`, `OrderPayments`, `NewOrderPayment`, `PaymentStage`, `IssueRequest`.
- Маршруты — `#[inline(never)] fn order_payment_routes()` в `crates/mw-api/src/routes/mod.rs`, обработчики — в `crates/mw-api/src/routes/order_payments.rs`, тег «Заказы». Тест `crates/mw-api/tests/stack.rs` зелёный.
- После изменения API:
  1. `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`;
  2. затем `cargo test -p mw-api --test contract` без переменной — должен пройти;
  3. `openapi.json` и `bindings/` — в том же коммите.
  В `documented_operations_match_the_api_map` растут число операций и список.
- Коды ошибок. Текст — в `message_ru` и в списке кодов в шапке `crates/mw-domain/src/validation.rs`; проверка — `field_error_messages_are_russian`.
  - новый `payment_day` у `day`: «День оплаты — не раньше приёма заказа и не позже сегодня»;
  - прежние коды:
    - `out_of_range`: `amount`, `payment`, `prepayment`;
    - `too_long`: `received_by`;
    - `invalid`: `request_key`, `status_id`;
    - `duplicate`: `request_key`;
    - `deleted`: `id`.
- Документы (`mw-docs`): данные заказа — только значения `data.json`; рендер — только `Core::export_*` под семафором; модель шаблона не меняется, `Template::validate` не трогается.
- Интерфейс (`apps/ui`):
  - Svelte 5 на рунах, обработчики — пропсы `on…`;
  - импорты `#lib/...` с расширениями, типы API — `import type` из `$api/...`;
  - цвета и размеры — токены `tokens.css`; ширины — `display` и `:global([data-width=…])`;
  - тексты — `ru.ts`; иконки `@lucide/svelte` по одной;
  - деньги — `#lib/format/money.js` и `MoneyInput`; даты — `#lib/format/date.js`;
  - `fetch` — только `client.ts`, данные — `#lib/api/orders.js`;
  - Ctrl/⌘+Enter в диалогах — `submitShortcut`.
- Сквозные тесты — Chromium и WebKit, по ролям и подписям, на настоящем `mw-server`. Общие серверы: 4174 (с администратором) и 4173 (без входа). Данные создаются через API с `uniq()`.
- Перед коммитом зелёное (ни одной ошибки и ни одного упавшего теста):
  - **полный Rust:** `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`, с `MW_TEST_PG_URL`;
  - в задачах 2, 4–8 ещё `cd apps/ui && npm run verify && npm run test:e2e`.
- Зависимостей не добавлять; `apps/desktop/src-tauri/Cargo.lock` не меняется.
- Коммиты заканчиваются строкой `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Заказ вернули в работу (или выдачу отменили) и выдают снова.** Деньги второй раз не берутся: при повторной выдаче «К оплате» = итог − скидка − всё внесённое, переключателя «Принять оплату» нет.
   - `reissue_after_return_to_work_counts_every_payment` (задача 4);
   - шаг 6 сквозного теста (задача 8).
2. **Акт выдачи печатают после оплаты при выдаче.** В нём прежнее «К оплате» (3 000), а не 0: платёж `at_issue` в предоплату не входит.
   - `issue_act_due_ignores_the_payment_at_issue` (задача 4, `crates/mw-core/tests/documents.rs`).
3. **Перенос предоплаты на обеих СУБД.** День берётся из `prepaid_at`, иначе из приёма. Сумма та же. Удалённые заказы тоже переносятся. Заказ, у которого платежи уже есть, не трогается.
   - `prepayments_become_payments`, `orders_with_payments_are_left_alone` (задача 1).
4. **Повторы не удваивают деньги.**
   - Тот же `request_key` — одна строка.
   - Повтор выдачи уже выданного заказа платежа не пишет.
   - Два писателя одного ключа — одна строка (UNIQUE).
   Тесты: `same_request_key_adds_once` (задача 2), `repeated_issue_takes_no_second_payment` (задача 4).
5. **Платёж не мешает открытой карточке.** Версия заказа не меняется. Правка карточки после платежа сохраняется без 409 и не затирает пересчитанную `prepayment`.
   - `payment_leaves_the_order_version` (задача 2);
   - `an_edit_after_a_payment_keeps_the_recounted_prepayment` (задача 6);
   - шаг 4 сквозного теста (задача 8).

---

### Task 1: Схема платежей и перенос предоплаты

Правила AGENTS.md: выпущенные миграции не меняются, только добавляются; `mw-domain` без ввода-вывода; каждый тест с БД — `all_backends()`; `Core::db()` только в тестах; импорт ничего не теряет (замена стирает и новую таблицу).

**Files:**
- Create: `crates/mw-domain/src/payment.rs` (+ `pub mod payment;` в `crates/mw-domain/src/lib.rs`)
- Modify: `crates/mw-domain/src/validation.rs` (код `payment_day`: шапка и `message_ru`), `crates/mw-core/tests/api_types.rs` (`field_error_messages_are_russian`)
- Create: `crates/mw-db/src/migration/m20261126_000001_order_payments.rs`, `crates/mw-db/src/migration/m20261126_000002_order_payments_from_prepayment.rs`; Modify: `crates/mw-db/src/migration/mod.rs` (модули и две последние строки `Migrator`)
- Create: `crates/mw-db/src/entity/order_payment.rs`; Modify: `crates/mw-db/src/entity/mod.rs`
- Test: `crates/mw-db/tests/order_payments_migration.rs`
- Modify: `crates/mw-core/src/import/write.rs` (`wipe`)
- Test: `crates/mw-core/tests/import.rs` (`replace_wipes_the_stock_written_after_the_previous_import`)

**Interfaces:**
- Produces (`mw_domain::payment`):
  - `code_enum! { /// Этап платежа относительно выдачи заказа. PaymentStage { BeforeIssue => "before_issue", AtIssue => "at_issue", AfterIssue => "after_issue" } }`;
  - `pub const MAX_PAYMENT: Money = Money::from_kopecks(99_999_999_999);`
  - `pub const MAX_RECEIVED_BY_CHARS: usize = 100;`
  - `pub fn amount_in_range(m: Money) -> bool` — `Money::ZERO < m && m <= MAX_PAYMENT`.
- Produces (схема, `m20261126_000001_order_payments`): таблица `order_payments`.
  - Колонки:
    - `id` BIGINT автоинкремент;
    - `order_id` BIGINT NOT NULL, внешний ключ `fk_order_payments_order` → `orders.id`, без каскада;
    - `day` DATE NOT NULL;
    - `amount` BIGINT NOT NULL;
    - `received_by` TEXT NOT NULL DEFAULT `''`;
    - `stage` TEXT NOT NULL;
    - `added_by_id` BIGINT NULL (без внешнего ключа, как у `order_parts`);
    - `added_by_name` TEXT NOT NULL DEFAULT `''`;
    - `added_at` TIMESTAMP NOT NULL;
    - `legacy` BOOLEAN NOT NULL DEFAULT false;
    - `request_key` TEXT NULL.
  - Индексы: `uq_order_payments_request_key` UNIQUE (`order_id`, `request_key`), `idx_order_payments_order_id`.
  - Помощники `a`, `id`, `text`, `number`, `index` — копия из `m20261124_000001_stock.rs`; `use_transaction() -> Some(true)`.
- Produces (данные, `m20261126_000002_order_payments_from_prepayment`):
  1. Выбрать `id`, `prepayment`, `prepaid_at`, `received_at` из `orders`, где `prepayment > 0` и `NOT EXISTS (SELECT 1 FROM order_payments WHERE order_id = orders.id)`, по `id`.
  2. Для каждого: `stamp = prepaid_at.unwrap_or(received_at)`; строка `{order_id, day: stamp.date(), amount: prepayment, received_by: '', stage: 'before_issue', added_by_id: NULL, added_by_name: '', added_at: stamp, legacy: true, request_key: NULL}`.
  3. Вставка `Query::insert` пачками по 500 строк (10 колонок × 500 — меньше пределов параметров SQLite и PostgreSQL).
  Чтение — `db.query_all`, `row.try_get_by_index`, как в `m20261124_000002_stock_data.rs`. Литерал `'before_issue'` заморожен.
- Produces (сущность): `order_payment::Model { id: i64, order_id: i64, day: Date, amount: i64, received_by: String, stage: String, added_by_id: Option<i64>, added_by_name: String, added_at: DateTime, legacy: bool, request_key: Option<String> }`; связь с `order` — как у `order_part`.
- Produces (`wipe`): `order_payment::Entity::delete_many()` — рядом с `order_part`, до `order::Entity::delete_many()`.

- [ ] **Step 1: Тесты:**
  - `payment.rs`:
    - `stages_have_stable_codes` — `as_str`, `FromStr` и serde: `"before_issue"`, `"at_issue"`, `"after_issue"`;
    - `amounts_are_from_one_kopeck_to_the_limit` — `0` и `−1` → `false`; `1` и `MAX_PAYMENT` → `true`; `MAX_PAYMENT + 1 коп.` → `false`.
  - `api_types.rs`, `field_error_messages_are_russian`:
    - `payment_day` — в список кодов;
    - точная сверка текста: `assert_eq!(FieldError::new("day", "payment_day").message_ru(), "День оплаты — не раньше приёма заказа и не позже сегодня")`.
    Сам тест проверяет только заглавную букву, а неизвестный код и так даёт «Недопустимое значение»: без `assert_eq!` он зелёный и без реализации.
  - `order_payments_migration.rs`: константы `BEFORE_PAYMENTS: u32 = 20` (все прежние миграции) и `BEFORE_PAYMENTS_DATA: u32 = 21`. Помощник `exec` и вставка клиента, статуса и заказа — как в `crates/mw-db/tests/stock_migration.rs`.
    - `prepayments_become_payments` (Review Focus 3): `all_backends_up_to(BEFORE_PAYMENTS)`, четыре заказа:
      - A — `prepayment` 50000, `prepaid_at` 2015-03-05 14:20, приём 2015-03-04 10:00;
      - B — 30000, `prepaid_at` NULL, приём 2015-03-06 09:05;
      - C — 0;
      - D — 70000, `prepaid_at` 2015-03-07 11:00, `deleted_at` задан.
      Затем `mw_db::migrate(&db)` → три платежа по `order_id`:
      - A — день 2015-03-05, 50000, `added_at` 2015-03-05 14:20;
      - B — день 2015-03-06, 30000, `added_at` 2015-03-06 09:05;
      - D — день 2015-03-07, 70000.
      У всех: `legacy`, `before_issue`, `received_by` и `added_by_name` пусты, `added_by_id` и `request_key` — NULL. У C платежа нет. `orders.prepayment` не изменился.
    - `orders_with_payments_are_left_alone`: `all_backends_up_to(BEFORE_PAYMENTS_DATA)` → заказ с `prepayment` 50000 и уже одной строкой (день 2026-11-20, 20000) → `migrate` → строка одна, та же.
    - `payments_need_an_order_and_a_unique_key`: `all_backends()`:
      - платёж с `order_id` 999 → ошибка;
      - два платежа одного заказа с ключом `"k"` → второй — ошибка;
      - тот же ключ у другого заказа → ок;
      - два платежа с `request_key` NULL → ок.
  - `import.rs`, `replace_wipes_the_stock_written_after_the_previous_import`: до второго импорта вставить строку `order_payment::ActiveModel` у импортированного заказа через `core.db()` → замена проходит, `order_payment::Entity::find().count()` = 0. В `sample_bundle()` предоплат пока нет.
- [ ] **Step 2: Запустить:**
  - не собирается:
    - `cargo test -p mw-domain -- payment`;
    - `cargo test -p mw-db --test order_payments_migration`;
    - `cargo test -p mw-core --test import -- replace_wipes`;
  - падает на новом `assert_eq!` текста `payment_day`: `cargo test -p mw-core --test api_types`.
- [ ] **Step 3: Реализовать** по Interfaces.
- [ ] **Step 4: Запустить** те же команды с `MW_TEST_PG_URL` → PASS на `sqlite` и `postgres`, затем полный Rust → PASS.
- [ ] **Step 5: Коммит** `feat(db): платежи заказов и перенос предоплаты`.

### Task 2: Платежи в ядре и API

Правила AGENTS.md:
- запись — `begin_write()`;
- права — в `mw-core`;
- деньги — `Money`, время — `Core::now()`;
- типы API — `deny_unknown_fields`, без `#[ts(export)]`;
- маршруты — в функции раздела;
- контракт обновляется командой и коммитится с кодом.

**Files:**
- Create: `crates/mw-core/src/orders/payments.rs`; Modify: `crates/mw-core/src/orders/mod.rs` (`mod payments; pub use payments::{NewOrderPayment, OrderPayment, OrderPayments};` и `pub(crate) use` помощников), `crates/mw-core/src/lib.rs` (`pub use`)
- Modify: `crates/mw-core/src/orders/create.rs` (платёж при приёме, `prepaid_at` не пишется, предел, сводка журнала)
- Test: `crates/mw-core/tests/order_payments.rs` (новый), `crates/mw-core/tests/orders_create.rs`, `crates/mw-core/tests/api_types.rs` (`check_new::<NewOrderPayment>`)
- Create: `crates/mw-api/src/routes/order_payments.rs`; Modify: `crates/mw-api/src/routes/mod.rs` (`mod order_payments;`, `order_payment_routes()`, `.merge(order_payment_routes())`), `crates/mw-api/src/contract.rs`, `crates/mw-api/tests/contract.rs`, `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`
- Test: `crates/mw-api/tests/order_payments.rs` (новый), `crates/mw-api/tests/orders.rs` (`intake_and_read`)

**Interfaces:**
- Consumes: `PaymentStage`, `MAX_PAYMENT`, `MAX_RECEIVED_BY_CHARS`, `amount_in_range`, `order_payment` (задача 1); `crate::attachments::valid_key`; `super::update::{load, deleted_error}`; `crate::export::dmy`; `Core::money_text(_, MoneyText::Short)`; `write_audit`.
- Produces (типы API):
  - `OrderPayment { id: i64, day: NaiveDate, amount: Money, received_by: String, stage: PaymentStage, added_by_name: String, added_at: NaiveDateTime, legacy: bool }`;
  - `OrderPayments { payments: Vec<OrderPayment>, paid: Money, prepayment: Money }`:
    - `paid` — все платежи;
    - `prepayment` — сумма `before_issue`, равна `orders.prepayment`;
  - `NewOrderPayment { day: NaiveDate, amount: Money, received_by: String, request_key: String }` — `deny_unknown_fields`, все поля обязательны.
- Produces (`mw-core`, `pub(crate)` в `orders/payments.rs`):
  - `const ORDER_ENTITY: &str = "заказ"; const PAYMENT_ENTITY: &str = "платёж заказа";` — как `ORDER_ENTITY`/`PART_ENTITY` в `stock/parts.rs` (там «запчасть заказа»);
  - `fn sum_kopecks(amounts: impl IntoIterator<Item = i64>) -> Result<Money, CoreError>`:
    - складывает копейки через `i64::checked_add`;
    - результат — `Money::from_kopecks`;
    - переполнение — `CoreError::Internal`;
  - `async fn paid_by_order(conn: &impl ConnectionTrait, ids: &[i64]) -> Result<HashMap<i64, Money>, CoreError>`:
    - выбирает `order_id`, `amount` кусками по `ID_CHUNK`, как `parts_cost` в `export/orders.rs`;
    - суммирует `sum_kopecks` по заказу;
    - заказа без платежей в карте нет.
    Задача 5 зовёт его из списка и документов;
  - `fn received_by_of(actor: &Actor) -> String` — решение 2;
  - `struct PaymentRow { day: NaiveDate, amount: Money, received_by: String, stage: PaymentStage, legacy: bool, request_key: Option<String> }`;
  - `async fn insert_payment(tx: &DatabaseTransaction, now: NaiveDateTime, actor: &Actor, order_id: i64, row: PaymentRow) -> Result<order_payment::Model, CoreError>` — `added_by_id = actor.user_id()`, `added_by_name = actor.name()`, `added_at = now`;
  - `async fn recount_prepayment(tx: &DatabaseTransaction, order_id: i64) -> Result<Money, CoreError>` — `sum_kopecks` по суммам `before_issue`; `UPDATE orders SET prepayment` без `version` и `updated_at`;
  - `fn payment_details(m: &order_payment::Model) -> serde_json::Value` — `{payment_id, day: "2026-11-11", amount: копейки, received_by, stage, legacy}`;
  - `async fn order_payments(conn: &impl ConnectionTrait, order_id: i64) -> Result<OrderPayments, CoreError>`:
    - порядок `day`, затем `id`;
    - `paid` — `sum_kopecks` по всем строкам, `prepayment` — по строкам `before_issue`. Второго способа подсчёта нет.
- Produces (`Core`):
  - `list_order_payments(&self, _actor: &Actor, order_id: i64) -> Result<OrderPayments, CoreError>`:
    - прав не нужно;
    - нет заказа → `NotFound { entity: ORDER_ENTITY, id }`;
    - удалённый заказ читается.
  - `add_order_payment(&self, actor: &Actor, order_id: i64, new: NewOrderPayment) -> Result<OrderPayments, CoreError>`:
    1. Право `orders_edit`.
    2. До транзакции — проверки:
       - `amount` (`amount_in_range`) → `amount`/`out_of_range`;
       - `received_by` (обрезка пробелов, ≤ 100 знаков) → `too_long`;
       - `valid_key(request_key)` → `request_key`/`invalid`.
    3. `begin_write()`, заказ: нет → `NotFound { entity: ORDER_ENTITY, id }` (404), удалён → `deleted_error()`.
    4. День: `day < received_at.date()` или `day > now.date()` → `day`/`payment_day`.
    5. Повтор по ключу — решение 11.
    6. Этап: статус заказа вида «выдан» → `after_issue`, иначе `before_issue`.
    7. `insert_payment`, затем `recount_prepayment`.
    8. Журнал `order.payment_added` (`order_id`, `client_id`):
       - сводка «Принята оплата 1 500 ₽ (11.11.2026)»; при непустом «кто принял» — « · Иванов»;
       - подробности — `payment_details`.
    9. Коммит, затем `order_payments`.
  - `remove_order_payment(&self, actor: &Actor, order_id: i64, payment_id: i64) -> Result<OrderPayments, CoreError>`:
    1. `orders_edit`.
    2. `begin_write()`; заказ живой.
    3. Платёж этого заказа: нет → `NotFound { entity: PAYMENT_ENTITY, id: payment_id }`.
    4. Удаление, `recount_prepayment`.
    5. Журнал `order.payment_removed`: «Удалён платёж 1 500 ₽ (11.11.2026)» (+ « · Иванов»), подробности — `payment_details`.
- Produces (приём, `create_order_once`):
  - `prepayment > MAX_PAYMENT` → `prepayment`/`out_of_range` (рядом с проверкой `< 0`);
  - `prepaid_at: Set(None)`;
  - при `prepayment > 0` — `insert_payment` с днём `now.date()`, `received_by_of(actor)`, `before_issue`, без ключа;
  - сводка `order.created` — «Заказ № N принят, предоплата 2 000 ₽», в подробностях `payment`; без предоплаты — прежняя.
- Produces (API, тег «Заказы», все ответы 200 `OrderPayments`):

  | Метод и путь | Тело | Ответ |
  |---|---|---|
  | `GET /api/v1/orders/{id}/payments` | — | `OrderPayments` |
  | `POST /api/v1/orders/{id}/payments` | `NewOrderPayment` | `OrderPayments` |
  | `DELETE /api/v1/orders/{id}/payments/{payment_id}` | — | `OrderPayments` |

  `contract.rs`: `mw_core::NewOrderPayment`, `OrderPayment`, `OrderPayments` (`PaymentStage` — зависимостью). Операций 128 → 131.

- [ ] **Step 1: Тесты ядра** (`order_payments.rs`, по `cores()`). Помощники:
  - `cashier()` = `Actor::for_tests(Some(7), "Петрова", PermissionSet::all())`;
  - `pay(day, rubles, by, key) -> NewOrderPayment`;
  - `day(y, m, d)`.
  Тесты:
  - `payment_is_listed_with_sums_and_journal`: заказ без предоплаты; `cashier()` вносит 1 500 ₽ за 11.11.2026, «Иванов». Ожидается:
    - одна строка: `received_by` «Иванов», `before_issue`, `added_by_name` «Петрова», `added_at` = `now`, не `legacy`;
    - `paid` = `prepayment` = 1 500 ₽, `get_order().prepayment` = 1 500 ₽;
    - `last_audit`: `order.payment_added`, сводка «Принята оплата 1\u{a0}500\u{a0}₽ (11.11.2026) · Иванов», в подробностях `amount` 150000.
  - `payment_leaves_the_order_version` (Review Focus 5): версия и `updated_at` заказа до и после платежа и удаления равны; `update_order` со старой версией проходит.
  - `payments_come_by_day_then_id`: `t.clock.advance(2 дня)`; внести 13.11, 12.11, 12.11 → порядок: 12.11 (вторая), 12.11 (третья), 13.11.
  - `day_must_be_between_intake_and_today`:
    - 10.11.2026 (раньше приёма) → `[("day","payment_day")]`;
    - 12.11.2026 (завтра) → то же;
    - строк и записей журнала нет.
  - `amount_received_by_and_key_are_checked`:
    - 0, −1 коп., `MAX_PAYMENT + 1 коп.` → `amount`/`out_of_range`; `MAX_PAYMENT` → ок;
    - 101 знак «кто принял» → `received_by`/`too_long`; 100 → ок; « Иванов » хранится «Иванов»;
    - ключ `""` и `"a b"` → `request_key`/`invalid`.
  - `same_request_key_adds_once` (Review Focus 4):
    - тот же ключ и содержимое → одна строка, ответ тот же, одна запись журнала;
    - тот же ключ с другой суммой → `request_key`/`duplicate`;
    - тот же ключ и содержимое от `Actor::for_tests(Some(8), "Другой", PermissionSet::all())` → `duplicate`. `user_actor(8)` не годится: у него нет прав, он получил бы `Forbidden`.
  - `removing_recounts_and_journals`:
    - внести 500 и 700, удалить 500 → `paid` и `prepayment` 700;
    - журнал `order.payment_removed` «Удалён платёж 500\u{a0}₽ (11.11.2026)»;
    - удалить снова → `NotFound`; платёж другого заказа → `NotFound`.
  - `a_payment_of_an_issued_order_comes_after_issue`: выключить «Обязательно при выдаче» (`SettingsPatch { issue_required: Some(vec![]) }`), `issue_order`, внести 300 → `after_issue`, `paid` 300, `prepayment` 0.
  - `deleted_order_payments_are_read_only`:
    - внести, `delete_order`;
    - `list_order_payments` — та же строка;
    - добавить и удалить → `[("id","deleted")]`.
  - `rights`:
    - `actor_with(&[OrdersCreate])` — добавить и удалить `Forbidden(OrdersEdit)`;
    - `actor_with(&[])` читает список;
    - нет заказа → `NotFound`.
  - `open_mode_leaves_received_by_empty`: `Actor::unrestricted("Без входа")` принимает заказ с предоплатой → платёж с пустым `received_by`, `added_by_name` «Без входа».
- [ ] **Step 2: Тесты приёма** (`orders_create.rs`, `intake_creates_client_order_barcode_and_audit`):
  - вместо `assert_eq!(o.prepaid_at, …)`: один платёж — 2 000 ₽, день 11.11.2026, `before_issue`, пустой «кто принял» (у `actor_with` нет `user_id`); `prepayment` 2 000 ₽;
  - сводка — `format!("Заказ № {} принят, предоплата 2\u{a0}000\u{a0}₽", o.id)`;
  - в проверке ошибок приёма — `prepayment: MAX_PAYMENT + 1 коп.` → `("prepayment","out_of_range")`;
  - повтор приёма с тем же `request_key` (если в файле есть такой тест — дописать, иначе новый `repeated_intake_pays_once`) → платёж один.
- [ ] **Step 3: `api_types.rs`:** `check_new::<NewOrderPayment>(json!({ "day": "2026-11-11", "amount": 150000, "received_by": "", "request_key": "k-1" }))`.
- [ ] **Step 4: Тесты API** (`crates/mw-api/tests/order_payments.rs`, `apis(Listen::Network)`):
  - `payments_are_added_listed_and_removed`:
    - POST → 200, `payments[0].day` = `"2026-11-11"`, `paid` 150000;
    - GET → то же;
    - DELETE → 200, `paid` 0; DELETE снова → 404.
  - `wrong_payments_answer_422_or_400`:
    - день `2026-11-12` → 422, поле `day`, код `payment_day`, `message` — текст кода;
    - сумма 0 → 422 `amount`/`out_of_range`;
    - лишнее поле → 400; без `request_key` → 400.
  - `payment_rights`: `token_with("kassa", &[OrdersCreate])` — POST и DELETE → 403, GET → 200.
  - `orders.rs`, `intake_and_read`: проверка `prepaid_at` заменяется на `GET …/payments` → одна строка 200000, `prepayment` 200000.
- [ ] **Step 5: Запустить** `cargo test -p mw-core --test order_payments` → не собирается.
- [ ] **Step 6: Реализовать** по Interfaces, затем обновить контракт (Global Constraints) и `documented_operations_match_the_api_map`: 131 и три новые строки.
- [ ] **Step 7: Запустить** → PASS на обеих СУБД:
  - `cargo test -p mw-core --test order_payments --test orders_create --test api_types`;
  - `cargo test -p mw-api --test order_payments --test orders --test contract --test stack`.
  Затем полный Rust → PASS; `cd apps/ui && npm run verify && npm run test:e2e` → PASS. Интерфейс не меняется: новые привязки ещё не используются; подсказка «внесена …» у новых заказов больше не показывается.
- [ ] **Step 8: Коммит** `feat(core): платежи по заказу — список, приём, удаление; предоплата при приёме — первый платёж`.

### Task 3: Импорт MyWork 2: платёж из предоплаты

Правила AGENTS.md:
- импорт ничего не теряет;
- новая форма значения старой базы — сначала строка в фикстуре и тест (здесь новых форм нет: `Predoplata` и `Data_predoplaty` уже разбираются в `mw-legacy/src/convert/orders.rs`);
- реальные базы в тесты не попадают.

**Files:**
- Modify: `crates/mw-core/src/import/write.rs` (`write_orders`: `prepaid_at: None`, платежи; сверка числа)
- Modify: `crates/mw-core/tests/common/mod.rs` (`sample_bundle`: у заказа 5 предоплата)
- Test: `crates/mw-core/tests/import.rs`

**Interfaces:**
- Consumes: `order_payment` (задача 1).
- Produces (в той же транзакции импорта, после вставки заказов):
  - на каждый `ImportedOrder` с `prepayment > 0` — строка `order_payment::ActiveModel`;
  - поля — как в миграции данных (решение 16): `stamp = prepaid_at.unwrap_or(received_at)`, `day = stamp.date()`, `added_at = stamp`, `legacy: true`, `before_issue`, остальное пусто;
  - вставка `insert_chunked`; сверка `expect_count::<order_payment::Entity>(tx, "платежи", n)`.
  - `orders.prepaid_at` пишется `None`; `ImportedOrder.prepaid_at` остаётся во входе.
  - `ImportSummary` не меняется.

- [ ] **Step 1: Тесты** (`import.rs`):
  - в `sample_bundle()` заказу 5 — `prepayment: Money::from_rubles(500)`, `prepaid_at: Some(dt(2015, 3, 5, 14, 20))`;
  - `legacy_prepayment_becomes_a_payment` — по `cores()`, как `import_into_empty_database_keeps_numbers_and_continues_them` (`ImportMode::IntoEmpty`):
    - у заказа 5 один платёж: день 2015-03-05, 50000, `legacy`, `before_issue`, пустые «кто принял» и `added_by_name`, `added_at` 2015-03-05 14:20;
    - у прочих заказов платежей нет;
    - `get_order(5).prepayment` = 500 ₽;
    - `orders.prepaid_at` заказа 5 — NULL (через `core.db()`);
  - `replace_wipes_the_stock_written_after_the_previous_import`: после замены платёж один — перенесённый (`legacy`); строки, вставленной до замены, нет;
  - прочие тесты на `sample_bundle()` предоплату не проверяют (`import.rs` и `reports.rs` её не упоминают). Если какой-то сравнивает заказ 5 целиком, в ожидание дописать `prepayment` 500 ₽.
- [ ] **Step 2: Запустить** `cargo test -p mw-core --test import` → падает (платежей нет).
- [ ] **Step 3: Реализовать.**
- [ ] **Step 4: Запустить** `cargo test -p mw-core --test import` и `cargo test -p mw-legacy` с `MW_TEST_PG_URL` → PASS; полный Rust → PASS.
- [ ] **Step 5: Коммит** `feat(import): предоплата MyWork 2 — платёж заказа`.

### Task 4: Выдача с оплатой и этапы платежей

Правила AGENTS.md: запись — одна `begin_write()` на выдачу с платежом; входные типы — `deny_unknown_fields` и `patch::present`; контракт — командой; сквозные тесты зовут API так же, как программа.

**Files:**
- Modify: `crates/mw-core/src/orders/types.rs` (`IssueRequest`), `crates/mw-core/src/orders/mod.rs`, `crates/mw-core/src/lib.rs`
- Modify: `crates/mw-core/src/orders/update.rs`:
  - `issue_order` → обёртка над `issue_order_with`;
  - `change_status_in(…, payment: Option<Money>)`, остальные вызовы передают `None`;
  - перевод этапов; платёж и сводка.
- Modify: `crates/mw-core/src/orders/payments.rs` (`restage_payments`)
- Test: `crates/mw-core/tests/order_payments.rs`, `crates/mw-core/tests/documents.rs`, `crates/mw-core/tests/api_types.rs` (`check_patch::<IssueRequest>()`)
- Modify: `crates/mw-api/src/routes/orders.rs` (`issue_order`: тело `IssueRequest`), `crates/mw-api/src/contract.rs`, `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`
- Test: `crates/mw-api/tests/orders.rs` (два вызова `/issue` — с телом `{}`; новый тест)
- Modify: `apps/ui/src/lib/api/orders.ts` (`issueOrder(id, version, req: IssueRequest = {})` шлёт тело)
- Modify: `apps/ui/e2e/reports.spec.ts:40`, `order-panel.spec.ts:543,675`, `issue-required.spec.ts:135`, `help-everywhere.spec.ts:77`, `order-card.spec.ts:95` (`data: {}` у `POST …/issue`)

**Interfaces:**
- Consumes: `insert_payment`, `recount_prepayment`, `payment_details`, `received_by_of` (задача 2).
- Produces (тип API):
  ```rust
  /// Выдача заказа: необязательный выдающий статус и оплата в той же транзакции.
  #[derive(Debug, Clone, Default, PartialEq, Deserialize, ts_rs::TS, utoipa::ToSchema)]
  #[serde(deny_unknown_fields)]
  #[ts(optional_fields)]
  pub struct IssueRequest {
      /// Статус вида «выдан»; не передан — первый неархивный такой.
      #[serde(default, deserialize_with = "crate::patch::present")]
      #[schema(nullable = false)]
      pub status_id: Option<i64>,
      /// Принятая при выдаче сумма, в копейках: 1…99 999 999 999.
      #[serde(default, deserialize_with = "crate::patch::present")]
      #[schema(nullable = false)]
      pub payment: Option<Money>,
  }
  ```
- Produces (`payments.rs`): `pub(crate) async fn restage_payments(tx: &DatabaseTransaction, order_id: i64) -> Result<(), CoreError>`. Все платежи заказа с `stage <> 'before_issue'` → `before_issue`, затем `recount_prepayment`.
- Produces (`Core`): `pub async fn issue_order_with(&self, actor: &Actor, id: i64, expected_version: i64, req: IssueRequest) -> Result<Order, CoreError>`.
  1. Право `orders_edit`.
  2. `payment` вне диапазона (`amount_in_range`) → `payment`/`out_of_range`.
  3. `begin_write()`.
  4. Если задан `status_id`, статус должен быть, не архивный и вида «выдан», иначе `status_id`/`invalid`.
  5. `change_status_in(&tx, actor, id, v, req.status_id, true, req.payment)`, коммит.
- Produces (`change_status_in`, после проверки «тот же статус → ответ без изменений» — решение 5):
  - `from_issued` — вид прежнего статуса «выдан»; `to_issued` — вид нового;
  - `from_issued != to_issued` → `restage_payments` (решение 3);
  - `to_issued` и задан `payment` → `insert_payment` с днём `now.date()`, `received_by_of(actor)`, `at_issue`, без ключа;
  - сводка `order.status_changed` — прежняя, а при платеже + `format!("; принята оплата {}", money_text(amount, Short))`;
  - подробности — `{from, to}` и при платеже `payment: payment_details(..)`.
- Produces (API): `POST /api/v1/orders/{id}/issue`, `request_body = IssueRequest`. Описание: «Тело обязательно: `{}` — выдача без оплаты». Ответ прежний — `Order`. Операций по-прежнему 131.
- Produces (интерфейс): `issueOrder = (id, version, req: IssueRequest = {}) => request<Order>('POST', …, { query: { version }, body: req })`. Вызовы в `actions.svelte.ts` пока прежние.

- [ ] **Step 1: Тесты ядра** (`order_payments.rs`). Помощник `issuable(t)` выключает «Обязательно при выдаче».
  - `issue_with_payment_writes_one_at_issue_payment`:
    - заказ с предоплатой 500, итог 3 500 → `issue_order_with(…, IssueRequest { payment: Some(3000 ₽), ..Default::default() })`;
    - платежи: 500 `before_issue` и 3 000 `at_issue`, день 11.11.2026; `paid` 3 500, `prepayment` 500;
    - новая запись журнала одна — `order.status_changed`, сводка оканчивается «; принята оплата 3\u{a0}000\u{a0}₽», `details.payment.amount` = 300000; `order.payment_added` нет.
  - `issue_refuses_a_wrong_payment_or_status`:
    - `payment` 0 → `[("payment","out_of_range")]`;
    - `status_id` статуса вида «в работе» → `[("status_id","invalid")]`;
    - заказ не выдан, платежей нет.
  - `issue_to_a_picked_issued_status`:
    - статуса «Куплен» в `cores()` нет (он только в `sample_bundle()`). Первой строкой его создать: `t.core.create_status(&admin(), NewStatus { name: "Куплен".into(), kind: StatusKind::Issued, color: "gray".into() })`;
    - выдача с `status_id` «Куплен» → заказ в нём, платёж `at_issue`.
  - `repeated_issue_takes_no_second_payment` (Review Focus 4): тот же запрос второй раз, со старой версией → заказ без изменений, платёж при выдаче один.
  - `reissue_after_return_to_work_counts_every_payment` (Review Focus 1):
    1. Предоплата 500, итог 3 500; выдача с 3 000.
    2. `change_status` в «Готов» → все платежи `before_issue`, `prepayment` = 3 500.
    3. Выдать снова без оплаты → «К оплате» (`final_price − prepayment − discount`) = 0.
    Тот же переход через `accept_warranty_case` переводит этапы так же.
  - `restaging_needs_a_change_of_kind`: «Куплен» создаётся так же через `create_status`; «Выдан» → «Куплен» (оба вида «выдан») — этапы не меняются.
  - `issue_without_payment_keeps_the_old_summary`: сводка — ровно «Статус: «…» → «…»».
- [ ] **Step 2: Тест акта** (`crates/mw-core/tests/documents.rs`, по образцу `issue_act_prints_an_order_not_issued_yet_without_the_issue_date`), `issue_act_due_ignores_the_payment_at_issue` (Review Focus 2):
  - предоплата 1 500, итог 5 000, скидка 500, выдача с оплатой 3 000;
  - в акте выдачи — `Предоплата{sum("1 500_руб.")}` и `К оплате{sum("3 000_руб.")}`.
- [ ] **Step 3: `api_types.rs`:** `check_patch::<IssueRequest>()`.
- [ ] **Step 4: Тесты API** (`orders.rs`):
  - в `edits_status_issue_delete_with_versions` у двух `POST …/issue` тело `Some(json!({}))`;
  - новый `issue_with_payment`: `{"payment": 300000}` → 200, `GET …/payments` → строка `at_issue`;
  - `{"payment": 0}` → 422 `payment`/`out_of_range`;
  - `{"bogus": 1}` → 400;
  - без тела → 400 (заголовок `Content-Type: application/json` есть).
- [ ] **Step 5: Запустить** `cargo test -p mw-core --test order_payments` → не собирается.
- [ ] **Step 6: Реализовать**, обновить контракт. В интерфейсе — `issueOrder` с телом. В сквозных тестах — `data: {}` у шести вызовов `/issue`.
- [ ] **Step 7: Запустить** → PASS на обеих СУБД:
  - `cargo test -p mw-core --test order_payments --test documents --test orders_update --test api_types`;
  - `cargo test -p mw-api --test orders --test contract`.
  Затем полный Rust → PASS; `cd apps/ui && npm run verify && npm run test:e2e` → PASS.
- [ ] **Step 8: Коммит** `feat(orders): выдача с оплатой и этапы платежей`.

### Task 5: «Оплачено» и «Остаток» в списке, Excel и документах

Правила AGENTS.md: данные заказа в документе — только значения `data.json`; рендер — только `Core::export_*`; модель шаблона не меняется; контракт — командой.

**Files:**
- Modify: `crates/mw-core/src/orders/list.rs` (`OrderListItem.paid`; `select_items` дописывает суммы через `paid_by_order` из задачи 2)
- Modify: `crates/mw-core/src/export/orders.rs` (`COLUMNS: [&str; 26]`, `order_row`)
- Modify: `crates/mw-core/src/documents.rs` (`field_values` берёт `paid`; `order_data(…, paid)`; `amount_remaining`)
- Modify: `crates/mw-docs/src/fields.rs` (два поля, `FieldDef` 39 → 41)
- Test: `crates/mw-core/tests/orders_list.rs`, `crates/mw-core/tests/exports.rs`, модульные тесты `crates/mw-core/src/documents.rs` и `crates/mw-docs/src/fields.rs`
- Modify: `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`
- Modify (интерфейс — только типы): `apps/ui/src/lib/app/palette.ts` (`listItemOf`: `paid: o.prepayment`, решение 20), `apps/ui/src/lib/demo/fake.ts` (`paid: prepayment`). Фикстуры `OrderListItem` в тестах Vitest — `paid: 0`:
  - `apps/ui/src/lib/app/fixtures.test-helper.ts` (`item()`, рядом с `prepayment: 0`; им пользуется и `list.test.ts`);
  - `apps/ui/src/lib/demo/fake.test.ts` (строка с `prepayment: 0`).

**Interfaces:**
- Consumes: `paid_by_order`, `sum_kopecks` (задача 2).
- Produces (`OrderListItem`): `/// Оплачено: все платежи заказа, в копейках. pub paid: Money`. Строится только в `select_items` (его зовут `list_orders`, `list_orders_capped`, `find_orders_by_barcode`): сначала строки, затем `paid_by_order` по их номерам.
- Produces (Excel): после «К оплате» — «Оплачено» (`Cell::Money(item.paid)`) и «Остаток»:
  - в копейках: `price.kopecks().checked_sub(discount.kopecks()).and_then(|v| v.checked_sub(paid.kopecks()))`, обёрнутое в имеющийся `checked(…)`, как «К оплате»;
  - без итога — `Cell::Empty`.
- Produces (документы):
  - `field_values` для заказа: `let paid = paid_by_order(&self.db, &[id]).await?.remove(&id).unwrap_or(Money::ZERO);`;
  - `order_data(&order, paid, &client, &point, &settings, today)`;
  - `("order.paid", positive(paid))`;
  - `("order.remaining", amount_remaining(order.final_price, order.discount, paid).map(money).unwrap_or_default())`;
  - `fn amount_remaining(final_price: Option<Money>, discount: Money, paid: Money) -> Option<Money>` — как `amount_due`: `Money::from_kopecks(price.kopecks().saturating_sub(discount.kopecks()).saturating_sub(paid.kopecks()))`.
- Produces (`mw-docs`), после `order.due`:
  - `field("order.paid", "заказ.оплачено", "Оплачено", "500 руб.", "")`;
  - `field("order.remaining", "заказ.остаток", "Остаток", "1 300 руб.", "")`.
  Пятый аргумент `field` — текст пустого поля (`&str`). Здесь `""`, как у `order.prepayment` и `order.due`. Виды документов — те же, что у `order.due` в `fields_for`.

- [ ] **Step 1: Тесты:**
  - `orders_list.rs`, `list_rows_carry_the_paid_sum`:
    - заказ A с предоплатой 500 и платежом 700 → `paid` 1 200, `prepayment` 1 200;
    - заказ B без платежей → `paid` 0;
    - `find_orders_by_barcode(A.barcode)` → `paid` 1 200.
  - `exports.rs`, `orders_export_follows_the_list_filter`:
    - предоплата — через `NewOrder.prepayment` (`create_order_with`), не через `OrderPatch`;
    - `ORDER_COLUMNS` из 26 имён;
    - у заказа с предоплатой 300 и итогом 900: «К оплате» 600, «Оплачено» 300, «Остаток» 600;
    - у заказа без итога «Остаток» пуст.
  - `documents.rs` (модуль):
    - `every_field_is_filled_from_a_full_order` — `("order.paid", "500\u{a0}руб.")`, `("order.remaining", "1\u{202f}300\u{a0}руб.")`;
    - `amount_remaining_is_the_final_price_less_discount_and_payments` — `(Some(2000), 200, 500)` → 1 300; `(None, …)` → `None`; `(Some(1000), 0, 1300)` → −300.
  - `fields.rs`: `link_fields_are_not_static` — `FIELDS.len() == 41`; новые ключи есть в `fields_for(DocumentKind::IssueAct)`.
- [ ] **Step 2: Запустить** → не собирается:
  - `cargo test -p mw-core --test orders_list --test exports`;
  - `cargo test -p mw-core --lib documents`;
  - `cargo test -p mw-docs --lib fields`.
- [ ] **Step 3: Реализовать**, обновить контракт.
- [ ] **Step 4: Запустить** те же команды с `MW_TEST_PG_URL` и `cargo test -p mw-docs` → PASS; полный Rust → PASS; `cd apps/ui && npm run verify && npm run test:e2e` → PASS. `e2e/exports.spec.ts` заголовков Excel не перечисляет, его не трогать.
- [ ] **Step 5: Коммит** `feat(orders): «Оплачено» и «Остаток» в списке, Excel и полях документов`.

### Task 6: Блок «Платежи» вместо поля «Предоплата»

Правила AGENTS.md:
- Svelte 5 на рунах, обработчики — пропсы `on…`;
- типы — `import type` из `$api/...`;
- деньги — `MoneyInput` и `money.js`, даты — `date.js`;
- тексты — `ru.ts`, иконки — по одной;
- Ctrl/⌘+Enter — `submitShortcut`;
- правка без изменений версии не создаёт;
- контракт — командой.

**Files:**
- Modify: `crates/mw-core/src/orders/types.rs`:
  - `OrderPatch.prepayment` убрать — и поле, и `.field("prepayment", …)` в ручном `impl Debug for OrderPatch`;
  - `Order.prepaid_at` убрать: поле, `From`, `.field("prepaid_at", …)` в `impl Debug for Order`, модель модульного теста.
- Modify: `crates/mw-core/src/orders/update.rs`. В `update_order_in` убрать:
  - `apply!(… prepayment …)`;
  - `("prepayment", …)` в проверке `< 0`;
  - блок `prepaid_at`;
  - `.col_expr(order::Column::PrepaidAt, …)` и `.col_expr(order::Column::Prepayment, …)`.
- Test: `crates/mw-core/tests/orders_update.rs` (`prepayment_and_clearing_rules` → `clearing_rules`; новый тест), `crates/mw-api/tests/orders.rs`
- Modify: `crates/mw-api/openapi.json`, `crates/mw-api/bindings/`
- Create: `apps/ui/src/lib/orders/payments.svelte.ts`, `apps/ui/src/lib/orders/OrderPayments.svelte`, `apps/ui/src/lib/orders/PaymentDialog.svelte`
- Modify: `apps/ui/src/lib/api/orders.ts` (`listOrderPayments`, `addOrderPayment`, `removeOrderPayment`)
- Modify: `apps/ui/src/lib/orders/OrderFields.svelte`:
  - без `prepayment` в `MONEY`, `LABELS`, `HELP`;
  - без подсказки `prepaid_at`;
  - без строки «К оплате» и `toPayNegative`.
- Modify: `apps/ui/src/lib/orders/editor.svelte.ts` (`ORDER_FIELDS` без `'prepayment'`), `apps/ui/src/lib/app/fixtures.test-helper.ts` (без `prepaid_at`)
- Modify: `apps/ui/src/lib/orders/view.ts` (`remaining`), `apps/ui/src/lib/orders/history.ts` (иконка `Banknote`)
- Modify: `apps/ui/src/lib/orders/OrderPanel.svelte`:
  - состояние платежей;
  - блок;
  - `OrderHistory refresh`.
- Modify: `apps/ui/src/routes/(app)/orders/[id]/+page.svelte`:
  - состояние платежей;
  - блок в сниппете `fields`;
  - `paymentsRevision` в `$effect` журнала.
- Modify: `apps/ui/src/lib/i18n/ru.ts`:
  - строки блока и диалога;
  - `help.order.payments`;
  - правка `help.intake.prepayment` и `help.order.finalPrice`;
  - удалить ставшие ненужными `order.prepaidAt`, `order.toPayNegative` и `help.order.prepayment`;
  - метка `order.prepayment` («Предоплата») ещё нужна `IssueDialog` и уходит в задаче 7;
  - `order.toPay` остаётся (строка «К оплате» в «Выдать»).
- Test: `apps/ui/src/lib/orders/payments.test.ts`, `OrderPayments.test.ts`, `PaymentDialog.test.ts`, `view.test.ts`, `history.test.ts`
- Modify (сквозные): `apps/ui/e2e/order-panel.spec.ts` (тест «prepayment of an order in work can be changed and «К оплате» follows», строки 363–391, удалить — замена в задаче 8), `apps/ui/e2e/settings.spec.ts` (строки 389–390)

**Interfaces:**
- Consumes: `OrderPayments`, `NewOrderPayment` (`$api/...`); `MasterList`/`listMasters` (`masters.ts`); `app.me`; `serverClock`; `isoDay`, `formatDayMonth`, `formatDate`.
- Produces (`#lib/api/orders.js`):
  - `listOrderPayments(orderId)` → `GET /orders/{id}/payments`;
  - `addOrderPayment(orderId, n: NewOrderPayment)` → `POST`;
  - `removeOrderPayment(orderId, paymentId)` → `DELETE`.
- Produces (`view.ts`): `remaining(o: { final_price: number | null; discount: number }, paid: number): number | null` — без итога `null`.
- Produces (`payments.svelte.ts`), по образцу `createOrderParts` (`parts.svelte.ts`):
  ```ts
  export interface OrderPaymentsState {
  	/** The order these payments belong to: an undo of another order's issue does not read them. */
  	readonly orderId: number;
  	readonly payments: readonly OrderPayment[];
  	readonly paid: number;
  	readonly prepayment: number;
  	readonly loading: boolean;
  	readonly error: string | null;
  	readonly adding: boolean;
  	/** Grows with every payment taken or removed: the journal of the order asks again. */
  	readonly revision: number;
  	isRemoving(paymentId: number): boolean;
  	load(): Promise<void>;
  	/** Takes a payment; pressed again before the answer — the same request and key. Throws the server's error. */
  	add(p: Omit<NewOrderPayment, 'request_key'>): Promise<void>;
  	/** Removes a payment; one already gone (404) counts as done, the list is read again. */
  	remove(paymentId: number): Promise<void>;
  	/** The order is left: an answer that comes later is ignored (as `detach` of `OrderPartsState`). */
  	detach(): void;
  }
  export function createOrderPayments(orderId: number): OrderPaymentsState;
  ```
  Ключ — новый на каждый успешный приём; после потери связи остаётся тот же (как у запчастей).
- Produces (`OrderPayments.svelte`): `Section` «Платежи» с `region` и `help`. Пропсы: `payments: OrderPaymentsState`, `order: Order` (живой `editor.current`), `masters: MasterList`, `canEdit: boolean`.
  - Строки — «12.11 · 1 500 ₽ · Иванов»: день — `formatDayMonth`, другого года — `formatDate`; имя — если не пусто.
  - Под строками:
    - «Оплачено N»;
    - «Остаток N» — или «Переплата N», или «Итог не указан» (решение 21).
  - Пусто — «Платежей нет».
  - При `canEdit` и неудалённом заказе:
    - кнопка «Принять оплату…»;
    - у строки `IconButton` «Удалить платёж» → `ConfirmDialog` «Удалить платёж 1 500 ₽ от 12.11?».
- Produces (`PaymentDialog.svelte`): заголовок «Оплата по заказу № N», поля:
  - «День» — `DateInput`, `min` — день приёма, `max` — сегодня по `serverClock`, по умолчанию сегодня;
  - «Сумма» — `MoneyInput`, по умолчанию остаток, если он больше 0, иначе пусто;
  - «Кто принял» — `Combobox` с подсказками из активных мастеров, по умолчанию `app.me.open_mode ? '' : app.me.name`.
  «Принять» и Ctrl/⌘+Enter (`submitShortcut`). Ошибки 422 — под своими полями: `day`, `amount`, `received_by`.
- Produces (панель, `OrderPanel.svelte`):
  - в `swap()` рядом с `parts`: `payments?.detach(); payments = next ? createOrderPayments(next.order.id) : null; void payments?.load();`;
  - блок `OrderPayments` — сразу после `OrderFields`, перед `OrderComments`: «Остаток» стоит у денежных полей;
  - журнал панели: `OrderHistory refresh={(parts?.revision ?? 0) + (payments?.revision ?? 0)}`. `refresh=` есть только у панели.
- Produces (карточка, `routes/(app)/orders/[id]/+page.svelte`):
  - в `swap()` — то же, что в панели;
  - блок `OrderPayments` — в сниппете `fields` сразу после `OrderFields`, перед `OrderComments`. Сниппет — главная вкладка «Заказ»: блок виден при обеих раскладках (`display.cardColumns` и вкладки). Он не ставится в две ветки, как `OrderParts` (строки 286 и 362 по `display.cardColumns`);
  - журнал карточки обновляет не `OrderHistory refresh`, а `$effect` по `version` и `partsRevision`, который зовёт `journal.refresh(n)`. Рядом с `partsRevision` — `const paymentsRevision = $derived(payments?.revision ?? 0);`, в `$effect` — `void paymentsRevision;`.
- Produces (`history.ts`): `order.payment_added` и `order.payment_removed` — иконка `Banknote`.

- [ ] **Step 1: Тесты Rust:**
  - `orders_update.rs`:
    - `prepayment_and_clearing_rules` → `clearing_rules`: только очистка `final_price` (`Some(None)`) и прежние проверки оценки;
    - новый `an_edit_after_a_payment_keeps_the_recounted_prepayment` (Review Focus 5): платёж 700 → `update_order` модели со старой версией → `prepayment` 700, версия +1, платёж на месте.
  - `crates/mw-api/tests/orders.rs`, новый `prepayment_is_not_patched`:
    - `PATCH /orders/{id}` с `{"prepayment": 100}` → 400;
    - ответ `GET /orders/{id}` без ключа `prepaid_at`.
- [ ] **Step 2: Тесты Vitest:**
  - `view.test.ts`, `remaining`: `(3500, 500, 1000)` → 2000; без итога → `null`; переплата → отрицательное.
  - `payments.test.ts`:
    - `loads rows and sums`;
    - `a press before the answer is the same request`;
    - `a lost connection repeats with the same key, a success takes a new one`;
    - `remove of a gone payment reads the list again`;
    - `revision grows on add and remove`;
    - `after detach a late answer changes nothing`.
  - `OrderPayments.test.ts`:
    - `shows rows, paid and remaining`;
    - `overpayment shows «Переплата»`;
    - `without a final price it says «Итог не указан»`;
    - `without orders_edit there is no «Принять оплату…» and no delete`;
    - `delete asks first`.
  - `PaymentDialog.test.ts`:
    - `starts with today, the remaining and the signed-in name`;
    - `open mode starts with an empty «Кто принял»`;
    - `a day after today is not sent`;
    - `the server's payment_day goes under «День»`;
    - `Ctrl+Enter takes the payment`.
  - `history.test.ts`: `payment actions have the banknote icon`.
- [ ] **Step 3: Запустить** → падает:
  - `cargo test -p mw-core --test orders_update`;
  - `cd apps/ui && npx vitest run src/lib/orders/payments.test.ts src/lib/orders/OrderPayments.test.ts src/lib/orders/PaymentDialog.test.ts src/lib/orders/view.test.ts src/lib/orders/history.test.ts`.
- [ ] **Step 4: Реализовать.** Затем:
  - обновить контракт;
  - `ru.ts`:
    - `help.intake.prepayment` — «Сколько клиент заплатил при приёме.» / «Сумма станет первым платежом заказа сегодняшним днём. При выдаче её вычитают: «К оплате» = итог − предоплата − скидка.»;
    - `help.order.finalPrice` — «Остаток = итог − скидка − оплачено.»;
    - `help.order.payments` — что такое платёж, «Оплачено», «Остаток»; ошибочный платёж удаляют и вносят заново; в акте выдачи «К оплате» — сумма до оплаты при выдаче.
  - сквозные тесты:
    - тест правки предоплаты в `order-panel.spec.ts` удалить;
    - в `settings.spec.ts` вместо поля «Предоплата» и строки «К оплате» — `panel.getByRole('region', { name: 'Платежи', exact: true })` с текстами «Оплачено 500 $» и «Остаток 3 000 $».
- [ ] **Step 5: Запустить** те же команды → PASS; полный Rust → PASS; `cd apps/ui && npm run verify && npm run test:e2e` → PASS.
- [ ] **Step 6: Коммит** `feat(ui): блок «Платежи» в заказе вместо поля «Предоплата»`.

### Task 7: «Принять оплату» при выдаче и столбцы списка

Правила AGENTS.md:
- «Выдать» из панели — `registerHotkey` без `allowWithOverlay` (не трогается);
- Ctrl/⌘+Enter — `submitShortcut`;
- вид списка живёт в адресе (`viewToSearch`): столбцы — как прежние скрытые;
- запросы — по ролям и подписям.

**Files:**
- Modify: `apps/ui/src/lib/orders/IssueDialog.svelte` (`paid`, строка «К оплате», переключатель и сумма; `IssueValues.payment`)
- Modify: `apps/ui/src/lib/orders/actions.svelte.ts`:
  - `OrderActions(editor, payments)`, геттер `paid`;
  - `issue` всегда через `issueOrder`;
  - перечитать платежи; уведомление при отмене.
- Modify: `apps/ui/src/lib/orders/OrderDialogs.svelte` (`paid={actions.paid}`), `apps/ui/src/lib/orders/OrderPanel.svelte`, `apps/ui/src/routes/(app)/orders/[id]/+page.svelte` (второй аргумент `OrderActions`)
- Modify: `apps/ui/src/lib/orders/events.ts` (`emitOrderPaid`, `onOrderPaid`), `apps/ui/src/lib/orders/payments.svelte.ts` (зовёт `emitOrderPaid` после чтения, приёма и удаления), `apps/ui/src/lib/orders/list.svelte.ts` (`withPaid`), `apps/ui/src/routes/(app)/orders/+page.svelte` (подписка; ячейки `paid`, `remaining`)
- Modify: `apps/ui/src/lib/orders/columns.ts` (`'paid'`, `'remaining'`), `apps/ui/src/routes/design/sections/Table.svelte` (те же ячейки)
- Modify: `apps/ui/src/lib/i18n/ru.ts`:
  - подписи столбцов, переключателя и поля суммы, текст уведомления;
  - удалить метку `order.prepayment` («Предоплата»): после задачи 7 её никто не зовёт. Метка `prepayment` формы приёма — другой ключ, она остаётся.
- Test: `apps/ui/src/lib/orders/IssueDialog.test.ts` (новый), `actions.test.ts`, `columns.test.ts` (три прежних списка и новый тест), `events.test.ts`, `list.test.ts`, `payments.test.ts`
- Modify (сквозные): `apps/ui/e2e/order-panel.spec.ts` (тест «issue fills required fields in the dialog, issues and undo brings it back», строки 485–530)

**Interfaces:**
- Consumes: `IssueRequest` (`$api/IssueRequest`), `issueOrder(id, version, req)` (задача 4), `OrderPaymentsState` (задача 6), `remaining` (`view.ts`).
- Produces (`IssueDialog`):
  - проп `paid: number`;
  - `pay = price === null ? null : price − discount − paid`, где итог и скидка — значения диалога;
  - строка «итог − оплачено − скидка» вместо «итог − предоплата − скидка» (`t.payments.paid.toLowerCase()` вместо `t.prepayment.toLowerCase()`);
  - при `pay > 0`:
    - переключатель `Switch` с подписью «Принять оплату» (без суммы в подписи), включён при открытии;
    - рядом отдельное поле `MoneyInput` с подписью «Сумма оплаты», по умолчанию `pay`, правится; при выключенном переключателе поле недоступно;
    - сумма ≤ 0 при включённом переключателе → ошибка поля «Сумма оплаты — больше нуля», выдачи нет;
  - при `pay ≤ 0` или без итога нет ни переключателя, ни поля;
  - `IssueValues.payment: number | null` — `null`, когда переключатель выключен или его нет.
- Produces (`OrderActions`):
  - `constructor(editor: () => OrderEditor | null, payments: () => OrderPaymentsState | null = () => null)`;
  - `get paid(): number` — `payments()?.paid ?? editor()?.order.prepayment ?? 0`;
  - `issue(values)`: `issueOrder(id, v, { ...(target === null ? {} : { status_id: target }), ...(values.payment === null ? {} : { payment: values.payment }) })`, затем `void this.#reloadPayments(id)`;
  - `#reloadPayments(id)`: `const p = this.payments(); if (p && p.orderId === id) await p.load();` — платежи перечитываются, только если блок показывает этот заказ;
  - откат: сумма платежа передаётся в замыкание — `undo.push(…, () => this.#undoIssue(id, before.status, clearIssuedAt, values.payment))`.
  - `#undoIssue(id, statusId, clearIssuedAt, payment: number | null)` — обе прежние ветки:
    - заказ открыт в редакторе — `ed.run(changeStatus…)`;
    - иначе — `revertIssue(id, …)`.
    После любой ветки:
    - `await this.#reloadPayments(id)`;
    - при `payment !== null` — `toast` `t.payments.stayed(formatMoney(payment))` (решение 7). Уведомление показывается в обеих ветках, и когда пользователь уже ушёл к другому заказу.
- Produces (`events.ts`): `emitOrderPaid(orderId: number, sums: { prepayment: number; paid: number })` и `onOrderPaid(fn) → unsubscribe`, по образцу `emitOrderChanged`.
- Produces (`list.svelte.ts`): `withPaid(row: OrderListItem, sums): OrderListItem` — меняет `prepayment` и `paid`.
- Produces (`columns.ts`): `ORDER_COLUMN_IDS` — `'paid'` и `'remaining'` сразу после `'prepayment'`: скрыты по умолчанию, ширина 120, выравнивание `end`, в `SORT_FIELD` их нет. Подписи «Оплачено» и «Остаток».
  - Ячейка «Остаток» — `remaining(row, row.paid)`: `null` — пусто, отрицательный — с минусом.

- [ ] **Step 1: Тесты Vitest:**
  - `IssueDialog.test.ts`:
    - `offers to take the rest`: итог 3 500, оплачено 500 → «К оплате 3 000 ₽», переключатель «Принять оплату» включён, в поле «Сумма оплаты» — 3 000;
    - `no switch when nothing is due`: оплачено 3 500 → нет ни переключателя, ни поля «Сумма оплаты»;
    - `the amount can be changed`: 1 000 → `onissue` с `payment: 100000`;
    - `switched off sends no payment`;
    - `a zero amount is refused`;
    - `the line says «оплачено», not «предоплата»`.
  - `actions.test.ts`:
    - `issue sends the payment to /issue`;
    - `a picked issued status goes to /issue with status_id`;
    - `payments are read again after the issue`;
    - `payments of another order are not read`;
    - `undo tells that the payment stayed` — в обеих ветках: заказ открыт и заказ уже другой (`revertIssue`);
    - `undo without a payment shows no extra toast`.
    Прежние тесты `new OrderActions(() => ed)` не меняются.
  - `columns.test.ts`:
    - прежние три утверждения обновить:
      - `shows the main columns and hides the rest` — в списке скрытых после `'prepayment'` идут `'paid'`, `'remaining'`, затем `'barcode'`;
      - `sorts by the columns the server sorts by, and only those` — `SORT_FIELD` прежний; цикл `sortable` проверяет, что у новых столбцов сортировки нет;
      - `headers are in Russian, every column has its cell` — после «Предоплата» идут «Оплачено», «Остаток», затем «Штрихкод»;
    - новый `paid and remaining are hidden by default and not sortable` — у обоих `align` `'end'`.
  - Фикстуры `OrderListItem` (`fixtures.test-helper.ts` `item()`, `fake.test.ts`) уже получили `paid: 0` в задаче 5. Новых полей здесь нет.
  - `events.test.ts`: `onOrderPaid gets the sums and unsubscribes`.
  - `list.test.ts`: `withPaid changes prepayment and paid only`.
  - `payments.test.ts`: `emits the sums after load, add and remove`.
- [ ] **Step 2: Запустить** `cd apps/ui && npx vitest run src/lib/orders` → падает.
- [ ] **Step 3: Реализовать.** Сквозной тест выдачи (`order-panel.spec.ts`, 485–530), после ввода итога 3500:
  - «К оплате 3 000 ₽»;
  - `dialog.getByRole('switch', { name: 'Принять оплату', exact: true })` включён;
  - `dialog.getByRole('textbox', { name: 'Сумма оплаты' })` — «3 000»;
  - после «Отменить» в уведомлениях — «Платёж 3 000 ₽ остался в заказе».
- [ ] **Step 4: Запустить:**
  - `cd apps/ui && npm run verify && npx playwright test e2e/order-panel.spec.ts` → PASS в Chromium и WebKit;
  - затем `npm run test:e2e` → PASS;
  - полный Rust → PASS.
- [ ] **Step 5: Коммит** `feat(ui): «Принять оплату» при выдаче, столбцы «Оплачено» и «Остаток»`.

### Task 8: Сквозной тест платежей и документы

Правила AGENTS.md:
- сквозные тесты — Chromium и WebKit, по ролям и подписям, на настоящем `mw-server`;
- данные — через API с меткой `uniq()`;
- документация — по-русски;
- спецификация и планы — в `docs/superpowers/`.

**Files:**
- Create: `apps/ui/e2e/order-payments.spec.ts`
- Modify: `docs/superpowers/plans/README.md`:
  - строка 8.7 дорожной карты (строка 33) уже есть — заменить: ссылка на план, число задач (8), «выполнен»;
  - заготовка «План 8.7 (платежи по заказу)» в «Заметках для следующих планов» (строка 218, один пункт «несколько платежей…») уже есть — заменить её содержимое, новый раздел не добавлять;
  - «Идеи для программы → Склад, цены и расчёты» — дописать пункты.
- Modify: `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` — в §5.1 абзац «Как сделано в плане 8.7» со ссылкой на `2026-11-25-order-payments-design.md`.

**Interfaces:**
- Consumes: всё из задач 1–7; `createOrder`, `patchOrder`, `masterId`, `statusId`, `uniq`, `sessionToken` (`e2e/support/api.ts`); `GET /api/v1/auth/me` (`name`).

- [ ] **Step 1: Сквозной тест** (`order-payments.spec.ts`, 1280×720). Основной сервер — 4174 (с администратором); вход и API-контекст — как в `order-parts.spec.ts`.
  - `payments are taken, issued with the rest and survive a second issue`. Подготовка через API:
    - заказ с моделью `uniq('M')`, статус «Готов», предоплата 500 ₽;
    - `patchOrder`: итог 3 500 ₽, работы, мастер `masterId(…, 'Тестов')`.
    Шаги:
    1. Панель заказа → `region` «Платежи»: одна строка с «500 ₽», «Оплачено 500 ₽», «Остаток 3 000 ₽».
    2. «Принять оплату…» → диалог «Оплата по заказу № N»:
       - «День» — сегодня;
       - «Кто принял» — имя из `/api/v1/auth/me`;
       - «Сумма» — «3 000».
       Ввести 1 000, «Кто принял» — «Иванов», «Принять» → строка «1 000 ₽ · Иванов», «Оплачено 1 500 ₽», «Остаток 2 000 ₽».
    3. «История» заказа — «Принята оплата 1 000 ₽ (…) · Иванов».
    4. Review Focus 5: поменять «Модель» в панели → «Сохранено», без «Изменено в другом месте»; `fetchOrder(…).prepayment` = 150000.
    5. «Выдать» → «К оплате 2 000 ₽». Переключатель `switch` «Принять оплату» включён, в поле «Сумма оплаты» — «2 000». Ctrl/⌘+Enter → «Оплачено 3 500 ₽», «Остаток 0 ₽». Через API последний платёж — `at_issue`.
    6. Review Focus 1:
       - «Ещё: Выдан» → «Вернуть в работу» → «Готов»;
       - «Выдать» → «К оплате 0 ₽»; нет ни переключателя «Принять оплату», ни поля «Сумма оплаты»;
       - Escape.
    7. Удалить платёж 1 000: кнопка «Удалить платёж» в его строке → «Удалить платёж 1 000 ₽ от …?» → «Удалить» → «Оплачено 2 500 ₽», «Остаток 1 000 ₽».
    8. Полная карточка `/orders/{id}` при 1280×720 (две колонки, `display.cardColumns`):
       - `region` «Платежи» в колонке «Заказ» (сниппет `fields`) с теми же суммами;
       - «Принять оплату…» 100 → в `region` «История изменений» без перезагрузки — «Принята оплата 100 ₽». Журнал карточки обновляет `$effect` по `paymentsRevision`.
       Затем `page.setViewportSize({ width: 900, height: 800 })` → раскладка вкладками: вкладка «Заказ» выбрана, `region` «Платежи» виден в ней («Оплачено 2 600 ₽»).
  - `the list shows Оплачено and Остаток`:
    - заказ с предоплатой 300 ₽ и итогом 900 ₽;
    - в меню столбцов списка (как в `e2e/table.spec.ts`) включить «Оплачено» и «Остаток» → в строке «300» и «600»; в адресе — столбцы вида;
    - панель → «Принять оплату…» 200 → закрыть панель → в строке «500» и «400» без перезагрузки (`emitOrderPaid`).
  - `in open mode «Кто принял» starts empty` (сервер 4173, `test.use({ baseURL: 'http://127.0.0.1:4173' })` в своём `describe`):
    - заказ через API, панель → «Принять оплату…»;
    - `combobox` «Кто принял» пуст;
    - сумма 100 → «Принять» → строка без имени.
- [ ] **Step 2: Запустить** `cd apps/ui && npx playwright test e2e/order-payments.spec.ts` → PASS в Chromium и WebKit. Тест пишется на готовый интерфейс; если упал, ошибку чинят в задаче, где она сделана.
- [ ] **Step 3: Документы.**
  - README, «Идеи для программы → Склад, цены и расчёты» — шесть пунктов «Вне плана» проекта решения:
    - касса и смены;
    - способ оплаты (наличные, карта, перевод);
    - возврат денег клиенту;
    - отчёт «Поступления за период» по дням платежей;
    - право «касса» (отдельно от «Правки заказов»);
    - оплата при массовой выдаче.
  - README, «План 8.7»: что сделано — таблица `order_payments`, перенос предоплаты, выдача с оплатой и этапы, поля документов, столбцы. Вне плана — то же, что в «Идеях».
  - Спека §5.1, «Как сделано в плане 8.7»:
    - «Предоплата» в акте — платежи до выдачи;
    - «К оплате» = итог − предоплата − скидка, как раньше;
    - «Остаток» и «Оплачено» — в карточке и полях `{заказ.остаток}`, `{заказ.оплачено}`;
    - при возврате в работу все платежи становятся платежами до выдачи.
- [ ] **Step 4: Запустить** полный Rust → PASS; `cd apps/ui && npm run verify && npm run test:e2e` → PASS.
- [ ] **Step 5: Коммит** `test(e2e): платежи по заказу; docs: план 8.7 в спеке и идеях`.

---

## Самопроверка плана

- **Покрытие проекта решения:**

  | Решение проекта | Задачи |
  |---|---|
  | 1 — платёж: день, сумма, кто принял, этап, кто записал, `legacy` | 1, 2 |
  | 2 — этап выводится сам (уточнён «Решениями плана» 3) | 1, 2, 4 |
  | 3 — «кто принял»: имя вошедшего, подсказки мастеров, текстом | 2, 4, 6 |
  | 4 — суммы: `prepayment` = до выдачи, поле правки убрано, приём создаёт платёж, «Оплачено», «Остаток», «Переплата» | 2, 5, 6 |
  | 5 — выдача: переключатель «Принять оплату» и поле «Сумма оплаты», частичная оплата, одна транзакция, массовая без платежей | 4, 7 |
  | 6 — платёж не правится, удаление с подтверждением и журналом, `legacy` удаляются так же | 2, 6 |
  | 7 — права (уточнены «Решениями плана» 1) | 2, 4 |
  | 8 — журнал `order.payment_added`/`order.payment_removed`, выдача — одна запись | 2, 4, 6 |
  | 9 — версия заказа не меняется, блок перечитывает себя | 2, 6, 8 |
  | 10 — перенос: миграция и импорт, `prepaid_at` не пишется | 1, 2, 3 |
  | 11 — документы: смысл прежний, `{заказ.оплачено}`, `{заказ.остаток}` | 4, 5 |
  | 12 — где видно: блок, столбцы, Excel | 5, 6, 7 |
  | 13 — вне плана в «Идеи» | 8 |

- **Имена сквозь задачи:** `PaymentStage`, `MAX_PAYMENT`, `amount_in_range`, `order_payment`, `OrderPayment`, `OrderPayments`, `NewOrderPayment`, `IssueRequest`, `received_by_of`, `sum_kopecks`, `insert_payment`, `recount_prepayment`, `restage_payments`, `paid_by_order` (задача 2, зовётся в задаче 5), `issue_order_with`, `createOrderPayments`, `OrderPaymentsState` (`orderId`, `detach`), `remaining`, `emitOrderPaid`, `withPaid` объявлены там, где впервые нужны, и дальше зовутся так же.
- **Число операций API:** 128 → 131 (задача 2); задачи 4–6 меняют типы, но не число операций.
- **Каждая задача оставляет всё зелёным:**
  - задача 4 меняет тело `/issue` вместе со всеми вызывающими: тесты API, `issueOrder` интерфейса, шесть вызовов в сквозных тестах;
  - задача 6 убирает `OrderPatch.prepayment` и `Order.prepaid_at` вместе с полем интерфейса и сквозными тестами, которые его правили;
  - задача 5 добавляет `OrderListItem.paid` вместе с местами интерфейса, которые строят строку списка.
- **Промежуточные коммиты.** «Зелёное» — это тесты, а не инвариант «`prepayment` = сумма `before_issue`»: он держится, начиная с задачи 6 (Global Constraints). Порядок задач не меняется, ветка вливается целиком.
  - В задачах 2–5 поле «Предоплата» в карточке ещё пишет `orders.prepayment` мимо платежей.
  - Между задачами 2 и 3 импорт ещё не создаёт платёж.
- **Review Focus:** у каждого пункта есть тест в задаче, которая пишет этот код.
