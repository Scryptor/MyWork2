# План 5.1. Заказы: настоящий главный экран — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** интерфейс работает с настоящим сервером. Приёмщик открывает программу в браузере по адресу сервера и
делает всю работу с заказами:
- видит список с быстрыми фильтрами, «Требуют внимания», фильтром `/` и «Фильтры ▾»;
- правит заказ в панели с автосохранением, видит конфликт правок;
- меняет статус с отменой, выдаёт, принимает по гарантии;
- делает массовые действия;
- открывает полную карточку;
- принимает устройство (без фото — их добавит план 5.2);
- находит заказ через Ctrl K и сканер;
- входит в программу, если в ней есть пользователи.

**Architecture:**
- `mw-server` раздаёт собранный интерфейс (`apps/ui/build`) с того же адреса, что и API. Интерфейс ходит в
  `/api/v1` того же источника. Поэтому CORS не нужен, а сквозные тесты работают так же, как будет работать десктоп
  (план 5.4) и телефоны (план 10).
- Все запросы интерфейса идут через `#lib/api/` — тонкий клиент над `fetch` с типами из `$api/...`.
- Экраны лежат в `src/routes/(app)/`, их логика без вида — в `src/lib/orders/`, `src/lib/intake/`, `src/lib/app/`.
  Панель заказа и лист приёма живут в общем макете `(app)`, поэтому открываются с любой страницы.
- Сквозные тесты Playwright запускают настоящий `mw-server` на временной папке данных и создают вымышленные
  заказы через API.
- Демо-экраны `/design/orders` и `/design/intake` заменяются настоящими и удаляются в задаче 11. Витрина
  компонентов `/design` остаётся.

**Tech Stack:** Rust 1.95 (Axum 0.8, SeaORM 2, `tower-http` 0.6 с фичей `fs` — новая зависимость, MIT), SvelteKit 3 +
Svelte 5, bits-ui 2.21, Playwright 1.65. Новых npm-пакетов нет.

**Spec:**
- [`docs/superpowers/specs/2026-11-15-ui-design-system-design.md`](../specs/2026-11-15-ui-design-system-design.md) —
  разделы 2, 5, 6.1–6.5, 6.10–6.12, 8, 9, 12;
- основная спека [`2026-11-11-mywork3-crossplatform-design.md`](../specs/2026-11-11-mywork3-crossplatform-design.md),
  разделы 4 и 5.0;
- дорожная карта [`README.md`](README.md), «Заметки для следующих планов → План 5».

## Global Constraints

- **Ветка и место.** Worktree `/Users/scrypto/dev/MyWork2/.superpowers/worktrees/plan-5-1`, ветка
  `plan-5-1-orders`. Ветку отправлять на GitHub только для прогона CI перед слиянием. После слияния в `main` —
  сразу `git push origin main`.
- **Все правила `AGENTS.md`** действуют без исключений. Самые важные здесь:
  - сначала тест, потом код; тесты с БД — по `all_backends()`;
  - деньги — `Money`, время — `Core::now()`/`Core::now_utc()`, поиск без учёта регистра — `mw_domain::search`;
  - любая запись — через `Core::begin_write()`;
  - выпущенные миграции не меняются, только новые;
  - правка построителей `search_text`/нормализации увеличивает `SEARCH_INDEX_VERSION`;
  - входные типы API — с `deny_unknown_fields`; после изменения API —
    `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1` и коммит вместе с кодом.
- **Rust перед коммитом:**
  `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`,
  для задач 3–4 ещё и с `MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres` (`docker start mw-pg`).
- **Интерфейс перед коммитом:** `cd apps/ui && npm run verify && npm run test:e2e`.
- **Правила интерфейса из `AGENTS.md`:**
  - токены вместо литералов;
  - руны;
  - импорты `#lib/...` с расширениями;
  - типы API — только `import type` из `$api/...`;
  - строки — в `#lib/i18n/ru.ts`;
  - иконки `@lucide/svelte` по одной;
  - даты — через `#lib/format/date.js`;
  - деньги — через `#lib/format/money.js` и `MoneyInput`;
  - однобуквенные клавиши — через `registerHotkey`.
- **Новое правило:** `fetch` в интерфейсе вызывает только `src/lib/api/client.ts`. Компоненты и страницы
  получают данные через функции `#lib/api/*.js`.
- **Данные тестов только вымышленные.** E2E создают заказы через API с уникальной меткой (`uniq()`), поэтому тесты
  не мешают друг другу при параллельном запуске и в двух браузерах.
- **Тексты** — по разделу 9 спеки интерфейса: кнопки глаголами, без «Вы уверены?», ошибка говорит, что случилось и что делать.

## Review Focus

1. **Медленная сеть при автосохранении.** Человек продолжает печатать, пока предыдущее сохранение в пути. Ответ
   на старый запрос не должен затирать набранное позже (заметка про `MoneyInput`/`PhoneInput`). Тесты: задача 8,
   `autosave.test.ts` «stale response keeps newer local text» и e2e «typing during a slow save keeps the text».
2. **Заказ изменили на другом компьютере.** Ответ 409 не теряет ни своё, ни чужое: поля без спора берут новое, спорные —
   выбор «Оставить моё» или «Взять новое». Тесты: задача 8, `merge.test.ts` и e2e «conflict callout».
3. **Сервер недоступен или ответил 500 во время правки.** Индикатор «Не сохранено · Повторить», набранное остаётся,
   «Повторить» досылает правку. Если закрыть панель с несохранённой правкой, правка досылается перед закрытием.
   Тесты: задача 8, `autosave.test.ts` «failure keeps changes and retry resends» и e2e «offline save shows retry».
4. **Один штрихкод у нескольких заказов** (в базе автора 277 групп). Нужен диалог выбора. Неизвестный код —
   уведомление. Скан в поле (IMEI) остаётся в поле. Тесты: задача 6, `scan.test.ts` и e2e «barcode with two orders
   asks which one».
5. **Регистр в старых данных.** «samsung» и «Samsung», «антон» и «Антон» — для фильтров «Фильтры ▾» и сортировки
   это одно значение. Тесты: задача 3, `filters_and_sorting_ignore_case`.

---

## Файловая структура

```
crates/mw-db/src/migration/m20261116_000001_order_keys.rs   новые колонки заказа + заполнение status_changed_at
crates/mw-db/src/backfill.rs                                SQL заполнения status_changed_at (миграция и импорт)
crates/mw-domain/src/status.rs                              STATUS_COLORS — 12 имён палитры
crates/mw-core/src/orders/summary.rs                        OrderSummary, order_summary, order_values
crates/mw-core/src/orders/history.rs                        order_history, accept_warranty_case
crates/mw-api/src/static_ui.rs                              раздача интерфейса из ui_dir с запасным index.html
apps/ui/src/lib/api/        client.ts, session.ts, server-clock.svelte.ts, orders.ts, catalogs.ts, clients.ts, auth.ts
apps/ui/src/lib/app/        context.svelte.ts (сервер, я, статусы, точки, настройки), undo.svelte.ts, recent.ts,
                            palette.ts, scan.ts, AppTopBar.svelte, AppSidebar.svelte, ScanChoiceDialog.svelte
apps/ui/src/lib/orders/     view.ts, columns.ts, list.svelte.ts, events.ts, autosave.ts, merge.ts, bulk.ts,
                            panel.svelte.ts, OrderPanel.svelte, OrderFields.svelte, IssueDialog.svelte,
                            WarrantyCaseDialog.svelte, ReturnToWorkDialog.svelte, FiltersMenu.svelte, OrderHistory.svelte
apps/ui/src/lib/intake/     draft.ts, IntakeSheet.svelte, ClientStep.svelte
apps/ui/src/routes/         +page.ts (→ /orders), login/+page.svelte,
                            (app)/+layout.ts, (app)/+layout.svelte, (app)/orders/+page.svelte, (app)/orders/[id]/+page.svelte
apps/ui/e2e/support/        server.mjs (запуск mw-server), api.ts (данные через API)
```

---

### Task 1: Дизайн-система: кнопки-триггеры, поля, общие помощники

Доделки из заметок к плану 5, которые нужны экранам.

**Files:**
- Modify: `apps/ui/src/lib/ui/Button.svelte`, `IconButton.svelte`, `DropdownMenu.svelte`, `Popover.svelte`,
  `Select.svelte`, `DateInput.svelte`, `MoneyInput.svelte`, `PhoneInput.svelte`, `TextInput.svelte`, все места со
  стилем `.trigger` (`grep -rn '\.trigger' apps/ui/src`)
- Create: `apps/ui/src/lib/design/storage.ts`, `apps/ui/src/lib/format/digits.ts`
- Modify: `apps/ui/src/lib/format/money.ts`, `plural.ts`, `date.ts`, `apps/ui/src/lib/ui/table/SelectionBar.svelte`,
  `apps/ui/src/lib/demo/orders.ts`, все 5 копий доступа к `localStorage` (`grep -rn localStorage apps/ui/src`)
- Test: `storage.test.ts`, `digits.test.ts`, `date.test.ts`, `TextInput.test.ts`, `DateInput.test.ts`,
  `MoneyInput.test.ts`, `PhoneInput.test.ts`, `apps/ui/e2e/layers.spec.ts`

**Interfaces:**
- Produces:
  - `readStorage(key: string, area?: 'local' | 'session'): string | null`,
    `writeStorage(key: string, value: string, area?: 'local' | 'session'): void`,
    `removeStorage(key: string, area?: 'local' | 'session'): void` — никогда не бросают исключений;
  - `groupDigits(digits: string): string` — разряды через U+202F;
  - `dayNumber(d: LocalDateTime): number` — номер дня для разницы в днях (из `date.ts`, копия в `demo/orders.ts` удаляется);
  - `Button` и `IconButton` передают прочие атрибуты (`...rest`) на `<button>`. Поэтому они работают как `child`
    у триггеров bits-ui: `{#snippet child({ props })}<Button {...props}>…</Button>{/snippet}`;
  - `TextInput`: проп `ariaLabel?: string` (когда нет видимой `label`) и `ref = $bindable<HTMLInputElement | null>(null)`;
  - `MoneyInput`, `PhoneInput`, `DateInput`: пока поле в фокусе, изменение `value` снаружи не трогает набранный текст;
    после ухода из фокуса текст показывает `value`;
  - `DateInput` разбирает дату при каждом вводе, как `MoneyInput`: `value` меняется, как только текст — верная дата;
    `invalid` и ошибка показываются при уходе из поля и Enter.

- [ ] **Step 1: Тесты помощников**

```ts
// storage.test.ts
it('reads null and swallows errors when storage throws', () => {
	vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => { throw new Error('denied'); });
	expect(readStorage('mw.x')).toBeNull();
	expect(() => writeStorage('mw.x', '1')).not.toThrow();
});
it('uses sessionStorage for the session area', () => {
	writeStorage('mw.s', 'a', 'session');
	expect(sessionStorage.getItem('mw.s')).toBe('a');
	expect(localStorage.getItem('mw.s')).toBeNull();
});
// digits.test.ts
it('groups by three with a narrow no-break space', () => {
	expect(groupDigits('6500')).toBe('6 500');
	expect(groupDigits('1234567')).toBe('1 234 567');
	expect(groupDigits('999')).toBe('999');
});
```

- [ ] **Step 2: Тесты полей** (Vitest, `@testing-library/svelte`)

```ts
// TextInput.test.ts
it('takes an aria-label without a visible label', () => {
	render(TextInput, { value: '', ariaLabel: 'Фильтр списка' });
	expect(screen.getByRole('textbox', { name: 'Фильтр списка' })).toBeInTheDocument();
});
// MoneyInput.test.ts
it('keeps typed text while focused when value changes from outside', async () => {
	// render with value 100_00, focus, type "4 5" → value 45_00; set prop value = 100_00 (late server answer)
	// → input still shows "4 5"; blur → shows "100"
});
// DateInput.test.ts
it('updates value on every keystroke that makes a valid date', async () => {
	// type "15.11.2026" → value '2026-11-15T00:00:00' before blur; type one more char → invalid only after blur
});
```

`PhoneInput.test.ts` — такой же тест, как для `MoneyInput`: набранное в фокусе не затирается.

- [ ] **Step 3: Запустить — тесты падают** (`npx vitest run src/lib/design/storage.test.ts src/lib/format src/lib/ui`)

- [ ] **Step 4: Реализовать.**
  - Перевести все 5 копий доступа к `localStorage` на `storage.ts`.
  - Перевести `money.ts`, `plural.ts` и `SelectionBar.svelte` на `groupDigits`, `demo/orders.ts` — на `dayNumber` из `date.ts`.
  - Убрать пять копий стиля `.trigger`: триггеры меню и всплывающих окон рисуются через `Button`/`IconButton`.
    Вид прежний: `secondary`, размер `sm`, иконка `ChevronDown` справа.
  - Фокус и «свой текст в фокусе» сделать в полях.

- [ ] **Step 5: Проверка.** `npm run verify && npm run test:e2e`. Все прежние тесты витрины зелёные, вид кнопок-триггеров
  на `/design` не изменился.

- [ ] **Step 6: Commit** — `feat(ui): trigger buttons, field focus and aria-label, shared storage and digit helpers`

---

### Task 2: Дизайн-система: слои, клавиши, сканер, выбор статуса

**Files:**
- Modify: `apps/ui/src/lib/design/hotkeys.svelte.ts`, `scanner.ts`, `layers.svelte.ts`,
  `apps/ui/src/lib/ui/StatusPicker.svelte`, `Sheet.svelte`, `Dialog.svelte`, `Tooltip.svelte`,
  `toast/Toaster.svelte`, формы с Ctrl/⌘+Enter (`grep -rn "matchShortcut\|mod+Enter" apps/ui/src`)
- Test: `hotkeys.test.ts`, `scanner.test.ts`, `StatusPicker.test.ts`, `apps/ui/e2e/layers.spec.ts`,
  `apps/ui/e2e/navigation.spec.ts`

**Interfaces:**
- Produces:
  - `registerHotkey(spec, handler, opts?)` — у однобуквенных клавиш без модификаторов (`n`, `/`, `?`) срабатывание
    откладывается на `SCAN_GAP_MS` (35 мс, константа из `scanner.ts`). Если за это время пришла ещё одна печатная
    клавиша, срабатывание отменяется: это скан;
  - `submitShortcut(handler: () => void): () => void` в `hotkeys.svelte.ts` — единственный способ Ctrl/⌘+Enter
    («Принять», «Выдать», «Сохранить»). Это `registerHotkey('mod+Enter', handler, { allowInFields: true, allowWithOverlay: true })`;
    регистрирует его открытый слой и снимает при закрытии. `onkeydown` с `matchShortcut` в формах удаляется;
  - `StatusPicker`: проп `max = 7` — статусы сверх `max` уходят в меню «Ещё ▾». Если выбранный статус в меню, кнопка
    «Ещё» показывает его имя. Стрелки двигают фокус без выбора, выбор — пробел, Enter или клик;
  - `Sheet` ставит фокус в первое поле после того, как SvelteKit закончил фокусировку после навигации: ждёт
    `afterNavigate` и `tick()`, если навигация идёт;
  - `Dialog`: если элемент, открывший диалог, исчез, после закрытия фокус уходит на `<main>` (у него
    `tabindex="-1"`);
  - уведомления нажимаются и при открытом модальном окне. Нажатие на уведомление не закрывает окно и не уводит из
    него фокус надолго: после нажатия фокус возвращается в окно;
  - открытая подсказка не съедает Esc: одно нажатие Esc в панели закрывает и подсказку, и панель.

- [ ] **Step 1: Модульные тесты**

```ts
// hotkeys.test.ts (vi.useFakeTimers)
it('fires a single-letter hotkey after the scan gap', () => {
	const h = vi.fn(); registerHotkey('n', h);
	press('n'); expect(h).not.toHaveBeenCalled();
	vi.advanceTimersByTime(36); expect(h).toHaveBeenCalledTimes(1);
});
it('drops a single-letter hotkey when another key follows within the gap (a scan)', () => {
	const h = vi.fn(); registerHotkey('/', h);
	press('/'); vi.advanceTimersByTime(10); press('1'); vi.advanceTimersByTime(100);
	expect(h).not.toHaveBeenCalled();
});
it('mod+Enter fires at once, also inside fields', () => { /* submitShortcut + keydown in <input> */ });
// StatusPicker.test.ts
it('puts statuses beyond max into «Ещё»', () => { /* 9 statuses, max 7 → 7 segments + button «Ещё» */ });
it('arrow keys move focus without choosing; space chooses', async () => {
	// focus segment 1, ArrowRight twice → onchange not called; Space → onchange(id3, id1) once
});
```

- [ ] **Step 2: Сквозные тесты** (Chromium и WebKit, страницы витрины)

```ts
// layers.spec.ts
test('toast undo is clickable while a dialog is open', async ({ page }) => {
	// /design: open the dialog sample, trigger a toast with «Отменить» from inside it,
	// click «Отменить» → the undo counter changes and the dialog is still open, focus is inside it
});
test('one Esc closes a panel even when a tooltip is open in it', async ({ page }) => { /* SlideOver sample */ });
test('dialog returns focus to main when its opener is gone', async ({ page }) => { /* sample removes the opener */ });
// navigation.spec.ts
test('sheet opened right after navigation keeps the caret in WebKit', async ({ page }) => {
	// navigate inside /design, press the key that opens the Sheet sample at once, type «абв» → the first field holds «абв»
});
```

Если для теста нужен образец на витрине (диалог, который вызывает уведомление; кнопка, которая исчезает), добавить
его в раздел `Layers.svelte`.

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать.** Перевести формы витрины и демо-экранов на `submitShortcut`.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): scan-safe hotkeys, one submit shortcut, status picker overflow, layer focus fixes`

---

### Task 3: Сервер: ключи сортировки, «Давно готовы», сводка и значения для фильтров

**Files:**
- Create: `crates/mw-db/src/migration/m20261116_000001_order_keys.rs`, `crates/mw-db/src/backfill.rs`,
  `crates/mw-core/src/orders/summary.rs`
- Modify: `crates/mw-db/src/migration/mod.rs`, `crates/mw-db/src/entity/order.rs`, `crates/mw-db/src/lib.rs`,
  `crates/mw-core/src/search_index.rs`, `orders/{create,update,list,types,mod}.rs`, `settings.rs`,
  `import/write.rs`, `crates/mw-api/src/routes/orders.rs`, `crates/mw-api/src/dto.rs`
- Test: `crates/mw-core/tests/orders_list.rs`, `crates/mw-core/tests/orders_summary.rs`, `crates/mw-core/tests/import.rs`,
  `crates/mw-core/tests/search_index.rs`, `crates/mw-api/tests/orders.rs`

**Interfaces:**
- Produces (БД):
  - у `orders` новые колонки `status_changed_at` (дата-время, пусто можно) и `device_type_key`, `brand_key`,
    `model_key`, `master_key` (текст, не пусто, по умолчанию `''`);
  - индексы на `brand_key`, `master_key`, `device_type_key`;
  - ключ = `mw_domain::search::normalize_search(значение)`;
  - `mw_db::backfill::status_changed_at(conn: &impl ConnectionTrait) -> Result<u64, DbErr>` выполняет
    `UPDATE orders SET status_changed_at = (SELECT MAX(a.at) FROM audit_log a WHERE a.order_id = orders.id AND a.action IN ('order.status_changed', 'legacy.status'))`
    и вызывается из новой миграции и в конце записи импорта.
- Produces (ядро):
  - `create_order`/`update_order` пишут ключи; `change_status`/`issue_order` пишут `status_changed_at = Core::now()`;
  - построитель индекса заполняет ключи; `SEARCH_INDEX_VERSION` = 3;
  - `AppSettings.ready_stale_days: u16`, по умолчанию 30 (и в `SettingsPatch`);
  - `OrderFilter` (и `OrderListQuery`) получает:
    - `device_type`, `brand`, `master: Option<String>` — сравнение по ключу;
    - `received_from`, `received_to`, `issued_from`, `issued_to: Option<NaiveDateTime>` — границы включительно;
    - `stale_ready: bool` — вид статуса `ready` и `COALESCE(status_changed_at, received_at) < now − ready_stale_days`;
  - сортировка по `device_type`/`brand`/`model`/`master` идёт по ключам, затем по `id` убыв.;
  - `Core::order_summary(&self, actor: &Actor, service_point_id: Option<i64>) -> Result<OrderSummary, CoreError>`;

    ```rust
    pub struct OrderSummary {
        /// Неудалённые заказы по статусам; статусы без заказов не включаются.
        pub by_status: Vec<StatusCount>,
        /// Дальше — только заказы в сервисе (вид статуса не `issued`).
        pub need_call: u64,
        pub need_parts: u64,
        pub overdue_diagnostics: u64,
        pub stale_ready: u64,
    }
    pub struct StatusCount { pub status_id: i64, pub count: u64 }
    ```

  - `Core::order_values(&self, actor: &Actor, field: OrderValueField, query: &str, brand: Option<&str>) -> Result<Vec<String>, CoreError>`:
    - `OrderValueField` = `device_type | brand | model | master` (serde `snake_case`);
    - различные значения по ключу, самые частые сверху, не больше 20;
    - для каждого ключа — самое частое написание;
    - `query` — подстрока ключа;
    - `brand` сужает модели до этого бренда.
- Produces (API):
  - `GET /api/v1/orders/summary?service_point_id=` → `OrderSummary`;
  - `GET /api/v1/orders/values?field=&query=&brand=` → `string[]`;
  - новые поля `OrderListQuery` и `AppSettings`.

- [ ] **Step 1: Тесты ядра** (`orders_list.rs`; помощник `create_order_with(t, |n: &mut NewOrder| …)` добавить в
  `tests/common/mod.rs`, если подобного нет)

```rust
#[tokio::test]
async fn filters_and_sorting_ignore_case() -> Result<(), CoreError> {
    for t in cores().await {
        let a = create_order_with(&t, |n| { n.brand = "samsung".into(); n.master = "антон".into(); }).await.id;
        let b = create_order_with(&t, |n| { n.brand = "Samsung".into(); n.master = "Антон".into(); }).await.id;
        let c = create_order_with(&t, |n| { n.brand = "Apple".into(); n.master = "Ёжиков".into(); }).await.id;
        let f = |brand: &str| OrderFilter { brand: Some(brand.into()), ..Default::default() };
        assert_eq!(sorted(list_ids(&t, f("SAMSUNG")).await), sorted(vec![a, b]), "{}", t.label);
        let by_master = OrderFilter { master: Some("АНТОН".into()), ..Default::default() };
        assert_eq!(sorted(list_ids(&t, by_master).await), sorted(vec![a, b]));
        let asc = list_sorted(&t, OrderSortField::Brand, false).await; // помощник: id по сортировке
        assert_eq!(asc, vec![c, b, a]); // apple < samsung; внутри ключа — id убыв.
    }
    Ok(())
}

#[tokio::test]
async fn stale_ready_uses_the_last_status_change_or_receipt() -> Result<(), CoreError> {
    // ready_stale_days = 30 (по умолчанию). Заказ A принят 60 дней назад и сегодня переведён в «Готов» → не давно готов.
    // Заказ B принят 60 дней назад и переведён в «Готов» 31 день назад (clock.advance) → давно готов.
    // Заказ C в «Готов» без истории (status_changed_at пуст), принят 40 дней назад → давно готов.
    // assert_eq!(sorted(list_ids(&t, OrderFilter { stale_ready: true, ..Default::default() }).await), sorted(vec![b, c]));
}

#[tokio::test]
async fn received_and_issued_periods_are_inclusive() -> Result<(), CoreError> {
    // received_from = received_to = день приёма A → [A]; issued_from после выдачи C → [] и т. д.
}
```

```rust
// orders_summary.rs
#[tokio::test]
async fn summary_counts_statuses_and_attention_in_service_only() -> Result<(), CoreError> {
    for t in cores().await {
        // 2 заказа в «Диагностика» (один need_call), 1 в «Готов» давно, 1 выданный need_call, 1 удалённый.
        let s = t.core.order_summary(&admin(), None).await?;
        assert_eq!(count_of(&s, diagnostics), 2, "{}", t.label);
        assert_eq!(count_of(&s, issued), 1);
        assert!(s.by_status.iter().all(|c| c.count > 0)); // статусы без заказов не включаются
        assert_eq!((s.need_call, s.stale_ready), (1, 1)); // выданный need_call не считается
        let other_point = t.core.order_summary(&admin(), Some(point_b)).await?;
        assert!(other_point.by_status.is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn values_are_distinct_by_key_most_frequent_first() -> Result<(), CoreError> {
    // бренды: «Samsung» ×2, «samsung» ×1, «Apple» ×1 → order_values(Brand, "", None) == ["Samsung", "Apple"]
    // order_values(Model, "a5", Some("SAMSUNG")) — только модели Samsung, содержащие «a5»
}
```

  - `import.rs`: после импорта синтетической фикстуры у заказа с историей `StatesMap` `status_changed_at` равен
    времени последней записи `legacy.status`.
  - `search_index.rs`: `Core::open` на базе с `SEARCH_INDEX_VERSION` = 2 заполняет ключи у старых заказов.
  - `mw-api/tests/orders.rs`: `GET /api/v1/orders/summary` и `GET /api/v1/orders/values?field=brand` отвечают по
    схеме, `field=color` → 400 `bad_request`.

- [ ] **Step 2: Запустить — тесты падают** (`cargo test -p mw-core --test orders_list --test orders_summary`)

- [ ] **Step 3: Миграция, `backfill.rs`, сущность `order`**

- [ ] **Step 4: Ядро: ключи при записи, `status_changed_at`, фильтры, сортировка, `order_summary`, `order_values`, индекс версии 3, импорт вызывает `backfill::status_changed_at`**

`order_summary` — два запроса: `GROUP BY status_id` по неудалённым и один агрегат с `SUM(CASE …)` по заказам в
сервисе. Условия просрочки — те же выражения, что у фильтров `overdue_diagnostics` и `stale_ready`; вынести их в
общие функции в `list.rs`.

- [ ] **Step 5: API и контракт** — маршруты в `routes/orders.rs`; `MW_UPDATE_CONTRACT=1 cargo test -p mw-api --test contract -- --test-threads=1`

- [ ] **Step 6: Полная проверка Rust на SQLite и PostgreSQL** — зелёная

- [ ] **Step 7: Commit** — `feat(core,api): order sort keys, stale ready, order summary and value suggestions`

---

### Task 4: Сервер: всё остальное для экранов и раздача интерфейса

**Files:**
- Create: `crates/mw-core/src/orders/history.rs`, `crates/mw-api/src/static_ui.rs`
- Modify: `crates/mw-domain/src/status.rs`, `crates/mw-core/src/statuses.rs`, `settings.rs`, `orders/{update,create,mod,types}.rs`,
  `crates/mw-api/src/{lib.rs,state.rs,dto.rs}`, `routes/{orders.rs,meta.rs}`, `crates/mw-api/Cargo.toml`,
  `Cargo.toml` (workspace: `tower-http = { version = "0.6", features = ["fs"] }`),
  `crates/mw-server/src/{config.rs,cli.rs,server.rs}`
- Test: `crates/mw-core/tests/{orders_update.rs,catalogs.rs,settings.rs}`, `crates/mw-core/tests/orders_history.rs`,
  `crates/mw-api/tests/{orders.rs,catalogs.rs}`, `crates/mw-api/tests/static_ui.rs`, тесты `mw-server`

**Interfaces:**
- Produces:
  - `ServerInfo.server_time: String` — `Core::now()` в том же виде, что все даты API (`2026-11-15T10:24:00`);
  - `Core::order_history(&self, actor: &Actor, id: i64, page: PageRequest) -> Result<Page<AuditEntry>, CoreError>`:
    - записи журнала с `order_id = id`, новые сверху;
    - права `audit_view` не нужно;
    - если заказа нет — 404;
    - маршрут `GET /api/v1/orders/{id}/history?limit=&offset=` → `AuditPage`;
    - `PageRequest` — тот же тип страницы, что у `list_audit`, по умолчанию `limit` = 50;
  - `StatusChange.stamp_issued_at: bool`, необязательное, по умолчанию `true`. При `false` и переходе в вид `issued`
    дата выдачи не ставится (массовое действие «не ставить дату»);
  - `mw_domain::status::STATUS_COLORS: [&str; 12]` = `["gray", "red", "orange", "amber", "lime", "green", "teal", "cyan", "blue", "indigo", "violet", "pink"]`
    и `pub fn is_status_color(name: &str) -> bool`. При создании статуса и правке цвета цвет не из списка даёт
    ошибку проверки: поле `color`, код `invalid`, текст «Цвет статуса — одно из 12 имён палитры». Правка без цвета
    старый цвет не проверяет;
  - точка по умолчанию, указывающая на архивную или несуществующую точку, считается пустой: `GET /settings` отдаёт
    `default_service_point_id: null`, приём без точки её не подставляет;
  - `Core::accept_warranty_case(&self, actor: &Actor, id: i64, expected_version: i64, case: NewWarrantyCase) -> Result<Order, CoreError>`:
    - `NewWarrantyCase { complaint: String, at: Option<NaiveDateTime> }` (`deny_unknown_fields`); пустое `at` — `Core::now()`;
    - нужно право `orders_edit`;
    - если заказ не выдан (вид статуса не `issued`) — ошибка проверки: поле `status`, «Принять по гарантии можно только выданный заказ»;
    - если жалоба пустая — `required` у поля `complaint`;
    - если в базе нет неархивного статуса, имя которого после `normalize_search` равно `«принят по гарантии»`, —
      ошибка проверки: поле `status`, «Нет статуса «Принят по гарантии». Добавьте его в настройках»;
    - в одной транзакции: смена статуса (дата выдачи остаётся прежней, `status_changed_at` = сейчас) и запись
      журнала `order.warranty_case` с текстом «Принят по гарантии: <жалоба>» и `details` = `{ "complaint": …, "at": … }`;
    - маршрут `POST /api/v1/orders/{id}/warranty-case?version=` с телом `NewWarrantyCase` → `Order`;
  - раздача интерфейса:
    - `ApiConfig::for_listen_addr(addr)` и `ApiConfig::with_ui_dir(self, dir: Option<PathBuf>) -> ApiConfig`;
      поле `loopback` закрыто, вместо него метод `loopback(&self) -> bool`. Так закрыта заметка «`ApiConfig` строить
      только из фактического адреса»;
    - если `ui_dir` задана, любой `GET`/`HEAD` вне `/api/` отдаёт файл из папки через `tower_http::services::ServeDir`,
      а неизвестный путь — `index.html` (SPA). Неизвестный путь под `/api/` — по-прежнему JSON 404;
    - у ответов с файлами те же заголовки безопасности. Проверка `Host` на loopback действует и для них;
    - `mw-server`: `--ui-dir` / `MW_UI_DIR`. Если папки или `index.html` в ней нет, сервер не запускается с
      ошибкой «папка интерфейса не найдена: <путь>».

- [ ] **Step 1: Тесты ядра**

```rust
// orders_update.rs
#[tokio::test]
async fn status_change_can_skip_the_issue_date() -> Result<(), CoreError> {
    // change_status_with(.., issued, stamp_issued_at: false) → status issued, issued_at == None, status_changed_at == now
}
// orders_history.rs
#[tokio::test]
async fn order_history_needs_no_audit_permission() -> Result<(), CoreError> {
    // actor_with(&[Permission::OrdersEdit]) видит историю заказа (создан, статус); list_audit тому же актёру → Forbidden
}
#[tokio::test]
async fn warranty_case_moves_an_issued_order_and_writes_history() -> Result<(), CoreError> {
    // заказ выдан (issued_at = X); accept_warranty_case(complaint "Снова не заряжается") →
    //   статус «Принят по гарантии», issued_at == X, last_audit(order).action == "order.warranty_case"
    // невыданный заказ → assert_validation(res, &[("status", "invalid")])
    // пустая жалоба → assert_validation(res, &[("complaint", "required")])
    // статус «Принят по гарантии» в архиве → assert_validation(res, &[("status", "invalid")])
}
// catalogs.rs
#[tokio::test]
async fn status_color_must_be_a_palette_name() -> Result<(), CoreError> {
    // create_status(color "#ff0000") → assert_validation(res, &[("color", "invalid")]); "pink" → ok
    // update_status(name only) у статуса с цветом не из палитры (поставлен напрямую в БД) → ok
}
// settings.rs
#[tokio::test]
async fn archived_default_point_reads_as_none() -> Result<(), CoreError> { /* … */ }
```

- [ ] **Step 2: Тесты API и раздачи**

```rust
// static_ui.rs — временная папка с index.html и _app/x.js
#[tokio::test]
async fn serves_files_and_falls_back_to_index() {
    // GET /_app/x.js → 200, тело файла; GET /orders/10481 → 200, тело index.html;
    // GET /api/v1/nope → 404 JSON с code "not_found"; GET /../secret → не выходит за папку (404 или index.html)
    // без ui_dir: GET / → 404 JSON, как раньше
}
```

  - `orders.rs` (API):
    - `server-info` содержит `server_time`;
    - `history` и `warranty-case` отвечают по схеме;
    - лишнее поле в `NewWarrantyCase` → 400.
  - Тест `mw-server`: запуск с несуществующей `--ui-dir` → ошибка с путём.

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать в порядке домен → ядро → API → сервер; обновить контракт**

- [ ] **Step 5: Полная проверка Rust на SQLite и PostgreSQL** — зелёная

- [ ] **Step 6: Commit** — `feat(core,api,server): order history, warranty case, status colors, server time, UI serving`

---

### Task 5: Клиент API в интерфейсе и сквозные тесты с настоящим сервером

**Files:**
- Create: `apps/ui/src/lib/api/{client.ts,session.ts,server-clock.svelte.ts,orders.ts,catalogs.ts,clients.ts,auth.ts}`
  и тесты к ним, `apps/ui/e2e/support/server.mjs`, `apps/ui/e2e/support/api.ts`, `apps/ui/e2e/api-smoke.spec.ts`
- Modify: `apps/ui/vite.config.ts` (прокси `/api` → `process.env.MW_API_URL ?? 'http://127.0.0.1:8080'`),
  `apps/ui/playwright.config.ts`, `apps/ui/package.json` (скрипт `test:e2e` = `npm run build && playwright test`),
  `.github/workflows/ci.yml` (задание `ui-e2e`: `rustup toolchain install` и `Swatinem/rust-cache@v2`, как в задании
  `rust`, до шагов npm), `apps/ui/src/lib/i18n/ru.ts`

**Interfaces:**
- Produces:
  - `class ApiError extends Error { status: number; code: ErrorCode | 'network'; body: ErrorBody | null }`.
    Сетевая ошибка — `status` 0, `code` `'network'`, `message` = `ru.api.offline` («Нет связи с сервером»);
  - `apiUrl(path: string, query?: Record<string, string | number | boolean | null | undefined | (string | number)[]>): string`:
    - массив — повтор ключа: `status_ids=1&status_ids=2`;
    - `null` и `undefined` пропускаются;
  - `request<T>(method: 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE', path: string, opts?: { query?: …; body?: unknown; signal?: AbortSignal }): Promise<T>`:
    - тело — JSON с `Content-Type: application/json`;
    - токен из `session.ts` — в `Authorization: Bearer …`;
    - ответ 204 → `undefined`;
    - ошибка → `ApiError`;
    - ответ 401 вызывает обработчик из `setUnauthenticatedHandler(fn: () => void)`;
  - `session.ts`: `getToken(): string | null`, `setToken(token: string, remember: boolean): void` (`remember` —
    `localStorage`, иначе `sessionStorage`; ключ `mw.session`), `clearToken(): void`;
  - `serverClock`: `sync(serverTime: string): void`, `now(): LocalDateTime` — местное время предприятия по часам
    сервера (поправка = время сервера − `Date.now()` в момент `sync`). `formatRelative` в экранах получает `serverClock.now()`;
  - функции по ресурсам — тонкие обёртки над `request` с типами `$api` (версия передаётся так, как ждёт маршрут, —
    `?version=`):
    - `orders.ts`: `listOrders(q: OrderListQuery, signal?)`, `getOrder(id)`, `updateOrder(id, version, patch: OrderPatch)`,
      `changeStatus(id, version, change: StatusChange)`, `issueOrder(id, version)`, `createOrder(n: NewOrder)`,
      `ordersByBarcode(code)`, `orderSummary(pointId: number | null)`, `orderValues(field, query, brand?)`,
      `orderHistory(id, page?)`, `acceptWarrantyCase(id, version, c: NewWarrantyCase)`;
    - `catalogs.ts`: `listStatuses()`, `listServicePoints()`, `listDictionary(kind)`, `addDictionaryItem(kind, value)`,
      `getSettings()`;
    - `clients.ts`: `listClients(q)`, `clientsByPhone(phone)`, `getClient(id)`;
    - `auth.ts`: `serverInfo()`, `me()`, `login(req)`, `logout()`, `changePassword(req)`;
  - E2E: два сервера Playwright (`webServer` — массив):
    - `node e2e/support/server.mjs --port 4173` — режим без входа;
    - `node e2e/support/server.mjs --port 4174 --admin` — с администратором `e2e-admin`, пароль — константа
      `E2E_ADMIN_PASSWORD` из `e2e/support/api.ts` (тестовое значение);
    - `server.mjs` собирает `cargo build -p mw-server` (корень репозитория), создаёт временную папку данных, при
      `--admin` создаёт администратора (`mw-server create-admin`, пароль на стандартный ввод) и запускает
      `mw-server --listen 127.0.0.1:<port> --data-dir <tmp> --ui-dir apps/ui/build serve`;
    - `baseURL` по умолчанию `http://127.0.0.1:4173`;
    - тесты витрины `/design` идут на этом же сервере;
  - `e2e/support/api.ts`:
    - `uniq(prefix = 'e2e'): string` — уникальная метка;
    - `createOrder(request: APIRequestContext, over?: Partial<NewOrder> & { clientName?: string; phone?: string }): Promise<Order>`;
    - `statusId(request, name): Promise<number>`;
    - `patchOrder(request, id, patch): Promise<Order>`.

- [ ] **Step 1: Модульные тесты клиента** (`fetch` подменяется `vi.stubGlobal`)

```ts
// client.test.ts
it('repeats array keys and skips empty values', () => {
	expect(apiUrl('/orders', { status_ids: [1, 2], query: '', limit: 50, point: null }))
		.toBe('/api/v1/orders?status_ids=1&status_ids=2&query=&limit=50');
});
it('throws ApiError with the server body', async () => {
	stubFetch(409, { code: 'conflict', message: 'Заказ изменили', fields: [], current_version: 7, permission: null, retry_after_secs: null });
	await expect(request('PATCH', '/orders/1')).rejects.toMatchObject({ status: 409, code: 'conflict', body: { current_version: 7 } });
});
it('maps a network failure to code network', async () => {
	vi.stubGlobal('fetch', () => Promise.reject(new TypeError('Failed to fetch')));
	await expect(request('GET', '/x')).rejects.toMatchObject({ status: 0, code: 'network', message: 'Нет связи с сервером' });
});
it('sends the bearer token and calls the 401 handler', async () => { /* setToken('t', true); 401 → handler called once */ });
// session.test.ts
it('remember keeps the token in localStorage, otherwise sessionStorage', () => { /* … */ });
// server-clock.test.ts
it('now follows server time plus elapsed local time', () => {
	vi.useFakeTimers(); vi.setSystemTime(new Date('2026-01-01T00:00:00Z'));
	serverClock.sync('2026-11-15T10:24:00'); vi.advanceTimersByTime(90_000);
	expect(serverClock.now()).toMatchObject({ year: 2026, month: 11, day: 15, hour: 10, minute: 25 });
});
```

- [ ] **Step 2: Сквозной тест обвязки**

```ts
// api-smoke.spec.ts
test('the server serves the UI and the API on one origin', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	expect(o.id).toBeGreaterThan(0);
	await page.goto('/design');
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
});
```

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать клиент, обвязку и CI.** Прежние e2e витрины проходят на новом сервере.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): API client, session, server clock; e2e against a real mw-server`

---

### Task 6: Каркас приложения, вход, Ctrl K, сканер, отмена

**Files:**
- Create: `apps/ui/src/routes/(app)/+layout.ts`, `(app)/+layout.svelte`, `apps/ui/src/routes/login/+page.svelte`,
  `apps/ui/src/lib/app/{context.svelte.ts,undo.svelte.ts,recent.ts,palette.ts,scan.ts,AppTopBar.svelte,AppSidebar.svelte,ScanChoiceDialog.svelte,ChangePasswordDialog.svelte}`
  и тесты к `.ts`, `apps/ui/src/lib/orders/panel.svelte.ts`, `apps/ui/src/lib/orders/events.ts`,
  `apps/ui/e2e/app.spec.ts`, `apps/ui/e2e/login.spec.ts`
- Modify: `apps/ui/src/routes/+page.ts` (редирект на `/orders`), `apps/ui/src/lib/i18n/ru.ts`

**Interfaces:**
- Consumes: функции `#lib/api/*` (задача 5), `OrderSummary` (задача 3).
- Produces:
  - `(app)/+layout.ts` `load`:
    - `serverInfo()` → `serverClock.sync(server_time)`;
    - если `auth_required` и нет токена — редирект на `/login?next=<путь>`;
    - иначе `me()`, `listStatuses()`, `listServicePoints()`, `getSettings()` параллельно;
    - 401 → `clearToken()` и редирект на вход;
    - `setUnauthenticatedHandler` ведёт на вход с текущим путём;
  - `context.svelte.ts`: объект `app` с полями `server: ServerInfo`, `me: CurrentUser`, `statuses: Status[]`,
    `points: ServicePoint[]`, `settings: AppSettings`, `point: number | null` (выбранная точка в верхней панели,
    `mw.point` на устройстве) и методами `can(p: Permission): boolean`, `statusById(id)`, `refreshCatalogs()`;
  - `orderEvents`: `emitOrderChanged(o: Order): void`, `onOrderChanged(fn: (o: Order) => void): () => void` —
    список и сводка обновляются после правок из любого места;
  - `orderPanel` (`panel.svelte.ts`):
    - `open(id: number, opts?: { neighbors?: () => number[]; back?: { label: string; href: string } })`, `close()`;
    - поля `id: number | null`, `next(dir: 1 | -1)` по `neighbors`;
    - сама панель — задача 8, макет держит её место;
  - `intake`: `open(prefill?: { clientId?: number })`, `close()` — лист приёма, задача 10;
  - `undo.svelte.ts`: `undo.push(label: string, run: () => Promise<void>)`, `undo.run(): Promise<void>`. Ctrl/⌘+Z вне
    поля выполняет последнее. Кнопка «Отменить» в уведомлении вызывает тот же `run`;
  - `recent.ts`: `pushRecent(id: number)`, `recentIds(): number[]` — до 8 номеров, `mw.recent`;
  - `palette.ts`: `paletteGroups(input: { query: string; orders: OrderListItem[]; exact: Order | null; clients: Client[]; recent: OrderListItem[]; commands: CommandItem[] }): CommandGroup[]`:
    - пустой запрос — «Недавние» и «Команды»;
    - иначе «Заказы» (точное совпадение номера первым, без повтора), «Клиенты», «Команды» (`filterCommands`);
  - запросы палитры: задержка 150 мс, `AbortController` отменяет прежний:
    - `listOrders({ query, limit: 5 })`;
    - `getOrder(n)`, если запрос — число от 1 до 7 цифр (404 молча);
    - `listClients({ query, limit: 5 })`;
  - выбор в палитре: заказ — Enter открывает панель, Ctrl/⌘+Enter — `/orders/<id>`; клиент — `/orders?client=<id>`
    (список заказов клиента — экран клиентов появится в 5.2); команды — «Принять устройство», «Тема: светлая/тёмная/как в системе»,
    пункты «Требуют внимания»;
  - `scan.ts`: `resolveScan(r: ScanResult, deps: { byBarcode(code): Promise<OrderListItem[]>; byNumber(n): Promise<Order | null> }): Promise<ScanAction>`,
    где `ScanAction = { kind: 'open'; id: number } | { kind: 'choose'; orders: OrderListItem[] } | { kind: 'notFound'; text: string } | { kind: 'search'; text: string }`;
    - `choose` → `ScanChoiceDialog` (№, дата приёма, устройство, клиент; Enter выбирает);
    - `notFound` → уведомление «Заказ со штрихкодом … не найден» / «Заказ № … не найден»;
    - `search` → палитра с этим текстом;
    - сканер ставится в макете `(app)` через `installScanner`; скан в поле остаётся в поле;
  - каркас:
    - `AppTopBar` — название, поле «Найти или выполнить… Ctrl K», выбор точки («Все точки» + неархивные),
      «＋ Принять» (N), переключатель темы, меню пользователя (только если `!me.open_mode`: «Сменить пароль», «Выйти»);
    - `AppSidebar` — «Заказы» со счётчиком «В работе», «Требуют внимания» (4 пункта по `OrderSummary`; нули скрыты;
      всё по нулям — «Всё в порядке ✓»), «Мои виды», внизу ничего. Пункты «Клиенты», «Склад», «Отчёты», «Настройки»
      появятся со своими планами — мёртвых ссылок нет;
    - на телефоне `BottomNav`: «Заказы», «＋», «Ещё» (тема, выход);
    - клавиши: N — приём, `?` — справка, Ctrl K — палитра, Ctrl/⌘+Z — отмена;
  - `/login` (раздел 6.11 спеки):
    - карточка по центру: «Логин», «Пароль», «Запомнить на этом устройстве» (включено);
    - ошибка под полем пароля: `invalid_credentials` → «Неверный логин или пароль»; `too_many_attempts` →
      «Слишком много попыток. Повторите через N с»; сетевая — `ru.api.offline`;
    - после входа — на `next` или `/orders`. Если вход не нужен, `/login` ведёт на `/orders`.

- [ ] **Step 1: Модульные тесты**

```ts
// palette.test.ts
it('puts the exact order number first without a duplicate', () => {
	const g = paletteGroups({ query: '104', orders: [item(10481), item(104)], exact: order(104), clients: [], recent: [], commands: [] });
	expect(g[0]!.items.map((i) => i.id)).toEqual(['order:104', 'order:10481']);
});
it('empty query shows recent and commands', () => { /* groups ['Недавние', 'Команды'] */ });
// scan.test.ts
it('one order by barcode opens it, two ask, none says not found', async () => {
	const deps = (list: OrderListItem[]) => ({ byBarcode: async () => list, byNumber: async () => null });
	expect(await resolveScan({ kind: 'barcode', code: '012345678905' }, deps([item(1)]))).toEqual({ kind: 'open', id: 1 });
	expect((await resolveScan({ kind: 'barcode', code: '012345678905' }, deps([item(1), item(2)]))).kind).toBe('choose');
	expect(await resolveScan({ kind: 'barcode', code: '012345678905' }, deps([]))).toMatchObject({ kind: 'notFound' });
});
// undo.test.ts
it('runs the last pushed action once', async () => { /* push a, push b → run() calls b; run() again calls a */ });
// recent.test.ts
it('keeps 8 unique ids, newest first', () => { /* … */ });
```

  Тип `ScanResult` — из `scanner.ts`; имена вариантов (`barcode`, `number`, `text`) сверить с ним.

- [ ] **Step 2: Сквозные тесты**

```ts
// app.spec.ts (сервер 4173, без входа)
test('root opens the orders screen with the shell', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByRole('navigation', { name: 'Разделы' })).toContainText('Заказы');
	await expect(page.getByRole('button', { name: /Принять/ })).toBeVisible();
});
test('Ctrl K finds an order by number and Enter opens its panel', async ({ page, request }) => { /* createOrder → type id → Enter → dialog/panel title «№ <id>» */ });
test('barcode with two orders asks which one', async ({ page, request }) => {
	// два заказа через API; page.route('**/api/v1/orders/by-barcode*') отдаёт оба;
	// клавиатурой «напечатать» 12 цифр с задержкой 5 мс и Enter → диалог «Несколько заказов…» со строками обоих;
	// Enter на втором → панель «№ <второй>»
});
test('unknown barcode shows a toast', async ({ page }) => { /* 12 цифр без заказа → «Заказ со штрихкодом … не найден» */ });
test('scan into a focused field stays in the field', async ({ page }) => { /* фокус в фильтре списка, скан → текст в поле, панели нет */ });
// login.spec.ts (test.use({ baseURL: 'http://127.0.0.1:4174' }))
test('login returns to the page that asked for it', async ({ page }) => {
	await page.goto('/orders?chips=all');
	await expect(page).toHaveURL(/\/login\?next=/);
	await page.getByLabel('Логин').fill('e2e-admin');
	await page.getByLabel('Пароль').fill('неверно');
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page.getByText('Неверный логин или пароль')).toBeVisible();
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders\?chips=all$/);
});
test('logout from the user menu goes to login', async ({ page }) => { /* … */ });
```

  У `PATCH /orders` поля `barcode` нет. Поэтому общий штрихкод в тесте подставляется подменой ответа
  `GET /api/v1/orders/by-barcode` через `page.route`. Сам API общего штрихкода уже проверен в `mw-api`.

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать.** Брать из `routes/design/orders/+page.svelte` готовые куски каркаса и палитры, но
  данные — только через `#lib/api`.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): app shell, login, command palette, scanner and undo on the real server`

---

### Task 7: Главный экран «Заказы»

**Files:**
- Create: `apps/ui/src/routes/(app)/orders/+page.svelte`,
  `apps/ui/src/lib/orders/{view.ts,columns.ts,list.svelte.ts,FiltersMenu.svelte,QuickFilters.svelte}` и тесты к `.ts`,
  `apps/ui/e2e/orders.spec.ts`
- Modify: `apps/ui/src/lib/demo/orders.ts` (общие помощники переезжают в `#lib/orders/view.ts`, демо импортирует
  их оттуда), `apps/ui/src/lib/app/AppSidebar.svelte`, `ru.ts`

**Interfaces:**
- Consumes: `listOrders`, `orderSummary`, `orderValues` (задачи 3, 5); `app`, `orderPanel`, `orderEvents` (задача 6).
- Produces:
  - `view.ts`:
    - `deviceOf`, `estimateText`, `toPay` — переезжают из демо без изменений;
    - `interface OrdersView { chips: string[]; attention: AttentionKey | null; query: string; filters: OrderFilters; sort: SortState; clientId: number | null }`;
    - `AttentionKey = 'call' | 'parts' | 'longDiagnostics' | 'longReady'`;
    - `interface OrderFilters { deviceType?: string; brand?: string; master?: string; pointId?: number; statusIds?: number[]; received?: [string | null, string | null]; issued?: [string | null, string | null]; needCall?: boolean; needParts?: boolean }`;
    - `viewToQuery(v: OrdersView, statuses: Status[], pointId: number | null): OrderListQuery`:
      - фишка `work` — статусы видов `open` и `diagnostics`; `issued` — `issued: 'issued'`; `all` — без фильтра
        статуса; иначе фишки — номера статусов;
      - `attention` подменяет фишки: `call` → `need_call` + `in_service`, `parts` → `need_parts` + `in_service`,
        `longDiagnostics` → `overdue_diagnostics`, `longReady` → `stale_ready`;
      - точка верхней панели и точка из фильтров — одно поле, фильтр важнее;
    - `viewFromSearch(p: URLSearchParams): OrdersView`, `viewToSearch(v: OrdersView): string` — вид живёт в адресе:
      назад/вперёд и «Мои виды» работают. По умолчанию — `chips=work`, сортировка «Принят», новые сверху;
    - `quickFilterItems(statuses: Status[], summary: OrderSummary): { main: QuickFilterItem[]; more: QuickFilterItem[] }`:
      - «В работе» (сумма видов `open` и `diagnostics`);
      - неархивные статусы по порядку со счётчиками;
      - статусы без заказов — в `more`;
      - «Выданы», «Все»;
    - «Мои виды» — `mw.views` на устройстве: `{ name: string; search: string }[]`, «Сохранить вид…» в меню
      «Фильтры ▾», удаление — крестик в пункте сайдбара;
  - `columns.ts`:
    - `orderColumns(ctx): Column<OrderListItem>[]`;
    - видны: `id`, `received`, `client`, `device`, `malfunction`, `status`, `master`, `amount`, `flags`;
    - скрыты: `phone`, `type`, `serial`, `point`, `issued`, `prepayment`, `barcode`;
    - `SORT_FIELD: Record<string, OrderSortField>` = `{ id: 'id', received: 'received_at', client: 'client_name', device: 'brand', type: 'device_type', status: 'status', master: 'master', amount: 'final_price', issued: 'issued_at' }`;
    - остальные колонки не сортируются;
    - ячейки как в демо: «Сумма» (итог / «≈ от–до» приглушённо / «—»), красная точка чёрного списка, часы у долгой
      диагностики;
  - `list.svelte.ts`: `createOrderList(): { rows: OrderListItem[]; total: number; loading: boolean; error: ApiError | null; load(q: OrderListQuery): void; loadMore(): void; replace(o: Order): void }`:
    - страница по 100;
    - каждый `load` отменяет прежний запрос;
    - `loadMore` создаёт новый массив `rows` (требование `DataTable.onendreached`);
    - `replace` обновляет поля строки из `Order`, клиентские поля не трогает;
    - после `replace` строка, которая перестала подходить под вид, остаётся до следующей загрузки, чтобы список не
      прыгал под рукой;
  - страница `/orders`:
    - строка быстрых фильтров (клик — один фильтр, Ctrl/⌘+клик добавляет статус);
    - поле фильтра (`TextInput` с `ariaLabel`, клавиша `/`, задержка 250 мс);
    - «Фильтры ▾» (`FiltersMenu`: тип, бренд, мастер — `Combobox` по `orderValues`; точка; статусы; период приёма;
      период выдачи; отметки) и чипы выбранных с крестиком;
    - чип «Клиент: …» при `client`;
    - `DataTable` в родителе с определённой высотой;
    - выделение сбрасывается при смене вида;
    - Enter или клик открывают `orderPanel.open(id, { neighbors: () => rows.map((r) => r.id) })`;
    - сортировка — `onsortchange` → `view.sort`;
    - на телефоне — `InfiniteList` из `ListCard` (как в демо);
    - пусто — `EmptyState` «Пока нет заказов. Примите первое устройство (N) или перенесите базу MyWork 2», а при
      фильтрах — «Ничего не нашлось» и «Сбросить фильтры»;
    - `orderEvents` → `list.replace` и обновление сводки;
    - сводка — `orderSummary(app.point)` при входе, при смене точки и после правок (не чаще раза в секунду).

- [ ] **Step 1: Модульные тесты**

```ts
// view.test.ts
it('work chip asks for open and diagnostics statuses', () => {
	const q = viewToQuery(defaultView(), STATUSES, null);
	expect(q.status_ids).toEqual(STATUSES.filter((s) => s.kind === 'open' || s.kind === 'diagnostics').map((s) => s.id));
	expect(q).toMatchObject({ sort: 'received_at', descending: true });
});
it('attention replaces chips', () => {
	expect(viewToQuery({ ...defaultView(), attention: 'call' }, STATUSES, null)).toMatchObject({ need_call: true, issued: 'in_service', status_ids: undefined });
	expect(viewToQuery({ ...defaultView(), attention: 'longReady' }, STATUSES, null)).toMatchObject({ stale_ready: true });
});
it('round-trips through the address', () => {
	const v = { ...defaultView(), query: 'a52', filters: { brand: 'Samsung', received: ['2026-11-01T00:00:00', null] } };
	expect(viewFromSearch(new URLSearchParams(viewToSearch(v)))).toEqual(v);
});
it('statuses without orders go to more', () => { /* summary без «Согласовано» → оно в more */ });
// list.test.ts (подменённый listOrders)
it('a new load aborts the previous one and loadMore makes a new array', async () => { /* … */ });
```

- [ ] **Step 2: Сквозные тесты**

```ts
// orders.spec.ts — данные с меткой m = uniq(); фильтр «/» по m отделяет свои заказы от чужих
test('quick filters and counts follow the server', async ({ page, request }) => {
	// 2 заказа в «Диагностика», 1 в «Готов» с моделью m; /orders; фильтр «/» = m → в «В работе» 2 строки;
	// клик «Готов» → 1 строка; Ctrl+клик «Диагностика» → 3; счётчик фишки «Готов» ≥ 1
});
test('filters menu narrows by brand ignoring case', async ({ page, request }) => {
	// бренды «samsung» и «Samsung» с моделью m; «Фильтры ▾» → Бренд «SAMSUNG» → 2 строки, чип «Бренд: SAMSUNG ✕»
});
test('sorting by device and hiding a column survive a reload', async ({ page }) => { /* … */ });
test('«Позвонить» in «Требуют внимания» shows orders that need a call', async ({ page, request }) => { /* need_call=true */ });
test('back button restores the previous view', async ({ page }) => { /* «Готов» → «Все» → назад → «Готов» */ });
test('phone layout shows cards', async ({ page }) => { /* page.setViewportSize({ width: 390, height: 844 }) → ListCard с № */ });
```

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать.** Брать разметку и стили из демо-экрана, данные — с сервера.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): orders screen on the server: quick filters, attention, filters, columns, saved views`

---

### Task 8: Панель заказа и выдача

**Files:**
- Create: `apps/ui/src/lib/orders/{autosave.ts,merge.ts,OrderPanel.svelte,OrderFields.svelte,OrderHistory.svelte,IssueDialog.svelte,ReturnToWorkDialog.svelte}`
  и тесты к `.ts`, `apps/ui/e2e/order-panel.spec.ts`
- Modify: `apps/ui/src/routes/(app)/+layout.svelte` (панель на месте, приготовленном в задаче 6), `ru.ts`

**Interfaces:**
- Consumes: `getOrder`, `updateOrder`, `changeStatus`, `issueOrder`, `orderHistory`, `getClient`, `listDictionary`
  (задача 5); `orderPanel`, `orderEvents`, `undo`, `app` (задача 6).
- Produces:
  - `autosave.ts`:

    ```ts
    export type SaveState = 'idle' | 'saving' | 'saved' | 'error';
    export interface Autosave<T extends object> {
    	readonly state: SaveState;
    	readonly error: ApiError | null;
    	/** Текст: уйдёт через delayMs после последней правки или при flush(). */
    	edit<K extends keyof T>(field: K, value: T[K]): void;
    	/** Выбор и переключатели: уходит сразу, вместе с ждущими текстовыми правками. */
    	commit<K extends keyof T>(field: K, value: T[K]): void;
    	flush(): Promise<void>;
    	retry(): void;
    	/** Значение, которое должно быть в поле: своё несохранённое или с сервера. */
    	value<K extends keyof T>(field: K, server: T[K]): T[K];
    	dispose(): void;
    }
    export function createAutosave<T extends object>(opts: {
    	save: (patch: Partial<T>) => Promise<void>;
    	delayMs: number; // 600
    	onstate?: (s: SaveState) => void;
    }): Autosave<T>;
    ```

    - одновременно в пути не больше одного запроса;
    - правки, сделанные во время запроса, уходят следующим запросом одним патчем;
    - после ответа поле показывает своё, только если его правили после отправки;
    - ошибка оставляет все правки ждущими, `retry` досылает их;
    - конфликт (`code` `conflict`) передаётся наверх через отклонённый `flush` и `error`, правки остаются;
  - `merge.ts`: `mergeRemote<T>(base: T, local: Partial<T>, remote: T): { take: Partial<T>; conflicts: (keyof T)[]; changedByOthers: (keyof T)[] }`:
    - поле, которое правил только другой, — в `take` и `changedByOthers`;
    - поле, которое правили оба разными значениями, — в `conflicts`;
    - одинаковые правки — не конфликт;
  - `OrderPanel` (раздел 6.2 спеки) — `SlideOver` поверх страницы, без затемнения:
    - шапка: «№ N · устройство», ‹ ›, ⤢ (`/orders/N`), ✕; «принят …, точка, индикатор сохранения» («Сохраняю…»,
      «✓ Сохранено», «Не сохранено · Повторить»);
    - `StatusPicker`: виды `issued` вето, вместо них открывается `IssueDialog`. После смены — уведомление
      «№ N → <статус> · Отменить» и `undo.push`. Отмена — `changeStatus` назад с новой версией;
    - выданный заказ: статус заблокирован, в ⋯ — «Вернуть в работу…» (`ReturnToWorkDialog`: список невыданных
      статусов, по умолчанию — статус до выдачи из истории (`order.status_changed` с `details.to` = текущий статус),
      иначе первый) и «Принять по гарантии…» (задача 9);
    - тело — `OrderFields` (общий с полной карточкой):
      - «Клиент»: имя (`displayName`), чёрный список, телефоны из `getClient` — основной, «ещё N номеров»;
      - «Устройство»: тип, бренд, модель, цвет, серийный/IMEI, пароль (скрыт, «показать»);
      - «Неисправность»;
      - «Работа»: выполнено, мастер (`Combobox` по справочнику `master`);
      - «Деньги»: оценка и предоплата текстом; итог, скидка, расходы — `MoneyInput`; «К оплате» = итог − предоплата −
        скидка, отрицательное — ошибкой;
      - «Гарантия» — `Combobox` по справочнику `warranty`;
      - «Отметки» — два `Switch`, сохраняются сразу;
      - «Комментарий (только для сервиса)»;
      - текст сохраняется через 600 мс или при уходе из поля, выбор — сразу;
      - пока у `MoneyInput` `invalid`, его значение не отправляется;
    - «История» — `OrderHistory` (три последних записи, «все» → `/orders/N`);
    - подвал: «Выдать» (главная, если вид статуса `ready`; обычная в остальных; у выданного нет), ⋯.
      «Печать ▾» появится в плане 6;
    - на телефоне (раздел 6.12) панель на весь экран: сверху «←», номер, ⋯; статус — кнопка, по ней снизу
      выезжают статусы (`Dialog` `position="bottom"`); телефоны — ссылки `tel:`; нижняя полоса «☎ Позвонить» и
      «Статус» («📷 Фото» — в 5.2); e2e «phone panel calls and changes status» на ширине 390;
    - клавиши: ‹ › и ↑↓ вне поля, Alt+↑/↓ всегда — соседний заказ (сначала `flush`). Esc — закрыть открытое меню,
      потом панель. Закрытие и переход ждут `flush`; если он не удался, панель остаётся открытой с «Не сохранено ·
      Повторить»;
    - конфликт:
      - `getOrder` → `mergeRemote` → `take` применяется молча, поля из `changedByOthers` коротко подсвечены;
      - при `conflicts` сверху `Callout` «Заказ изменили на другом компьютере» и у каждого спорного поля
        «Оставить моё» / «Взять новое»;
      - «Оставить моё» досылает своё с новой версией;
    - каждый ответ сервера → `emitOrderChanged`;
  - `IssueDialog` (раздел 6.4) — перенос из демо:
    - пустые «Выполненные работы», «Мастер», «Итог» заполняются прямо в диалоге (сначала `updateOrder`, затем `issueOrder`);
    - расчёт и крупное «К оплате»;
    - гарантия по умолчанию из `app.settings.default_warranty`. «0 мес» и «0 дней» — «без гарантии»: проверяется
      `warranty_term.value == 0`, единица не важна;
    - «Выдать» — Ctrl/⌘+Enter (`submitShortcut`);
    - ошибки проверки сервера (`fields`) показываются у полей;
    - после выдачи — уведомление с «Отменить»: статус назад; если даты выдачи не было, `updateOrder({ issued_at: null })`
      (нужно право `order_dates_edit`; без права дата остаётся, уведомление об этом не нужно);
    - переключатель «Печатать акт выдачи» появится в плане 6.

- [ ] **Step 1: Модульные тесты**

```ts
// autosave.test.ts (vi.useFakeTimers; save — управляемое обещание)
it('debounces text and sends two fields in one patch', async () => {
	a.edit('malfunction', 'x'); vi.advanceTimersByTime(300); a.edit('work_done', 'y'); vi.advanceTimersByTime(600);
	expect(save).toHaveBeenCalledWith({ malfunction: 'x', work_done: 'y' });
});
it('commit sends at once together with pending text', () => { /* edit + commit → один вызов с обоими полями */ });
it('never has two requests in flight', async () => { /* edit, flush (в пути), edit, advance → второй save только после ответа первого */ });
it('stale response keeps newer local text', async () => {
	a.edit('malfunction', 'не за'); await flushTimers(); // запрос 1 в пути
	a.edit('malfunction', 'не заряжается');            // набрано позже
	resolve1();
	expect(a.value('malfunction', 'не за')).toBe('не заряжается');
});
it('failure keeps changes and retry resends', async () => { /* reject → state 'error'; retry → save с той же правкой */ });
// merge.test.ts
it('splits remote changes into taken and conflicting fields', () => {
	const r = mergeRemote({ a: 1, b: 1, c: 1 }, { a: 2, c: 5 }, { a: 3, b: 2, c: 5 });
	expect(r).toEqual({ take: { b: 2 }, conflicts: ['a'], changedByOthers: ['b'] });
});
```

- [ ] **Step 2: Сквозные тесты** (перенести смысл тестов панели из `orders-demo.spec.ts`)

```ts
// order-panel.spec.ts
test('edits save by themselves and survive a reload', async ({ page, request }) => {
	// открыть заказ; «Неисправность» += «!»; «✓ Сохранено»; перезагрузка → в поле есть «!»
});
test('typing during a slow save keeps the text', async ({ page, request }) => {
	// page.route('**/api/v1/orders/*', задержать первый PATCH на 1,5 с); набрать «не за», пауза 700 мс, дописать «ряжается» →
	// после обоих ответов поле «не заряжается», на сервере тоже
});
test('offline save shows retry and retry saves', async ({ page, request }) => { /* route.abort() на PATCH → «Не сохранено · Повторить» → снять → «Повторить» → «✓ Сохранено» */ });
test('conflict callout lets me take the new value', async ({ page, request }) => {
	// открыть заказ; patchOrder(request) меняет «Неисправность»; в панели правим ту же «Неисправность» →
	// Callout «Заказ изменили на другом компьютере»; «Взять новое» → поле показывает значение из API
});
test('status change shows a toast and undo restores it', async ({ page, request }) => { /* … */ });
test('Alt+↓ moves to the next order and money typed does not leak into it', async ({ page, request }) => {
	// регрессия плана 4: набрать сумму в «Итог», сразу Alt+↓ → у следующего заказа итог прежний, у первого — набранный
});
test('issue fills required fields in the dialog, issues and undo brings it back', async ({ page, request }) => { /* … */ });
test('Esc closes an open menu first, then the panel', async ({ page }) => { /* … */ });
```

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать.** Разметку брать из `routes/design/orders/OrderPanel.svelte` и `IssueDialog.svelte`.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): order panel with autosave, conflicts, status undo and issue`

---

### Task 9: Массовые действия, полная карточка, приём по гарантии

**Files:**
- Create: `apps/ui/src/lib/orders/{bulk.ts,WarrantyCaseDialog.svelte,BulkIssuedDateDialog.svelte}`, `bulk.test.ts`,
  `apps/ui/src/routes/(app)/orders/[id]/+page.svelte`, `apps/ui/e2e/order-card.spec.ts`, `apps/ui/e2e/bulk.spec.ts`
- Modify: `apps/ui/src/routes/(app)/orders/+page.svelte` (панель выделения), `OrderPanel.svelte` (пункт ⋯), `ru.ts`

**Interfaces:**
- Consumes: всё из задач 5–8; `acceptWarrantyCase` (задачи 4, 5).
- Produces:
  - `bulk.ts`: `runBulk(rows: { id: number; version: number }[], apply: (row) => Promise<Order>): Promise<{ done: Order[]; failed: { id: number; error: ApiError }[] }>`:
    - по одному заказу подряд;
    - при `conflict` берёт свежую версию (`getOrder`) и повторяет один раз;
    - результат каждого → `emitOrderChanged`;
  - панель выделения:
    - «Статус ▾» — все неархивные статусы. Вид `issued` сначала спрашивает дату выдачи (`BulkIssuedDateDialog`:
      «Сейчас» / «Не ставить» → `stamp_issued_at`). Массовое меню без `issued` из плана 4 расширяется;
    - «Мастер ▾» — справочник `master` и «Снять мастера»;
    - «Отметки ▾» — «Позвонить: поставить/снять», «Заказать запчасть: поставить/снять»;
    - после действия — уведомление «Статус «Готов» у 3 заказов · Отменить» (`plural`) и `undo.push` с возвратом
      прежних значений по каждому заказу;
    - если часть не удалась — «Не удалось у 1 заказа: № 10481 — <сообщение>»;
    - «Печать ▾» появится в плане 6;
  - `/orders/[id]` (раздел 6.3 спеки):
    - шапка: «←» (назад по истории, без неё — `/orders`), номер `--text-2xl`, устройство, `StatusPicker`, «Выдать», ⋯;
    - слева `OrderFields`;
    - справа «Статусы и сроки» (`Timeline` из истории: `order.created`, `order.status_changed`, `legacy.status`,
      `order.warranty_case`), «Гарантийные случаи» (записи `order.warranty_case`: дата и жалоба), «История изменений»
      (вся история страницами по 50, «Показать ещё»);
    - уже 1024px — вкладки «Заказ», «История»;
    - «Фото» и «Файлы» появятся в 5.2;
    - открытие — ⤢ в панели и Ctrl/⌘+Enter в палитре; `pushRecent`;
  - `WarrantyCaseDialog`:
    - «Жалоба» (`Textarea`, обязательно) и «Дата» (`DateInput`, по умолчанию сегодня по `serverClock`);
    - «Принять» — Ctrl/⌘+Enter → `acceptWarrantyCase`;
    - ошибки сервера у полей (`status` — над формой);
    - доступен в ⋯ панели и карточки только у выданного заказа.

- [ ] **Step 1: Модульные тесты**

```ts
// bulk.test.ts
it('retries a conflict once with the fresh version', async () => {
	// apply: 1-й вызов для id 2 → ApiError conflict; getOrder(2) → version 9; 2-й вызов с version 9 → ok
	expect(r.done.map((o) => o.id)).toEqual([1, 2]); expect(r.failed).toEqual([]);
});
it('collects failures and goes on', async () => { /* id 2 → validation → failed [{id: 2}], id 3 сделан */ });
```

- [ ] **Step 2: Сквозные тесты**

```ts
// bulk.spec.ts
test('bulk issue without date leaves the issue date empty', async ({ page, request }) => {
	// 2 заказа с моделью m, мастером и работой; выделить оба → «Статус ▾» → «Выдан» → «Не ставить» →
	// уведомление «… у 2 заказов»; через API у обоих issued_at === null
});
test('bulk master change and undo', async ({ page, request }) => { /* … */ });
// order-card.spec.ts
test('full card opens from the panel and shows status history', async ({ page, request }) => { /* ⤢ → URL /orders/<id>, «Статусы и сроки» содержит смену статуса */ });
test('warranty case on an issued order', async ({ page, request }) => {
	// выданный заказ → ⋯ «Принять по гарантии…» → жалоба → «Принять» → статус «Принят по гарантии», «Гарантийные случаи» содержит жалобу
});
test('narrow card uses tabs', async ({ page }) => { /* ширина 900 → вкладки «Заказ», «История» */ });
```

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать**

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): bulk status, master and flags; full order card; warranty case`

---

### Task 10: Приём устройства

**Files:**
- Create: `apps/ui/src/lib/intake/{draft.ts,draft.test.ts,IntakeSheet.svelte,ClientStep.svelte}`, `apps/ui/e2e/intake.spec.ts`
- Modify: `apps/ui/src/routes/(app)/+layout.svelte` (лист на месте, приготовленном в задаче 6),
  `apps/ui/src/lib/demo/intake.ts` (общее переезжает в `#lib/intake/draft.ts`), `ru.ts`

**Interfaces:**
- Consumes: `clientsByPhone`, `getClient`, `listClients`, `listOrders`, `createOrder`, `listDictionary`,
  `addDictionaryItem`, `orderValues` (задача 5); `app`, `intake`, `orderPanel`, `orderEvents` (задача 6).
- Produces:
  - `draft.ts`:
    - `IntakeDraft` (из демо) с полями клиента: `clientId: number | null`, `fullName`, `phones: ClientPhone[]`,
      `source`, `address`;
    - `INTAKE_DRAFT_KEY = 'mw.intake.draft'`, `parseDraft`, `emptyDraft(settings: AppSettings, statuses: Status[])`:
      мастер и гарантия — из настроек, статус — первый неархивный вида `diagnostics`;
    - `missingRequired(d): RequiredKey[]` — телефон или ФИО, тип устройства, неисправность;
    - `toNewOrder(d: IntakeDraft, pointId: number | null): NewOrder` — `client` = `{ existing: id }` или
      `{ new: NewClient }`; пустые строки обрезаны; деньги в копейках;
  - `IntakeSheet` (раздел 6.5 спеки) — `Sheet` шириной `min(70rem, 100vw − 4rem)`:
    - «Клиент» (`ClientStep`):
      - первое поле — телефон;
      - от 5 цифр `clientsByPhone` с задержкой 300 мс;
      - найденные — карточками: все телефоны, чёрный список (`Callout` `danger`), число заказов (`listOrders({ client_id, limit: 1 }).total`),
        «откуда узнали», заметка;
      - «Это он» выбирает клиента, «Новый клиент» — форма: ФИО, телефоны с «＋ номер» и пометкой, «откуда узнали»
        (`Combobox` по `client_source`), адрес под «ещё»;
    - «Устройство»:
      - тип и бренд — `Combobox` по справочникам и `orderValues`;
      - модель — `orderValues('model', текст, бренд)`;
      - цвет — справочник;
      - серийный/IMEI — скан остаётся в поле;
      - пароль — `revealable`;
      - комплектность и состояние — `ChipGroup` по справочникам `completeness` и `condition`;
      - «+ своё» добавляет значение в заказ и, если `app.can('settings_manage') || app.can('orders_create')`, в
        справочник (`addDictionaryItem`; ошибка не мешает приёму);
    - «Неисправность» — поле и чипы справочника `malfunction`;
    - «Работа и деньги»: мастер, статус, оценка «от — до», предоплата, гарантия, комментарий;
    - справа:
      - сводка незаполненного;
      - прошлые заказы выбранного клиента (`listOrders({ client_id, limit: 5 })`, клик — панель);
      - «Этот IMEI уже был» (`listOrders({ query: serial, limit: 5 })` от 5 символов). Если там есть выданный заказ
        с действующей гарантией (`issued_at` + `warranty_term` позже `serverClock.now()`), пометка «на гарантии до <дата>»;
    - внизу «Отмена» и «Принять» (Ctrl/⌘+Enter). Переключатели «Акт приёма» и «Наклейка» появятся в плане 6;
    - «Приёмщик: <имя>» — только если `!app.me.open_mode`;
    - точка заказа — выбранная в верхней панели, иначе `settings.default_service_point_id`;
    - черновик пишется на устройство при каждой правке (`writeStorage`, задержка 300 мс) и восстанавливается при
      открытии. Esc и ✕ закрывают с сохранением черновика, «Отмена» очищает его. После «Принять» черновик очищается;
    - после приёма — уведомление «Принят № N» с «Открыть» (панель), `emitOrderChanged`, лист закрывается;
    - ошибки проверки сервера — у полей;
    - если «Принять» не прошёл из-за сети, черновик цел, ошибка — над кнопкой.

- [ ] **Step 1: Модульные тесты**

```ts
// draft.test.ts
it('builds a new client order with kopecks and trimmed text', () => {
	const d = { ...emptyDraft(SETTINGS, STATUSES), fullName: ' Пётр ', phones: [{ phone: '89211234567', note: '' }], deviceType: 'Телефон', malfunction: 'не заряжается', prepayment: 200_000 };
	expect(toNewOrder(d, 1)).toMatchObject({ client: { new: { full_name: 'Пётр', phones: [{ phone: '89211234567', note: '' }] } }, service_point_id: 1, prepayment: 200_000 });
});
it('uses the existing client id', () => { /* clientId 5 → client: { existing: 5 } */ });
it('empty draft takes master, warranty and the first diagnostics status', () => { /* … */ });
it('requires a phone or a name, a device type and a malfunction', () => { /* … */ });
it('garbage in storage gives an empty draft', () => { /* parseDraft('{') */ });
```

- [ ] **Step 2: Сквозные тесты**

```ts
// intake.spec.ts
test('N opens intake, a new client order appears in the list', async ({ page }) => {
	// /orders; N; телефон, ФИО uniq(), тип «Телефон», неисправность; Ctrl+Enter → «Принят № …»; «Открыть» → панель с этим ФИО
});
test('an existing client is found by a second phone', async ({ page, request }) => {
	// createOrder c клиентом с двумя номерами; в приёме ввести второй номер → карточка клиента с обоими номерами → «Это он»
});
test('draft survives a reload', async ({ page }) => { /* ввести неисправность, Esc, reload, N → неисправность на месте */ });
test('missing required fields block «Принять» and are listed', async ({ page }) => { /* … */ });
test('typing right after opening keeps the caret in WebKit', async ({ page }) => { /* N и сразу набор → текст в поле телефона */ });
```

- [ ] **Step 3: Запустить — тесты падают**

- [ ] **Step 4: Реализовать.** Разметку брать из `routes/design/intake/+page.svelte`.

- [ ] **Step 5: Проверка** `npm run verify && npm run test:e2e` — зелёные

- [ ] **Step 6: Commit** — `feat(ui): device intake on the server with client search, suggestions and draft`

---

### Task 11: Уборка демо, правила, документы, финальная проверка

**Files:**
- Delete: `apps/ui/src/routes/design/orders/`, `apps/ui/src/routes/design/intake/`, `apps/ui/e2e/orders-demo.spec.ts`,
  `apps/ui/e2e/intake-demo.spec.ts`, `apps/ui/src/lib/demo/{orders,intake,photos}.ts` и их тесты — если витрина
  ими не пользуется (`grep -rn "lib/demo" apps/ui/src`); `fake.ts` остаётся для разделов витрины
- Modify: `apps/ui/src/routes/design/+page.svelte` (ссылки на демо → на `/orders`), `AGENTS.md`, `CONTRIBUTING`
  (если там есть раздел про интерфейс), `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (раздел 11 —
  решения 13–20 ниже, раздел 12 — закрытые заметки), `docs/superpowers/plans/README.md`

- [ ] **Step 1: Удалить демо.** Убедиться, что каждый тест из `orders-demo.spec.ts` и `intake-demo.spec.ts` имеет
  замену в `orders.spec.ts`, `order-panel.spec.ts`, `bulk.spec.ts`, `intake.spec.ts`. Недостающие — перенести на
  настоящие экраны.

- [ ] **Step 2: `AGENTS.md`, раздел интерфейса — добавить:**
  - `fetch` — только в `src/lib/api/client.ts`, данные — через `#lib/api/*.js`;
  - сквозные тесты идут на настоящем `mw-server` (`e2e/support/server.mjs`), данные создают через API с меткой `uniq()`;
  - вид списка живёт в адресе (`viewToSearch`);
  - автосохранение — только через `createAutosave`.

- [ ] **Step 3: Спека интерфейса, раздел 11 — решения Claude:**
  13. Интерфейс раздаёт сам сервер (`--ui-dir`), API того же источника; CORS не нужен.
  14. «Давно готовы» — по `orders.status_changed_at` (последняя смена статуса; для старых баз — последняя запись
      `legacy.status`), без неё — по дате приёма.
  15. Ключи `*_key` (нормализованный текст) — фильтры и сортировка по типу, бренду, модели, мастеру без учёта регистра.
  16. Фильтр по модели — через `/`; в «Фильтры ▾» — тип, бренд, мастер.
  17. Панель заказа и лист приёма живут в общем макете и открываются с любой страницы.
  18. Массовые действия — запросами по одному заказу, конфликт повторяется один раз со свежей версией.
  19. «Вернуть в работу» выбирает статус, по умолчанию — статус до выдачи.
  20. Гарантийный случай — запись журнала `order.warranty_case` и статус «Принят по гарантии» по имени; дата
      выдачи остаётся прежней.
  Ещё: «Клиенты», «Склад», «Отчёты», «Настройки» в меню появляются со своими планами; печать (переключатели актов,
  «Печать ▾») — план 6.

- [ ] **Step 4: Дорожная карта `README.md`:**
  - строка плана 5 заменяется строками 5.1–5.4:
    - 5.1 — этот план, «выполнен»;
    - 5.2 «Фото и клиенты» — вложения (таблица `attachments`, загрузка с поворотом по EXIF, удалением метаданных,
      уменьшением и миниатюрами), веб-камера и файлы в приёме, панели и карточке, регистрация `ClientFiles` из
      `legacy_rows`, экран клиентов, похожие и объединение;
    - 5.3 «Настройки и справочники» — разделы 6.7–6.8: справочники («используется в N», объединение,
      «переименовать и в заказах», архив, порядок), статусы (цвет из 12, предупреждение о смене вида), точки, общие
      настройки и внешний вид;
    - 5.4 «Десктоп, первый запуск и импорт» — Tauri 2 со встроенным сервером на `127.0.0.1:0`, раздающим интерфейс,
      логи в файл, первый запуск (раздел 6.9), импорт из интерфейса в процессе сервера с прогрессом и отчётом;
  - заметки к плану 5 разнести по 5.2–5.4; закрытые этим планом — удалить;
  - новые заметки, найденные при выполнении, — дописать.

- [ ] **Step 5: Финальная проверка всего**

```bash
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace
MW_TEST_PG_URL=postgres://postgres:postgres@localhost:55432/postgres cargo test --workspace
cd apps/ui && npm run verify && npm run test:e2e
```

Expected: всё зелёное.

- [ ] **Step 6: Commit** — `docs: plan 5.1 done; roadmap split into 5.1–5.4; UI rules for API and e2e`

---

## После плана

- Итоговое ревью всей ветки (`opus`), одна волна исправлений.
- Ветку — на GitHub для CI; после зелёного CI — перемотка `main`, `git push origin main`, удаление worktree и ветки.
- Доложить автору коротко: что работает, «Решения, которые я принял», что дальше (5.2).
