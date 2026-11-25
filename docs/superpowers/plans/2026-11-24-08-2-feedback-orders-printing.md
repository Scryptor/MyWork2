# План 8.2. Доводка по отзывам: заказы и печать

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** закрыть отзывы на предрелизы 0.0.2–0.0.3 о заказах и печати. Что входит:
- смена точки у заказа и след перемещений;
- предоплата у заказа в работе;
- графический ключ;
- списки вместо рядов бейджей;
- краткий просмотр прошлых заказов;
- выбор страниц при печати;
- закреплённое меню настроек.

**Architecture:** почти всё — интерфейс поверх того, что сервер уже умеет (`OrderPatch.service_point_id`, `OrderPatch.prepayment`, `dictionary_usage`). Новое на сервере:
- запись журнала `order.moved`;
- столбец `orders.device_pattern`;
- поля документов «Пароль устройства» и «Графический ключ»;
- скидка в строке списка заказов;
- выбор страниц `pages` при выгрузке документа. Страницы отбирает сервер, поэтому команды печати Tauri не меняются.

**Tech Stack:** Rust (mw-domain, mw-db/SeaORM, mw-core, mw-api/Axum + utoipa + ts-rs, mw-docs/Typst 0.15.1), SvelteKit 3 + Svelte 5 (руны), bits-ui 2.21, Playwright.

**Spec:** пункты «Идеи для программы» в `docs/superpowers/plans/README.md`:
- «Смена точки у оформленного заказа»;
- «Перемещение заказа между точками»;
- «Предоплата у заказа в работе»;
- «Графический ключ при приёме»;
- «Умные списки в форме «Новый заказ»»;
- «Быстрый просмотр прошлых заказов при наведении»;
- «Выбор страниц при печати»;
- «Меню настроек не уезжает при прокрутке».

Правила интерфейса — `docs/superpowers/specs/2026-11-15-ui-design-system-design.md`.

## Решения плана

1. **Перемещение — своя запись журнала.**
   - Смена точки пишет `order.moved` с текстом `Перемещён: «Центр» → «Север»` (нет точки — `без точки`) и `details: {"from": id|null, "to": id|null}`.
   - В `order.updated` точка больше не попадает. Если в одной правке сменились точка и бренд, будут две записи.
   - Статус «в пути» и отчёт о перемещениях не делаются; это остаётся в README к плану 8.4.
2. **Предоплата — одна сумма**, как сейчас в базе. Несколько платежей с датами — к плану 8.4 «Отчёты» (README). Прочее уже есть:
   - журнал «Изменено: предоплата»;
   - право `orders_edit`;
   - «К оплате» = итог − предоплата − скидка;
   - в актах — `{заказ.предоплата}`.
3. **Графический ключ** хранится, как пароль: столбец `orders.device_pattern TEXT NOT NULL DEFAULT ''`, без шифрования.
   - В журнале вместо значения `***`, в `Debug` маска, в `search_text` ключа нет.
   - Значение — строка цифр 1–9 без повторов, пусто или от 4 до 9 точек.
   - Подсказка «хранится в зашифрованном виде» из README не делается.
   - В MyWork 2 ключа не было, импорт не меняется.
3а. **Пароль и ключ в документах** — только как поля шаблона, которые мастерская сама добавляет в свой акт. В стандартные акты по умолчанию не выводятся: акт отдают клиенту (пользователь 2026-11-24: «по умолчанию не должно попадать, но возможность добавить нужна»).
   - «Пароль устройства»: `order.password`, `{заказ.пароль}`.
   - «Графический ключ»: `order.pattern`, `{заказ.графический_ключ}`, текстом «1 · 5 · 8 · 9».
4. **Ввод ключа — щелчками по точкам сетки 3 × 3.** Переход через среднюю точку добавляет её, если она ещё не выбрана, как в Android. Ведение пальцем по сетке — на потом (README).
   - Поле ключа открывает всплывающее окно с сеткой.
   - Новое значение принимается кнопкой «Готово», если ключ пуст или в нём не меньше 4 точек. Закрытие без «Готово» ничего не меняет.
5. **Частота значений — по запросу, без таблицы частот и порога.** Используется уже готовый `Core::dictionary_usage`: один `GROUP BY` по заказам. Его право расширяется до `settings_manage` **или** `orders_create` **или** `orders_edit`.
   - Пока счёт не пришёл (или запрос не удался), список идёт в порядке справочника.
   - Карточка заказа по-прежнему правит «Комплектность» и «Состояние» текстом. Список в карточке — на потом (README).
   - «Частые неисправности» остаются бейджами: их всего 12.
6. **Краткий просмотр** — всплывающая карточка `HoverCard` (bits-ui `LinkPreview`) у строк заказов в карточке клиента.
   - На телефоне (`display.phone`) наведения нет: в строке появляется вторая строчка с итогом и датой выдачи.
   - В `OrderListItem` добавляется `discount`.
7. **Выбор страниц — параметр `pages` у выгрузки одного документа** (`GET /orders/{id}/documents/{kind}` и `/documents/{kind}`).
   - Формат: `1,3-4`, допускаются пробелы и «–».
   - Отбирает сервер: PDF через `PdfOptions.page_ranges`, PNG — по номерам страниц. Поэтому одинаково работают печать на Windows (PNG), macOS/Linux (PDF через `lp`) и «Сохранить PDF».
   - Выбор делается в предпросмотре: галочки «Страница N» у каждой страницы, если их больше одной.
   - «Печать ▾» без предпросмотра, Ctrl/⌘P и печать пачкой печатают все страницы, как раньше.
8. **Меню настроек** закреплено на широком окне (`position: sticky`). На узком и на телефоне всё как сейчас.

## Global Constraints

- Код и идентификаторы на английском; тексты для людей и документация на русском.
- Сначала тест, потом код. Каждый тест с БД идёт по `all_backends()`.
- Перед коммитом зелёное: `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace` (PostgreSQL — `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres`) и для интерфейса `cd apps/ui && npm run verify && npm run test:e2e`.
- Деньги — только `Money` (в интерфейсе копейки через `#lib/format/money.js` и `MoneyInput`); время — `Core::now()`; поиск без учёта регистра — `mw_domain::search` (в интерфейсе `normalizeSearch`).
- Выпущенные миграции не меняются, только добавляются новые. Любая запись в БД — только через `Core::begin_write()`.
- Типы API — типы `mw-core`/`mw-domain` с `Serialize`/`Deserialize`, `ts_rs::TS`, `utoipa::ToSchema`; входные — с `deny_unknown_fields`; «не передано» и `null` различают `patch::present`/`patch::nullable`; `#[ts(export)]` не ставить.
- `openapi.json` и `bindings/` после изменения API: `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`, затем `cargo test -p mw-api --test contract`; коммит вместе с кодом.
- Новые маршруты API не появляются; если понадобятся — в свой раздел `crates/mw-api/src/routes/mod.rs`.
- Документы: данные заказа попадают в Typst только через `data.json`; рендерит только `Core::export_document`.
- Правка без изменений не создаёт версию и запись в журнал.
- `search_text` не меняется: ключ и пароль в поиск не попадают, `SEARCH_INDEX_VERSION` не трогать.
- Интерфейс:
  - Svelte 5 на рунах, обработчики — пропсы `on…`;
  - импорты `#lib/...` с явными расширениями, типы API — `import type` из `$api/...`;
  - компоненты — в `#lib/ui/...` с разделом на витрине `/design`;
  - тексты — по-русски, общие строки — в `#lib/i18n/ru.ts`;
  - иконки — `@lucide/svelte`, по одной;
  - цвета и размеры — только из `tokens.css`;
  - ширины — только `display` и `:global([data-width=…])`, без медиазапросов по ширине;
  - даты — через `#lib/format/date.js`;
  - `fetch` — только в `src/lib/api/client.ts`;
  - сквозные тесты — Chromium и WebKit, на настоящем `mw-server`, запросы по ролям и подписям, данные через API с `uniq()`.
- Субагенты: `cargo`/`rustc` только в worktree `/Users/ruf/dev/MyWork2/.superpowers/worktrees/plan-8-2` с `CARGO_TARGET_DIR=/Users/ruf/dev/MyWork2/target`; Node — `/opt/homebrew/opt/node@24/bin`; долгие команды — фоном с уведомлением, без циклов `pgrep -f`; логи — в папке черновиков, не в `/tmp`.
- Коммиты кончаются строкой `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. **Две вкладки правят точку одного заказа.** Вторая получает спор (как у мастера), а не молча затирает первую. В журнале — одна `order.moved` на каждую сохранённую смену. → Task 2, e2e `the point of an order changes from the panel and history says where it went` и тест ядра `moving_and_editing_together_writes_both_entries`.
2. **Предоплата больше итога.** «К оплате» уходит в минус и показывает уже существующее предупреждение `toPayNegative`; сохранение не запрещено. → Task 3, e2e шаг с предоплатой 5000 при итоге 3000.
3. **Ключ из трёх точек и «перепрыгнутая» точка.** Ключ из трёх точек не уходит на сервер: «Готово» недоступно, а сервер отвечает 422 `device_pattern` `invalid`. Щелчки 1 → 3 дают `123`. → Task 4 `a_short_pattern_is_rejected`, Task 5 `pattern.test.ts` и `PatternInput.test.ts`.
4. **Сотни значений «Комплектности» после импорта.** Список рисует не больше 50 строк, остальное находится вводом. Отказ `dictionaryUsage` не ломает приём. → Task 6, `MultiCombobox.test.ts` (`shows at most the limit`) и `IntakeSheet` с отказом usage.
5. **Документ в одну страницу и страница за концом.** У документа в одну страницу галочек нет. Если снять все галочки, «Печать» и «Сохранить PDF» недоступны. `pages=5` у документа в одну страницу — 422 `pages` `out_of_range`. → Task 8, `DocumentPreview.test.ts` и `pages_beyond_the_document_are_refused`.

---

### Task 1: Закреплённое меню настроек

**Files:**
- Modify: `apps/ui/src/routes/(app)/settings/+layout.svelte` (стили `.sections`)
- Test: `apps/ui/e2e/settings.spec.ts`
- Modify: `docs/superpowers/plans/README.md` (пункт «Меню настроек не уезжает при прокрутке» → `[x]`, «Сделано в плане 8.2»)

**Interfaces:** нет.

- [ ] **Step 1: Тест.** В `settings.spec.ts` новый `test('the section menu stays in view while a long settings page scrolls')`:
  - окно 1280 × 720, страница `/settings/documents`;
  - прокрутить `main` до конца: `page.locator('main').evaluate((el) => el.scrollTo(0, el.scrollHeight))`;
  - `await expect(page.getByRole('link', { name: 'Документы и реквизиты' })).toBeInViewport()`.
  
  Второй тест `test('on a phone the section menu scrolls with the page')`: при 375 × 812 у `nav` раздела настроек вычисленный `position` не `sticky`.
- [ ] **Step 2:** `npx playwright test e2e/settings.spec.ts -g "section menu"` — первый тест падает.
- [ ] **Step 3: Стили.** Только на широком окне (не `.narrow`, не `.phone`):
  - у `.sections` — `position: sticky; top: <отступ сетки>; align-self: start`;
  - если меню выше видимой части, оно прокручивается само: `max-height` по высоте прокручиваемого `main` за вычетом отступов, `overflow-y: auto`;
  - размеры — только токены.
- [ ] **Step 4:** тесты проходят в Chromium и WebKit; `npm run verify` зелёный.
- [ ] **Step 5:** коммит `fix(ui): меню настроек закреплено при прокрутке`.

### Task 2: Смена точки у заказа и перемещения в истории

**Files:**
- Modify: `crates/mw-core/src/orders/update.rs` (точка уходит из меток `order.updated`, своя запись `order.moved`)
- Test: `crates/mw-core/tests/orders_update.rs`, `crates/mw-api/tests/orders.rs`
- Modify: `apps/ui/src/lib/orders/editor.svelte.ts` (`ORDER_FIELDS` + `'service_point_id'`), `apps/ui/src/lib/orders/OrderFields.svelte` (раздел «Работа»), `apps/ui/src/lib/orders/history.ts` (значок), `apps/ui/src/lib/i18n/ru.ts`
- Test: `apps/ui/src/lib/orders/history.test.ts`, `apps/ui/e2e/order-panel.spec.ts`
- Modify: `docs/superpowers/plans/README.md`:
  - «Смена точки у оформленного заказа» → `[x]`;
  - в «Перемещение заказа между точками» оставить нерешённое: «в пути» и отчёт (план 8.4); история уже сделана.

**Interfaces:**
- Produces: действие журнала `order.moved`, `summary` `Перемещён: «<из>» → «<в>»` (`без точки` без кавычек, когда точки нет), `details` `{"from": i64|null, "to": i64|null}`.
- Consumes: `OrderPatch.service_point_id: Option<Option<i64>>` (уже есть), проверка точки `update.rs:282-292` (уже есть).

- [ ] **Step 1: Тесты ядра** (`orders_update.rs`, `all_backends`). Точки создаются в тесте: «Центр» и «Север».
  - `moving_an_order_writes_its_own_history_entry`. Заказ в «Центре», правка `service_point_id: Some(Some(север))`. Последняя запись журнала заказа: `action == "order.moved"`, `summary == "Перемещён: «Центр» → «Север»"`, `details == json!({"from": центр, "to": север})`. Записи `order.updated` от этой правки нет.
  - `moving_and_editing_together_writes_both_entries`. Правка точки и `brand` сразу. Две новые записи: `order.updated` с `summary == "Изменено: бренд"` и `order.moved`. Версия заказа выросла на 1.
  - `clearing_the_point_says_without_point`. `Some(None)` → `summary == "Перемещён: «Центр» → без точки"`.
  - `setting_the_same_point_changes_nothing`. Правка на ту же точку не меняет версию и не пишет журнал.
  
  В `crates/mw-api/tests/orders.rs`: `patching_the_point_moves_the_order` — PATCH возвращает новую точку, `GET …/history` первой записью отдаёт `order.moved`.
- [ ] **Step 2:** `cargo test -p mw-core --test orders_update moving` — падают.
- [ ] **Step 3: Код.** В `update_order`:
  - точка сравнивается и применяется как раньше, но в `Diff` её метка не попадает;
  - при смене в той же транзакции после записи `order.updated` (если она есть) пишется запись `order.moved`;
  - имена точек читаются в этой же транзакции, архивные — тоже по имени.
- [ ] **Step 4: Тест интерфейса.** `history.test.ts`: у записи `order.moved` значок `MapPin`, а `text` — `summary` как есть.
- [ ] **Step 5: Интерфейс.**
  - `'service_point_id'` добавить в `ORDER_FIELDS`.
  - В разделе «Работа» `OrderFields.svelte` после «Мастера» — `Select` с подписью «Точка» (`id={idOf('service_point_id')}`). Сохраняется сразу через `autosave.commit`, как мастер; спор и вспышка — как у мастера; в тексте спора — имя точки.
  - Варианты — `app.activePoints`. Текущая архивная точка показывается недоступным вариантом `<имя> (в архиве)`, отсутствие точки — «Без точки».
  - Поле видно, если в программе есть точки (`app.points.length > 0`); при `readonly` недоступно.
  - Значок `MapPin` для `order.moved` в `history.ts`.
- [ ] **Step 6: e2e.** `order-panel.spec.ts`: `test('the point of an order changes from the panel and history says where it went')`.
  - Через API две точки `Центр <uniq>` и `Север <uniq>` и заказ в первой.
  - Открыть панель, выбрать в «Точка» вторую.
  - В шапке панели — `Север <uniq>`; в «Истории» — текст `Перемещён: «Центр <uniq>» → «Север <uniq>»`; после перезагрузки точка сохранена.
- [ ] **Step 7:** контракт (`MW_UPDATE_CONTRACT=1 …`, если OpenAPI изменился), весь набор Rust и `npm run verify` + e2e зелёные.
- [ ] **Step 8:** коммит `feat: смена точки у заказа и перемещения в истории`.

### Task 3: Предоплата у заказа в работе

**Files:**
- Modify: `apps/ui/src/lib/orders/editor.svelte.ts` (`ORDER_FIELDS` + `'prepayment'`), `apps/ui/src/lib/orders/OrderFields.svelte` (блок «Деньги»), `apps/ui/src/lib/i18n/ru.ts` (подсказка)
- Test: `apps/ui/e2e/order-panel.spec.ts`
- Modify: `docs/superpowers/plans/README.md` («Предоплата у заказа в работе» → `[x]`, решения 2 из этого плана; несколько платежей — в заметки плана 8.4)

**Interfaces:**
- Consumes: `OrderPatch.prepayment: Option<Money>` (сервер уже применяет, журнал «предоплата»), `toPay` из `#lib/orders/view.js`.

- [ ] **Step 1: e2e.** `test('prepayment of an order in work can be changed and «К оплате» follows')`:
  - заказ через API: итог 3000 ₽, предоплата 0;
  - в панели «Предоплата» = 1000 → «К оплате 2 000 ₽»;
  - перезагрузка → в поле «1 000», в «Истории» — `Изменено: предоплата`;
  - «Предоплата» = 5000 → видно существующее предупреждение об отрицательной сумме (`ru.order.toPayNegative`), значение сохраняется.
- [ ] **Step 2:** тест падает (поля нет).
- [ ] **Step 3: Код.**
  - `MONEY` = `['final_price', 'prepayment', 'discount', 'extra_costs']`, порядок полей в сетке такой же.
  - Подпись — `t.prepayment`. Подсказка `ru.help.order.prepayment`: «Сколько клиент внёс заранее. «К оплате» = итог − предоплата − скидка».
  - Под полем, если предоплата больше нуля и есть `prepaid_at`, — `hint` «внесена {formatRelative(prepaid_at)}».
  - Из приглушённой строки над сеткой сумма предоплаты уходит, ориентировочная стоимость остаётся.
  - `pay` считает от живого значения поля (`money('prepayment')`).
  - Невалидная сумма отбрасывается, как у скидки.
- [ ] **Step 4:** e2e в Chromium и WebKit и `npm run verify` зелёные.
- [ ] **Step 5:** коммит `feat(ui): предоплата правится у заказа в работе`.

### Task 4: Графический ключ — данные, журнал, поля документов

**Files:**
- Create: `crates/mw-domain/src/pattern.rs` (+ `pub mod pattern;` в `lib.rs`)
- Create: `crates/mw-db/src/migration/m20261124_000003_device_pattern.rs` (+ регистрация в `migration/mod.rs`)
- Modify: `crates/mw-db/src/entity/order.rs`, `crates/mw-core/src/orders/{types.rs,create.rs,update.rs}`, `crates/mw-core/src/import/write.rs` (пустой ключ), `crates/mw-core/src/documents.rs` (`order_data`), `crates/mw-docs/src/fields.rs`, тестовые помощники с полным `NewOrder`/моделью (`crates/mw-core/tests/common/mod.rs`, `crates/mw-api/tests/common/mod.rs`, `types.rs` тестовая модель)
- Test: `crates/mw-domain/src/pattern.rs` (модульные), `crates/mw-core/tests/{orders_update,orders_create,documents}.rs`, `crates/mw-docs/src/fields.rs` (тесты списка)
- Contract: `crates/mw-api/openapi.json`, `crates/mw-api/bindings/*`

**Interfaces:**
- Produces:
  - `mw_domain::pattern::{PATTERN_MIN_POINTS: usize = 4, validate_pattern(text: &str) -> Result<(), &'static str>, pattern_text(text: &str) -> String}`;
  - `Order.device_pattern: String`, `NewOrder.device_pattern: String` (обязательное поле тела, без `#[serde(default)]`), `OrderPatch.device_pattern: Option<String>` (`patch::present`);
  - *Поправка при исполнении:* сначала здесь стоял `#[serde(default)]`; контролёр отменил его (коммит `bab1b60`). `NewOrder` — только тело запроса, из хранилища его не читают, предрелизам совместимость не нужна, а все прежние поля `NewOrder` обязательны (`request_bodies_require_every_field`). Не возвращать.
  - поля документов `order.password` (`заказ.пароль`, «Пароль устройства», образец `1234`, пусто `""`) и `order.pattern` (`заказ.графический_ключ`, «Графический ключ», образец `1 · 5 · 8 · 9 · 6 · 3`, пусто `""`) сразу после `order.serial`.

- [ ] **Step 1: Тесты домена.**
  - `validate_pattern`: принимает `""`, `"1589"`, `"123456789"`; не принимает `"158"` (3 точки), `"1123"` (повтор), `"1580"` (`0`), `"a123"`, `"1234567891"` (10 знаков).
  - `pattern_text("158963") == "1 · 5 · 8 · 9 · 6 · 3"`, `pattern_text("") == ""`.
- [ ] **Step 2: Тесты ядра** (`all_backends`).
  - `orders_create.rs`: `a_new_order_keeps_its_pattern_key` — `device_pattern: "1596"` сохраняется; `NewOrder` без поля в JSON отклоняется (поле обязательно; по решению контролёра при исполнении вместо прежнего «читается с пустым ключом», см. *Interfaces*).
  - `orders_update.rs`: `the_pattern_key_is_saved_and_masked_in_history` — правка `"15963"` сохраняется; запись журнала `summary == "Изменено: графический ключ"`, `details.changes.device_pattern == json!(["***", "***"])`, в сериализованных `details` нет `"15963"`.
  - `orders_update.rs`: `a_short_pattern_is_rejected` — `"158"` → `CoreError::Validation` с полем `device_pattern` и кодом `invalid`; у создания то же.
  - `documents.rs`: `password_and_pattern_reach_the_document_data` — заказ с паролем `0000` и ключом `1236` → `values["order.password"] == "0000"`, `values["order.pattern"] == "1 · 2 · 3 · 6"`.
  - `documents.rs`: `standard_acts_do_not_print_password_or_pattern` — тот же заказ (пароль `Zx-0000-pw`, ключ `1236`), стандартные акты приёма и выдачи без сохранённых шаблонов: в тексте документа (`Rendered::text` или как уже проверяют текст в этом файле) нет `Zx-0000-pw` и нет `1 · 2 · 3 · 6`.
  - Модульный тест `debug_output_masks_device_pattern` в `types.rs`.
  - `fields.rs`: длина списка 38, ключи и заполнители уникальны (существующие тесты).
- [ ] **Step 3:** тесты падают.
- [ ] **Step 4: Код.**
  - Миграция: `orders.device_pattern TEXT NOT NULL DEFAULT ''` по образцу `m20261116_000001_order_keys`.
  - Поле в сущности, `Order`, `NewOrder`, `OrderPatch`, в списке столбцов `update.rs`, в `create.rs`, в маске `Debug`.
  - Проверка `validate_pattern` при создании и правке → `FieldError::new("device_pattern", "invalid")`.
  - В журнале — как пароль: `MASK`, метка «графический ключ».
  - Поля документов и `order_data`.
  - `search_text` не трогать.
- [ ] **Step 5:** контракт обновлён; полный набор Rust зелёный (обе базы).
- [ ] **Step 6:** коммит `feat: графический ключ устройства — хранение, журнал, поля документов`.

### Task 5: Графический ключ — ввод при приёме и в заказе

**Files:**
- Create: `apps/ui/src/lib/orders/pattern.ts`, `apps/ui/src/lib/ui/PatternInput.svelte`, `apps/ui/src/lib/ui/PatternField.svelte`
- Modify: `apps/ui/src/routes/design/+page.svelte` (раздел «Графический ключ»; путь витрины — по образцу других разделов), `apps/ui/src/lib/intake/draft.ts`, `apps/ui/src/lib/intake/IntakeSheet.svelte`, `apps/ui/src/lib/orders/editor.svelte.ts`, `apps/ui/src/lib/orders/OrderFields.svelte`, `apps/ui/src/lib/i18n/ru.ts`
- Test: `apps/ui/src/lib/orders/pattern.test.ts`, `apps/ui/src/lib/ui/PatternInput.test.ts`, `apps/ui/src/lib/intake/draft.test.ts`, `apps/ui/e2e/intake.spec.ts`
- Modify: `docs/superpowers/plans/README.md` («Графический ключ при приёме» → `[x]`; ведение пальцем и вывод ключа картинкой в акте — строкой «на потом»)

**Interfaces:**
- Consumes: `Order.device_pattern`, `NewOrder.device_pattern`, `OrderPatch.device_pattern` (Task 4).
- Produces:
  - `addPoint(value: string, point: number): string`, `patternText(value: string): string`, `patternReady(value: string): boolean` в `#lib/orders/pattern.js`;
  - `PatternInput` — пропсы `{ value = $bindable(''), disabled?: boolean, id?: string }`;
  - `PatternField` — пропсы `{ value: string, onchange: (value: string) => void, label: string, help?: string, disabled?: boolean, id?: string }`.

- [ ] **Step 1: Тесты.**
  - `pattern.test.ts`:
    - `addPoint('', 5) === '5'`;
    - `addPoint('1', 3) === '123'`;
    - `addPoint('21', 3) === '213'` (2 уже есть);
    - `addPoint('1', 9) === '159'`;
    - `addPoint('7', 3) === '753'`;
    - `addPoint('5', 5) === '5'`;
    - `patternText('1236') === '1 · 2 · 3 · 6'`;
    - `patternReady('123') === false`, `patternReady('1236') === true`.
  - `PatternInput.test.ts`:
    - щелчки по кнопкам «Точка 1», «Точка 3», «Точка 6», «Точка 9» → значение `12369`, текст `1 · 2 · 3 · 6 · 9`, «Ключ готов»;
    - «Отменить шаг» → `1236`, «Сбросить» → пусто;
    - после «Точка 1» — «Ещё 3 точки»;
    - выбранные точки — `aria-pressed="true"`.
  - `draft.test.ts`: `toNewOrder` передаёт `device_pattern` из черновика; черновик с ключом восстанавливается.
- [ ] **Step 2:** тесты падают.
- [ ] **Step 3: Код.**
  - `PatternInput`:
    - сетка 3 × 3 из кнопок с номером порядка у выбранных;
    - линия между точками (SVG, цвета из токенов);
    - под сеткой — последовательность и состояние «Ключ готов» / «Ещё N точки»;
    - кнопки «Отменить шаг» и «Сбросить».
  - `PatternField`:
    - подпись и кнопка с текстом «Не задан» / «Задан, N точек»;
    - всплывающее окно (`Popover`) с `PatternInput` и кнопкой «Готово»;
    - «Готово» доступно, если ключ пуст или `patternReady`, и вызывает `onchange`;
    - закрытие без «Готово» отбрасывает правку.
  - Приём: `PatternField` «Графический ключ» рядом с «Паролем», подсказка «Необязательно. Виден сотрудникам в карточке заказа»; черновик хранит `pattern`.
  - Карточка (`editDevice`): `device_pattern` в `ORDER_FIELDS`, `PatternField` рядом с паролем, сохранение через `autosave.commit`.
  - Панель: в строке пароля — «Ключ ••••», кнопка показа общая с паролем, открытый вид — `patternText`.
  - Раздел на витрине `/design`.
- [ ] **Step 4: e2e.** `intake.spec.ts`: `test('a pattern key from intake is kept on the order')`:
  - открыть приём и заполнить обязательное;
  - в «Графический ключ» щёлкнуть точки 1, 5, 9, 6, нажать «Готово» и создать заказ;
  - через API у заказа `device_pattern === '1596'`;
  - в панели — «Ключ ••••»; после «Показать» видно `1 · 5 · 9 · 6`.
- [ ] **Step 5:** `npm run verify` и e2e (Chromium, WebKit) зелёные.
- [ ] **Step 6:** коммит `feat(ui): графический ключ при приёме и в карточке заказа`.

### Task 6: «Комплектность» и «Состояние» — списком, частые сверху

**Files:**
- Modify: `crates/mw-core/src/usage.rs` (право `dictionary_usage`), маршрут `crates/mw-api/src/routes/catalogs.rs:385-397` (описание права в OpenAPI, если оно там названо)
- Test: `crates/mw-core/tests/catalog_usage.rs`
- Create: `apps/ui/src/lib/ui/MultiCombobox.svelte`, `apps/ui/src/lib/intake/ranking.ts`
- Modify: `apps/ui/src/lib/intake/IntakeSheet.svelte`, витрина `/design`, `apps/ui/src/lib/i18n/ru.ts`
- Test: `apps/ui/src/lib/ui/MultiCombobox.test.ts`, `apps/ui/src/lib/intake/ranking.test.ts`, существующие тесты и e2e приёма, которые щёлкали бейджи «Комплектности»/«Состояния», `apps/ui/e2e/intake.spec.ts`
- Modify: `docs/superpowers/plans/README.md` («Умные списки…» → `[x]` с решением 5; список в карточке заказа — «на потом»)

**Interfaces:**
- Produces:
  - `Core::dictionary_usage` пускает `require_any(&[SettingsManage, OrdersCreate, OrdersEdit])`;
  - `rankByUsage(items: DictionaryItem[], usage: CatalogUsage[]): string[]`;
  - `MultiCombobox` — пропсы `{ selected = $bindable<string[]>([]), options: string[], label: string, help?: string | readonly string[], allowCustom?: boolean, oncreate?: (value: string) => void, placeholder?: string, id?: string, disabled?: boolean, limit?: number /* 50 */ }`.

- [ ] **Step 1: Тест ядра.** `catalog_usage.rs`, `all_backends`:
  - `an_intake_clerk_can_read_usage_counts` — актёр только с `orders_create` получает счёт;
  - актёр без `settings_manage`, `orders_create` и `orders_edit` получает `Forbidden`.
- [ ] **Step 2: Тесты интерфейса.**
  - `ranking.test.ts`: `rankByUsage([{id:1,value:'Чехол'},{id:2,value:'Коробка'},{id:3,value:'Шнур'}], [{id:3,count:5},{id:2,count:9}])` → `['Коробка','Шнур','Чехол']`; при пустом `usage` — порядок справочника. Поля `DictionaryItem` — как в привязке; в тесте только нужные.
  - `MultiCombobox.test.ts`:
    - выбранные — снимаемые метки (кнопка «Убрать: Чехол»);
    - ввод «кор» оставляет в списке только подходящие (без учёта регистра, `normalizeSearch`) и не показывает уже выбранные;
    - ↓ и Enter выбирают подсвеченное;
    - Enter со своим текстом при `allowCustom` добавляет его и вызывает `oncreate`;
    - Backspace в пустом поле снимает последнюю метку;
    - Escape закрывает список;
    - `shows at most the limit` — при 300 вариантах в списке 50 строк.
- [ ] **Step 3:** тесты падают.
- [ ] **Step 4: Код.**
  - Право в `dictionary_usage`.
  - `MultiCombobox` (на bits-ui `Combobox` с `type="multiple"` или своей разметкой listbox — на выбор исполнителя; доступность как у `Combobox`) + раздел витрины.
  - В приёме «Комплектность» и «Состояние» — `MultiCombobox` вместо `ChipGroup`:
    - варианты: `mergeSuggestions(rankByUsage(словарь, счёт), выбранные)`;
    - счёт грузится вместе со справочниками: `dictionaryUsage(kind).catch(() => [])`, только для этих двух видов;
    - `oncreate` → `remember(...)`, как сейчас;
    - подсказки прежние.
  - «Частые неисправности» не трогать.
- [ ] **Step 5: e2e.** `intake.spec.ts`: `test('completeness is picked from a list, the most used first')`:
  - через API в справочник «Комплектность» значения `Реже <uniq>` и `Чаще <uniq>`; три заказа с `Чаще <uniq>`, один с `Реже <uniq>`;
  - в приёме ввести `<uniq>` в «Комплектность» → первый вариант `Чаще <uniq>`, второй `Реже <uniq>`;
  - выбрать первый, ввести `Своё <uniq>` и Enter;
  - создать заказ → у заказа `completeness === 'Чаще <uniq>, Своё <uniq>'`.
  
  Существующие e2e приёма, которые щёлкали бейджи этих полей, перевести на список.
- [ ] **Step 6:** Rust и интерфейс зелёные.
- [ ] **Step 7:** коммит `feat: комплектность и состояние списком, частые сверху`.

### Task 7: Прошлые заказы клиента — краткий просмотр при наведении

**Files:**
- Modify: `crates/mw-core/src/orders/list.rs` (`OrderListItem.discount`)
- Test: `crates/mw-core/tests/orders_list.rs`
- Contract: `openapi.json`, `bindings/OrderListItem.ts`
- Create: `apps/ui/src/lib/ui/HoverCard.svelte`, `apps/ui/src/lib/clients/brief.ts`
- Modify: `apps/ui/src/lib/clients/ClientOrders.svelte`, витрина `/design`, `apps/ui/src/lib/i18n/ru.ts`, фабрики `apps/ui/src/lib/app/fixtures.test-helper.ts` и демо `apps/ui/src/lib/demo/fake.ts` (новое поле)
- Test: `apps/ui/src/lib/clients/brief.test.ts`, `apps/ui/e2e/clients.spec.ts`
- Modify: `docs/superpowers/plans/README.md` («Быстрый просмотр прошлых заказов…» → `[x]`)

**Interfaces:**
- Produces:
  - `OrderListItem.discount: Money`;
  - `HoverCard` — пропсы `{ trigger: Snippet<[Record<string, unknown>]>, children: Snippet, openDelay?: number /* 400 */, side?: 'top'|'right'|'bottom'|'left' /* 'right' */ }`;
  - `orderBrief(o: OrderListItem, statusName: string): { title: string; lines: string[] }`.

- [ ] **Step 1: Тест ядра.** `orders_list.rs`, `all_backends`: `list_items_carry_the_discount` — у заказа со скидкой 200 ₽ строка списка отдаёт `discount == Money::from_kopecks(20_000)`; конструктор — как в коде.
- [ ] **Step 2: Тест `brief.test.ts`.** Даты — через `formatDate` из `#lib/format/date.js`, в ожиданиях — тот же вызов.
  - Заказ: итог 2500 ₽, скидка 200 ₽, предоплата 0, выдан. `title === '№ 41 · ' + formatDate(received_at)`.
  - `lines` по порядку:
    - устройство (`deviceOf`);
    - первая строка неисправности (если есть);
    - `Итог: 2 500 ₽`;
    - `Скидка: 200 ₽`;
    - `Выдан ` + `formatDate(issued_at)`.
  - Без итога — `Итог не указан`; скидка 0 — строки нет; предоплата больше нуля — `Предоплата: …` после итога; не выдан — имя статуса вместо строки «Выдан».
- [ ] **Step 3:** тесты падают.
- [ ] **Step 4: Код.**
  - `discount` в выборке, строке и `From` списка.
  - Контракт.
  - `HoverCard` на bits-ui `LinkPreview`: открывается при наведении и фокусе, стиль `.mw-popover`.
  - В `ClientOrders` строка-кнопка — триггер `HoverCard` с `orderBrief`.
  - На `display.phone` вместо карточки под устройством вторая строчка: `formatMoney(итог)` и «выдан <дата>».
- [ ] **Step 5: e2e.** `clients.spec.ts`: `test('hovering an earlier order shows its brief')`:
  - через API клиент и заказ с итогом 2500 и скидкой 200;
  - открыть карточку клиента и навести на строку заказа;
  - видно `Итог: 2 500 ₽` и `Скидка: 200 ₽`.
- [ ] **Step 6:** Rust, `npm run verify`, e2e (Chromium, WebKit) зелёные.
- [ ] **Step 7:** коммит `feat: краткий просмотр прошлых заказов в карточке клиента`.

### Task 8: Выбор страниц при печати

**Files:**
- Create: `crates/mw-domain/src/pages.rs` (+ `pub mod pages;`)
- Modify:
  - `crates/mw-docs/src/{render.rs,job.rs}` и протокол процесса-рендерера (`RenderPool`), если задание идёт через него;
  - `crates/mw-core/src/documents.rs` (`DocumentRequest.pages`, ошибка `out_of_range`);
  - `crates/mw-api/src/dto.rs` (`DocumentQuery.pages`) и обработчики документов в `crates/mw-api/src/routes/`.
- Test:
  - `crates/mw-domain/src/pages.rs` (модульные);
  - `crates/mw-docs` (тест выгрузки страниц);
  - `crates/mw-core/tests/documents.rs`;
  - `crates/mw-api/tests/` (файл с тестами документов).
- Create: `apps/ui/src/lib/print/pages.ts`
- Modify: `apps/ui/src/lib/api/documents.ts`, `apps/ui/src/lib/print/print.svelte.ts`, `apps/ui/src/lib/print/DocumentPreview.svelte`, `apps/ui/src/lib/i18n/ru.ts`
- Test: `apps/ui/src/lib/print/pages.test.ts`, `apps/ui/src/lib/print/DocumentPreview.test.ts`, `apps/ui/e2e/print.spec.ts`
- Modify: `docs/superpowers/plans/README.md` («Выбор страниц при печати» → `[x]`, решение 7)

**Interfaces:**
- Produces:
  - `mw_domain::pages::PageSelection` — отсортированные уникальные номера с 1. Методы: `PageSelection::parse(text: &str) -> Result<PageSelection, &'static str>`, `contains(page: usize) -> bool` (с 1), `as_slice() -> &[u16]`.
  - `DocumentRequest.pages: Option<PageSelection>`.
  - Параметр запроса `pages` у `GET …/documents/{kind}`.
  - `pageSelection(checked: boolean[]): string | null` в `#lib/print/pages.js`.
  - `documentPdf`/`documentPages` и `printDocument`/`savePdf` принимают `pages?: string`.

- [ ] **Step 1: Тесты домена.**
  - `parse` принимает:
    - `"1"` → `[1]`;
    - `"1,3"` → `[1,3]`;
    - `" 1, 3-4 "` → `[1,3,4]`;
    - `"3–4"` (en dash) → `[3,4]`;
    - `"4,1,1"` → `[1,4]`.
  - Не принимает: `""`, `"0"`, `"2-1"`, `"a"`, `"1-"`, `"1,,2"`, `"70000"`.
- [ ] **Step 2: Тест mw-docs.** `only_the_chosen_pages_are_exported`. Документ из двух страниц (`#pagebreak()` в разметке теста):
  - PNG с выбором `[2]` — одна страница, равная второй странице полной выгрузки;
  - PDF с выбором `[2]` — одна страница (счёт объектов `/Type /Page`, за которыми не идёт `s`);
  - выбор `[5]` — новая ошибка `DocsError::NoPages`.
- [ ] **Step 3: Тесты ядра и API.**
  - `documents.rs`, `all_backends`:
    - `a_page_selection_prints_only_those_pages` — документ из двух страниц (черновик шаблона через `Core::with_code_in_process` с `#pagebreak()` или стандартная раскладка с экземплярами на отдельных листах, если она есть) и `pages: [2]` → в PNG одна страница;
    - `pages_beyond_the_document_are_refused` — акт приёма в одну страницу и `pages: [5]` → `Validation` с полем `pages` и кодом `out_of_range`.
  - API: `?pages=abc` → 422 с полем `pages`, `?pages=1` у акта приёма → 200.
- [ ] **Step 4:** тесты падают.
- [ ] **Step 5: Код сервера.**
  - Выбор проходит до места, где строятся PDF и PNG: `PdfOptions { page_ranges: Some(…), ..Default::default() }` и фильтр PNG по номеру.
  - Через процесс-рендерер — новым полем кадра с `#[serde(default)]`.
  - `pages` вне документа → `DocsError::NoPages` → `FieldError::new("pages", "out_of_range")`.
  - Пачка (`/batch`) не меняется.
  - Контракт.
- [ ] **Step 6: Тесты интерфейса.**
  - `pages.test.ts`:
    - `pageSelection([true, true]) === null`;
    - `pageSelection([true, false]) === '1'`;
    - `pageSelection([false, true, true, false, true]) === '2-3,5'`;
    - `pageSelection([false, false]) === ''`.
  - `DocumentPreview.test.ts`:
    - у документа из двух страниц — две галочки «Страница 1» и «Страница 2», обе отмечены;
    - снять вторую и нажать «Печать» → `print` вызван с `{ pages: '1' }`;
    - «Сохранить PDF» тоже передаёт `pages: '1'`;
    - снять обе → «Печать» и «Сохранить PDF» недоступны;
    - у документа в одну страницу галочек нет.
- [ ] **Step 7: Код интерфейса.**
  - Параметр `pages` в `documentQuery` (не передаётся, если `null`).
  - `printDocument(kind, target, { noPreview, pages })` и `savePdf(kind, target, pages?)` передают его в `send`.
  - Галочки в `DocumentPreview` поверх или под каждой страницей.
- [ ] **Step 8: e2e.** `print.spec.ts`: `test('a two-page document saves only the checked page')`:
  - через API шаблон акта приёма с блоком «Typst-код» `#pagebreak()` (сервер e2e строит его процессом-рендерером);
  - открыть предпросмотр — две галочки;
  - снять вторую и нажать «Сохранить PDF» → запрос документа ушёл с `pages=1`, проверка через `page.waitForRequest`.
  
  После теста вернуть стандартный шаблон.
- [ ] **Step 9:** Rust (обе базы), `npm run verify`, e2e (Chromium, WebKit) зелёные.
- [ ] **Step 10:** коммит `feat: выбор страниц при печати из предпросмотра`.
