# План 4. Дизайн-система — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** в `apps/ui` появляется Svelte-интерфейс с дизайн-системой MyWork 3. В нём:
- токены светлой и тёмной темы;
- шрифт;
- настройки устройства (тема, масштаб, плотность);
- форматирование денег, телефонов, имён и дат;
- все компоненты из раздела 4 спеки интерфейса;
- витрина `/design` и демо-экраны: главный экран с панелью заказа, приём, телефонная раскладка — на вымышленных
  данных.

Всё проверено модульными тестами (Vitest) и сквозными (Playwright, Chromium и WebKit).

**Architecture:**
- SvelteKit 3 в режиме SPA (`ssr = false`, adapter-static с `fallback: 'index.html'`), Svelte 5 на рунах.
- Токены — CSS-переменные в `src/lib/design/tokens.css`. Тему выбирает атрибут `data-theme` у `<html>`, его
  ставит встроенный скрипт в `app.html` до первой отрисовки.
- Компоненты лежат в `src/lib/ui`. Сложные примитивы (Select, Combobox, Dialog, Popover, DropdownMenu, Tooltip,
  Tabs, Command, DatePicker) взяты из bits-ui и стилизованы токенами.
- Логика без вида — в `src/lib/design` и `src/lib/format`, это чистые функции с тестами.
- Витрина `/design` и демо `/design/orders`, `/design/intake` — часть приложения, по ним идут сквозные тесты.
- С сервером план не работает. Типы API берутся из `crates/mw-api/bindings` через псевдоним `$api`, только
  `import type`.

**Tech Stack:** Node ≥ 22.17 (CI — 24), npm. Версии:

| Пакет | Версия | Пакет | Версия |
|---|---|---|---|
| `svelte` | 5.57.3 | `@sveltejs/kit` | 3.2.0 |
| `@sveltejs/adapter-static` | 4.0.1 | `@sveltejs/vite-plugin-svelte` | 7.3.1 |
| `vite` | 8.3.8 | `typescript` | **6.0.3** |
| `bits-ui` | 2.21.0 | `@internationalized/date` | 3.12.4 |
| `@lucide/svelte` | 1.73.0 | `@fontsource-variable/inter` | 5.3.0 |
| `@tanstack/svelte-virtual` | 3.13.52 | `svelte-check` | 4.7.8 |
| `eslint` | 10.15.0 | `typescript-eslint` | 8.72.2 |
| `eslint-plugin-svelte` | 3.24.1 | `prettier` | 3.10.1 |
| `prettier-plugin-svelte` | 4.1.1 | `vitest` | 5.1.0 |
| `jsdom` | 31.0.1 | `@testing-library/svelte` | 5.4.2 |
| `@testing-library/jest-dom` | 7.0.1 | `@testing-library/user-event` | 14.6.10 |
| `@playwright/test` | 1.65.1 | | |

TypeScript 7 не брать: SvelteKit 3 требует `^6.0.0`, typescript-eslint — `<6.1.0`.

**Spec:**
- [`docs/superpowers/specs/2026-11-15-ui-design-system-design.md`](../specs/2026-11-15-ui-design-system-design.md) —
  спека интерфейса, все значения токенов и поведение компонентов берутся оттуда;
- раздел 5.0 основной спеки
  [`2026-11-11-mywork3-crossplatform-design.md`](../specs/2026-11-11-mywork3-crossplatform-design.md);
- дорожная карта [`README.md`](README.md).

## Global Constraints

- **Ветка и место.** Корень репозитория — `/Users/scrypto/dev/MyWork2`. Работа идёт в git worktree
  `/Users/scrypto/dev/MyWork2/.superpowers/worktrees/plan-4`, ветка `plan-4-design-system`. Все команды запускаются
  из этой папки или её `apps/ui`. Удалённого репозитория нет, ничего не отправляется.
- **Rust-часть не меняется.** Полная проверка Rust остаётся зелёной:
  `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`.
  Её прогоняет последняя задача.
- **Перед каждым коммитом** в `apps/ui` зелёные `npm run verify` (svelte-check, Prettier, ESLint, Vitest, сборка,
  лицензии) и `npm run test:e2e` (Playwright, Chromium и WebKit).
- **Svelte 5 только на рунах:** `$props`, `$state`, `$derived`, `$effect`, `{#snippet}`. Без `export let`, `$:`,
  `<slot>` и `createEventDispatcher`.
- **TypeScript строгий** (`strict`, `noUncheckedIndexedAccess`), без `any`. Обработчики — пропсы
  `onxxx` (`onclick`, `onopen`, `onclose`, …).
- **Цвета, размеры, тени и длительности — только через токены.** Литералов цвета (`#…`, `rgb(…)`, `hsl(…)`) нет
  нигде, кроме `src/lib/design/tokens.css`. Это проверяет тест `no-raw-colors` (задача 2).
- **Тексты для человека — по-русски.** Общие строки компонентов лежат в `src/lib/i18n/ru.ts`. Код и идентификаторы —
  английские.
- **Иконки** — только `@lucide/svelte`, по одной: `import Phone from '@lucide/svelte/icons/phone'`.
- **Лицензии.** Всё, что попадает в сборку, лежит в `dependencies`: `svelte`, `@sveltejs/kit`, `bits-ui`,
  `@internationalized/date`, `@lucide/svelte`, `@fontsource-variable/inter`, `@tanstack/svelte-virtual`.
  Инструменты — в `devDependencies`. Для дерева `dependencies` разрешены MIT, MIT-0, Apache-2.0, BSD-2-Clause,
  BSD-3-Clause, ISC, 0BSD, Zlib, Unicode-3.0 и Unicode-DFS-2016. **OFL-1.1 — только для `@fontsource/*` и
  `@fontsource-variable/*`.** Проверяет `npm run licenses`.
- **Где что тестируется.**
  - Чистая логика и простые компоненты — Vitest с jsdom.
  - Всё, что зависит от bits-ui (всплывающие слои, фокус, клавиатура в списках), — Playwright на страницах
    витрины, в Chromium (как WebView2 на Windows) и WebKit (как macOS и Linux).
  - В сквозных тестах — доступные запросы: `getByRole`, `getByLabel`, `getByText`. `data-testid` — только там,
    где роли нет.
- **Демо-данные вымышленные.** Телефоны — `7000…` (показываются «+7 000 …»). Имена — из списков в `fake.ts`,
  ВЕРХНИМ регистром, как в старых базах.
- **Настройки устройства** лежат в `localStorage`. Любое чтение и запись — в `try/catch`. Без доступа к хранилищу всё
  работает со значениями по умолчанию.
- **Коммит** заканчивается строкой `Co-Authored-By: Claude <модель исполнителя> <noreply@anthropic.com>`.

## Review Focus

1. **Хранилище недоступно или испорчено.** Приватный режим, запрет сайта, мусор в `mw.prefs`, `localStorage`
   бросает исключение. Программа открывается со значениями по умолчанию, без ошибки и мигания темы. Встроенный
   скрипт `app.html` и `parsePrefs` разбирают одинаково. Тесты: `prefs.test.ts` (`storage that throws…`) и
   `app-html.test.ts` (задача 2).
2. **Esc и слои.** Открытый Select, Combobox или меню внутри панели заказа закрывается первым Esc, панель — вторым.
   Esc в широкой панели приёма закрывает её, черновик остаётся. Тесты: `layers.test.ts` (задача 6),
   `e2e/layers.spec.ts`: `Esc closes the select first, then the panel` (задача 6).
3. **Однобуквенные клавиши** N, `/`, `?:`
   - не срабатывают, пока фокус в поле ввода, во время набора через IME и при открытом диалоге;
   - работают в русской раскладке: буквы сравниваются по `KeyboardEvent.code`.

   Тесты: `hotkeys.test.ts` (задача 3), `e2e/orders-demo.spec.ts`: `typing n in the filter does not open intake`
   (задача 9).
4. **Ввод денег.**
   - «6 500,5», «6500.50», пробелы любого вида — понимаются;
   - три знака после запятой, минус, `1e3`, буквы, больше 999 999 999,99 — отказ без округления;
   - поле с ошибкой не портит прежнее значение.

   Тесты: `money.test.ts` (задача 3), `MoneyInput.test.ts` (задача 5).
5. **Имена и даты из реальных баз.**
   - ВЕРХНИЙ регистр, дефисы, лишние пробелы, «ООО РОМАШКА», латиница, пустая строка;
   - даты с долями секунды, без секунд, через пробел, несуществующие («2026-02-30»), мусор;
   - мусор показывается как есть, никогда «Invalid Date».

   Тесты: `name.test.ts`, `date.test.ts` (задача 3).

---

## Файловая структура

```
apps/ui/
  package.json, package-lock.json, vite.config.ts, tsconfig.json, eslint.config.js,
  .prettierrc, .prettierignore, playwright.config.ts, .gitignore
  scripts/check-licenses.mjs          проверка лицензий дерева dependencies
  scripts/check-licenses.test.ts
  src/app.html                        встроенный скрипт настроек до отрисовки
  src/app.d.ts
  src/test-setup.ts                   jest-dom для Vitest
  src/lib/design/
    tokens.css  base.css              токены (спека 3.1–3.4) и базовые стили
    prefs.svelte.ts                   настройки устройства (спека 3.6)
    breakpoints.ts                    ширины экрана (спека 3.5)
    status-colors.ts                  оттенки статусов (спека 3.2)
    contrast.ts                       расчёт контраста для тестов
    layers.svelte.ts                  стек слоёв для Esc
    hotkeys.svelte.ts                 горячие клавиши
    scanner.ts                        распознавание сканера
  src/lib/format/ money.ts phone.ts name.ts date.ts plural.ts search.ts index.ts
  src/lib/i18n/ru.ts
  src/lib/ui/        компоненты (плоско) + index.ts
  src/lib/ui/toast/  toast.svelte.ts Toaster.svelte
  src/lib/ui/table/  DataTable.svelte ColumnsMenu.svelte SelectionBar.svelte column-prefs.ts selection.ts types.ts
  src/lib/ui/shell/  AppShell.svelte TopBar.svelte SidebarNav.svelte BottomNav.svelte
  src/lib/demo/fake.ts                вымышленные данные
  src/routes/+layout.ts               ssr = false
  src/routes/+layout.svelte           стили, шрифт, Tooltip.Provider, Toaster, prefs.init()
  src/routes/+page.ts                 / → /design
  src/routes/design/+page.svelte      витрина: заголовок с настройками + разделы
  src/routes/design/sections/         Tokens Basics Fields Layers Navigation Table Media .svelte
  src/routes/design/orders/+page.svelte   демо главного экрана (спека 6.1, 6.2, 6.4, 6.12)
  src/routes/design/intake/+page.svelte   демо приёма (спека 6.5)
  e2e/*.spec.ts                       Playwright
```

---

### Task 1: Каркас `apps/ui`, проверки, CI, лицензии

**Files:**
- Create: всё в `apps/ui/` из списка выше, кроме `src/lib/**` и разделов витрины, плюс `src/routes/design/+page.svelte`
  с заголовком «Дизайн-система» и `e2e/smoke.spec.ts`
- Modify: `.github/workflows/ci.yml`, корневой `.gitignore`

**Interfaces:**
- Produces:
  - **npm-скрипты:**
    - `dev`, `build`, `preview`;
    - `check` (`svelte-kit sync && svelte-check`);
    - `lint` (`prettier --check . && eslint .`), `format`;
    - `test` (`vitest run`);
    - `test:e2e` (`playwright test`);
    - `licenses` (`node scripts/check-licenses.mjs`);
    - `verify` = `check && lint && test && build && licenses`.
  - **Псевдоним `$api`** → `../../crates/mw-api/bindings`.
  - **`export function isAllowed(license: string, packageName: string): boolean`** в `scripts/check-licenses.mjs`.

- [ ] **Step 1: Создать проект официальным скаффолдером**

Run (из корня worktree; точный синтаксис дополнений сверить с `npx sv@1.4.0 create --help` и `npx sv@1.4.0 add --help`):
`npx sv@1.4.0 create apps/ui --template minimal --types ts --add prettier eslint vitest="usages:unit,component" playwright="demo:no" sveltekit-adapter="adapter:static" --install npm`

Expected: проект собран под SvelteKit 3, настройка в `vite.config.ts` (`svelte.config.js` в Kit 3 нет).

Потом довести до решений плана:
- версии из Tech Stack, TypeScript ровно `6.0.3`;
- рантайм-пакеты — в `dependencies`;
- `"engines": { "node": ">=22.17" }`;
- `printWidth: 100` в `.prettierrc`;
- в `tsconfig.json` — `"strict": true, "noUncheckedIndexedAccess": true`.

Если скаффолдер положил демо-файлы или пример теста — удалить.

- [ ] **Step 2: Настроить SPA и псевдоним**

- `vite.config.ts`:
  - `sveltekit({ adapter: adapter({ fallback: 'index.html', pages: 'build', assets: 'build' }), alias: { $api: '../../crates/mw-api/bindings' } })`;
  - в `test` — jsdom, `setupFiles: ['src/test-setup.ts']`, `include: ['src/**/*.test.ts', 'scripts/**/*.test.ts']`;
  - для компонентных тестов — плагин `svelteTesting()` из `@testing-library/svelte/vite`.
- `src/routes/+layout.ts`: `export const ssr = false; export const prerender = false;`.
- `src/routes/+page.ts`: `redirect(307, '/design')` в `load`.
- `src/app.html`: `<html lang="ru">`.

- [ ] **Step 3: Написать падающие тесты**

`scripts/check-licenses.test.ts`:
```ts
expect(isAllowed('MIT', 'svelte')).toBe(true);
expect(isAllowed('(MIT OR Apache-2.0)', 'x')).toBe(true);
expect(isAllowed('MIT AND ISC', 'x')).toBe(true);
expect(isAllowed('MIT AND GPL-3.0', 'x')).toBe(false);
expect(isAllowed('MPL-2.0', 'x')).toBe(false);
expect(isAllowed('OFL-1.1', '@fontsource-variable/inter')).toBe(true);
expect(isAllowed('OFL-1.1', 'some-icons')).toBe(false);
expect(isAllowed('', 'x')).toBe(false);
expect(isAllowed('UNLICENSED', 'x')).toBe(false);
```
`e2e/smoke.spec.ts`: `/` перенаправляет на `/design`, виден заголовок «Дизайн-система» (`getByRole('heading', { level: 1 })`).

`playwright.config.ts`:
- `webServer: { command: 'npm run build && npm run preview -- --port 4173 --strictPort', port: 4173, reuseExistingServer: !process.env.CI }`;
- `projects`: `chromium` (`devices['Desktop Chrome']`) и `webkit` (`devices['Desktop Safari']`);
- `testDir: 'e2e'`.

- [ ] **Step 4: Убедиться, что тест лицензий падает**

Run: `cd apps/ui && npx vitest run scripts` → FAIL: `isAllowed` не определена.

- [ ] **Step 5: Реализовать `scripts/check-licenses.mjs`**

- **`isAllowed`:** убрать скобки. Выражение с `OR` проходит, если проходит любая ветка; ветка с `AND` — если
  разрешены все части. Списки разрешённого — из Global Constraints.
- **Запуск как скрипта** (`import.meta.url === pathToFileURL(process.argv[1]).href`):
  - пути пакетов — из `npm ls --omit=dev --all --parseable`;
  - лицензия — из `package.json` каждого пакета (`license`, у старых — `licenses[].type`);
  - нарушения печатаются как `имя@версия: лицензия`, при их наличии `process.exit(1)`;
  - при успехе — `Лицензии в порядке: N пакетов`.

- [ ] **Step 6: CI и `.gitignore`**

В `.github/workflows/ci.yml` добавить задачи:
- `ui` — матрица ubuntu, macos, windows:
  - `defaults.run.working-directory: apps/ui`;
  - `actions/setup-node@v4` с `node-version: 24`, `cache: npm`, `cache-dependency-path: apps/ui/package-lock.json`;
  - `npm ci`;
  - `npm run verify`.
- `ui-e2e` на ubuntu: то же, затем `npx playwright install --with-deps chromium webkit` и `npm run test:e2e`.

В корневой `.gitignore` добавить `.svelte-kit/`, `apps/ui/build/`, `test-results/`, `playwright-report/`.

- [ ] **Step 7: Проверить**

Run: `cd apps/ui && npm run verify && npx playwright install chromium webkit && npm run test:e2e`.

Expected: всё зелёное. `licenses` печатает «Лицензии в порядке», smoke проходит в обоих браузерах.

- [ ] **Step 8: Commit**

```bash
git add .gitignore .github/workflows/ci.yml apps/ui
git commit -m "feat(ui): SvelteKit 3 SPA skeleton, checks, license gate, CI jobs"
```

---

### Task 2: Токены, тема, шрифт, настройки устройства

**Files:**
- Create:
  - `src/lib/design/tokens.css`, `base.css`, `prefs.svelte.ts`, `breakpoints.ts`, `status-colors.ts`, `contrast.ts`;
  - тесты `tokens.test.ts`, `prefs.test.ts`, `app-html.test.ts`, `no-raw-colors.test.ts`;
  - `src/routes/design/sections/Tokens.svelte`;
  - `e2e/theme.spec.ts`.
- Modify: `src/app.html`, `src/routes/+layout.svelte`, `src/routes/design/+page.svelte`

**Interfaces:**
- Produces:
```ts
// prefs.svelte.ts
export type ThemePref = 'system' | 'light' | 'dark';
export type Density = 'compact' | 'comfortable';
export type UiScale = 0.9375 | 1 | 1.125 | 1.25;
export interface Prefs { theme: ThemePref; scale: UiScale; density: Density }
export const PREFS_KEY = 'mw.prefs';
export const DEFAULT_PREFS: Prefs; // { theme: 'system', scale: 1, density: 'comfortable' }
export function parsePrefs(raw: string | null): Prefs;        // никогда не бросает; неверное поле → по умолчанию
export function applyPrefs(p: Prefs, root: HTMLElement, systemDark: boolean): void; // data-theme=light|dark, data-density, --ui-scale
export const prefs: { readonly value: Prefs; readonly resolvedTheme: 'light' | 'dark';
  set(patch: Partial<Prefs>): void; init(): () => void };     // init: читает хранилище, слушает matchMedia, возвращает отписку
// breakpoints.ts
export type ShellLayout = 'phone' | 'rail' | 'full';
export const PHONE_QUERY = '(max-width: 599px)';
export const RAIL_QUERY = '(min-width: 600px) and (max-width: 1279px)';
export function layoutForWidth(width: number): ShellLayout;   // <600 phone, <1280 rail, иначе full
// status-colors.ts
export const STATUS_HUES = ['gray','red','orange','amber','lime','green','teal','cyan','blue','indigo','violet','pink'] as const;
export type StatusHue = (typeof STATUS_HUES)[number];
export function statusHue(color: string): StatusHue;          // trim + lower; неизвестное → 'gray'
// contrast.ts
export function contrastRatio(a: string, b: string): number;  // '#rrggbb' или 'rgb(r, g, b)' → WCAG 2.x
```
CSS-классы для статусов: `.st-<оттенок>` задаёт `--st-bg`, `--st-fg`, `--st-dot` из `--st-<оттенок>-*`.

- [ ] **Step 1: Написать падающие тесты**

- **`tokens.test.ts`.** `tokens.css` читается через `import css from './tokens.css?raw'`. Блок `:root` разбирается
  в карту светлой темы, блок `[data-theme='dark']` — тёмной.
  - Все пары контраста из спеки 3.1 проходят свои пороги в обеих темах: 4,5 для текста, 1,5 для `--border-strong`.
  - Для каждого из 12 оттенков есть `--st-<h>-bg`, `--st-<h>-fg`, `--st-<h>-dot` в обеих темах, и `fg` на `bg` —
    не меньше 4,5.
  - Значения совпадают со спекой. Проверить хотя бы `--accent: #0b7a6d` (светлая), `--accent: #2bb3a0` (тёмная),
    `--bg: #f4f3f1`, `--focus: #0e8a7b`.
  - `contrastRatio('#ffffff', '#0e8a7b')` в пределах 4,24–4,26: это обоснование `#0b7a6d`.
- **`prefs.test.ts`:**
  - `parsePrefs(null)` → `DEFAULT_PREFS`;
  - `parsePrefs('{bad')` → `DEFAULT_PREFS`;
  - `parsePrefs('{"theme":"dark","scale":3,"density":"x"}')` → `{ theme: 'dark', scale: 1, density: 'comfortable' }`;
  - `applyPrefs({theme:'system',…}, root, true)` → `data-theme="dark"`;
  - `applyPrefs({theme:'light',…}, root, true)` → `light`;
  - `--ui-scale` равен `"1.125"` при `scale: 1.125`;
  - `prefs.set({ theme: 'dark' })` пишет JSON в `localStorage['mw.prefs']`;
  - **storage that throws:** `getItem` и `setItem` бросают → `init()` и `set()` не бросают, `value` — по умолчанию
    и после `set`.
- **`app-html.test.ts`.** Встроенный скрипт из `src/app.html` (между `<script>` и `</script>`, читается через `?raw`)
  запускается в jsdom с `localStorage` и `matchMedia`, для набора строк:
  - `null`;
  - `'{bad'`;
  - `'{"theme":"dark"}'`;
  - `'{"theme":"system"}'` при тёмной системе;
  - `'{"scale":1.25,"density":"compact"}'`.

  Атрибуты `<html>` совпадают с результатом `applyPrefs(parsePrefs(raw), …)`. Если `localStorage` бросает, скрипт
  не бросает.
- **`no-raw-colors.test.ts`.** `import.meta.glob('/src/**/*.{svelte,css}', { query: '?raw', import: 'default', eager: true })`
  без `src/lib/design/tokens.css`. Ни в одном файле нет `/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/` (комментарии
  убрать перед проверкой).
- **`e2e/theme.spec.ts`:**
  - `localStorage` `{"theme":"dark"}` и перезагрузка → `html[data-theme="dark"]`;
  - `{"theme":"system"}` и `page.emulateMedia({ colorScheme: 'dark' })` → `dark`; затем `colorScheme: 'light'` без
    перезагрузки → `light`;
  - у `body` вычисленный `font-family` начинается с `"Inter Variable"`.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/design` → FAIL: модулей нет.

- [ ] **Step 3: Реализовать**

- **`tokens.css`** — все значения спеки 3.1–3.4 дословно: цвета, оттенки статусов, текст, отступы, высоты, радиусы,
  тени, движение, слои. При `prefers-reduced-motion: reduce` длительности равны `0.01ms`.
- **`base.css`:**
  - `html { font-size: calc(16px * var(--ui-scale, 1)); }`;
  - `body`: `--text-md`, `--leading-normal`, шрифт `'Inter Variable', system-ui, …`, цвета `--text` на `--bg`;
  - `:focus-visible { outline: 2px solid var(--focus); outline-offset: 2px; }`;
  - `::selection` — `--accent-soft`;
  - тонкие полосы прокрутки из токенов: `scrollbar-width`, `scrollbar-color` и `::-webkit-scrollbar`;
  - класс `.num { font-variant-numeric: tabular-nums; }`;
  - класс `.label-caps` — подписи групп по спеке 3.3.
- **`app.html`:** встроенный скрипт без импортов, логика `parsePrefs` и `applyPrefs`, весь в `try/catch`.
- **`+layout.svelte`:** импорт `@fontsource-variable/inter`, `tokens.css`, `base.css`; `$effect(() => prefs.init())`.
- **`Tokens.svelte`:**
  - образцы всех цветов обеих тем с именем токена и контрастом к `--surface`; значения и контраст берутся во время работы из `getComputedStyle`, литералов цвета в разделе нет;
  - шкала текста;
  - метки всех 12 оттенков.

  Подключить в `design/+page.svelte` разделом с `id="tokens"` и заголовком «Токены».

- [ ] **Step 4: Убедиться, что тесты проходят**

Run: `npm run verify && npm run test:e2e` → PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui && git commit -m "feat(ui): design tokens, Inter font, per-device prefs applied before paint"
```

---

### Task 3: Форматирование и служебная логика

**Files:**
- Create:
  - `src/lib/format/{money,phone,name,date,plural,search,index}.ts` и тесты к каждому;
  - `src/lib/design/hotkeys.svelte.ts`, `scanner.ts`, `layers.svelte.ts` с тестами;
  - `src/lib/i18n/ru.ts`.

**Interfaces:**
- Produces:
```ts
// money.ts
export const MAX_MONEY_KOPECKS = 99_999_999_999;   // 999 999 999,99
export function formatMoney(kopecks: number, opts?: { currency?: string | null }): string; // currency по умолчанию '₽'
export function parseMoney(input: string): number | null; // копейки или null; отрицательное, >2 знаков, > MAX → null
// phone.ts
export function phoneDigits(input: string): string;
export function formatPhone(digits: string): string;
// name.ts
export function displayName(stored: string): string;
export function shortName(stored: string): string;
export function initials(stored: string): string;   // до двух букв; пусто → '?'
// date.ts
export interface LocalDateTime { year: number; month: number; day: number; hour: number; minute: number; second: number }
export function parseLocalDateTime(value: string): LocalDateTime | null;
export function nowLocal(): LocalDateTime;           // по часам устройства
export function formatRelative(value: string, now: LocalDateTime): string;
export function formatFull(value: string): string;   // «12 ноября 2024, 09:51»
export function formatDate(value: string): string;   // «12.11.2024»
// plural.ts
export type PluralForms = readonly [one: string, few: string, many: string];
export function plural(n: number, forms: PluralForms): string;
export function countLabel(n: number, forms: PluralForms): string; // «1 234 заказа»
// search.ts — зеркало mw_domain::search::normalize_search
export function normalizeSearch(input: string): string;
export function matchesQuery(text: string, query: string): boolean; // все слова запроса входят в normalizeSearch(text)
// hotkeys.svelte.ts
export function isMac(): boolean;
export function formatShortcut(spec: string, opts?: { mac?: boolean }): string;
export function matchShortcut(e: KeyboardEvent, spec: string, opts?: { mac?: boolean }): boolean;
export function isEditableTarget(target: EventTarget | null): boolean;
export function registerHotkey(spec: string, handler: (e: KeyboardEvent) => void,
  opts?: { allowInFields?: boolean; allowWithOverlay?: boolean }): () => void;
// scanner.ts
export class ScanDetector { constructor(opts?: { minLength?: number; maxIntervalMs?: number }); // 6 и 35
  feed(key: string, timeMs: number): string | null }
export type ScanResult = { kind: 'barcode'; value: string } | { kind: 'order'; id: number } | { kind: 'text'; value: string };
export function classifyScan(code: string): ScanResult;
export function installScanner(onscan: (r: ScanResult) => void): () => void;
// layers.svelte.ts
export type LayerKind = 'popup' | 'modal' | 'panel';
export const layers: { open(kind: LayerKind, close: () => void): () => void;
  readonly topKind: LayerKind | null; readonly blocking: boolean };
// ru.ts
export const ru: { common: { close: 'Закрыть'; cancel: 'Отмена'; undo: 'Отменить'; retry: 'Повторить'; more: 'Ещё';
  loading: 'Загрузка…'; nothingFound: 'Ничего не найдено'; back: 'Назад' };
  save: { saving: 'Сохраняю…'; saved: 'Сохранено'; failed: 'Не сохранено' } }; // дальше пополняется задачами
```

- [ ] **Step 1: Написать падающие тесты (значения — точные)**

`money.test.ts` (NNBSP = ` `, NBSP = ` `):
```ts
expect(formatMoney(650000)).toBe('6 500 ₽');
expect(formatMoney(650050)).toBe('6 500,50 ₽');
expect(formatMoney(0)).toBe('0 ₽');
expect(formatMoney(12345678900)).toBe('123 456 789 ₽');
expect(formatMoney(-50000)).toBe('−500 ₽');
expect(formatMoney(650000, { currency: null })).toBe('6 500');
expect(parseMoney('6500')).toBe(650000);
expect(parseMoney('6 500,5')).toBe(650050);
expect(parseMoney('6 500.50')).toBe(650050);
expect(parseMoney(' 0 ')).toBe(0);
for (const bad of ['', 'abc', '-100', '−100', '1e3', '10,555', '1,2,3', '999999999999'])
  expect(parseMoney(bad)).toBeNull();
expect(parseMoney('999999999,99')).toBe(99_999_999_999);
```
`phone.test.ts`:
```ts
expect(formatPhone('89211234567')).toBe('+7 921 123-45-67');
expect(formatPhone('79211234567')).toBe('+7 921 123-45-67');
expect(formatPhone('9211234567')).toBe('+7 921 123-45-67');
expect(formatPhone('123456')).toBe('12-34-56');
expect(formatPhone('12345')).toBe('12345');
expect(formatPhone('4951234567890')).toBe('4951234567890');
expect(formatPhone('')).toBe('');
expect(phoneDigits('+7 (921) 123-45-67')).toBe('79211234567');
```
`name.test.ts`:
```ts
expect(displayName('ИВАНОВ ИВАН')).toBe('Иванов Иван');
expect(displayName('  ПЕТРОВ   ВИКТОР ')).toBe('Петров Виктор');
expect(displayName('САЛТЫКОВ-ЩЕДРИН МИХАИЛ')).toBe('Салтыков-Щедрин Михаил');
expect(displayName('Иванов иван')).toBe('Иванов иван');       // есть строчные — как есть
expect(displayName('ООО РОМАШКА')).toBe('ООО Ромашка');       // ООО ОАО ЗАО ПАО АО ИП ЧП ТОО МУП ГУП ФГУП НКО ТЦ СЦ
expect(displayName('IVANOV IVAN')).toBe('Ivanov Ivan');
expect(displayName('')).toBe('');
expect(shortName('ИВАНОВ ИВАН ИВАНОВИЧ')).toBe('Иванов И. И.');
expect(shortName('ИВАНОВ')).toBe('Иванов');
expect(shortName('ООО РОМАШКА')).toBe('ООО Ромашка');
expect(initials('ИВАНОВ ИВАН')).toBe('ИИ');
expect(initials('')).toBe('?');
```
`date.test.ts` (`now = { year: 2026, month: 11, day: 15, hour: 12, minute: 0, second: 0 }`):
```ts
expect(parseLocalDateTime('2026-11-15T10:24:00')).toEqual({ year: 2026, month: 11, day: 15, hour: 10, minute: 24, second: 0 });
expect(parseLocalDateTime('2026-11-15T10:24:00.123')?.minute).toBe(24);
expect(parseLocalDateTime('2026-11-15T10:24')).not.toBeNull();
expect(parseLocalDateTime('2026-11-15 10:24:00')).not.toBeNull();
expect(parseLocalDateTime('2026-11-15')).toEqual({ year: 2026, month: 11, day: 15, hour: 0, minute: 0, second: 0 });
expect(parseLocalDateTime('2026-02-30T10:00:00')).toBeNull();
expect(parseLocalDateTime('мусор')).toBeNull();
expect(formatRelative('2026-11-15T10:24:00', now)).toBe('сегодня 10:24');
expect(formatRelative('2026-11-14T17:02:00', now)).toBe('вчера 17:02');
expect(formatRelative('2026-11-12T09:51:00', now)).toBe('12 ноя 09:51');
expect(formatRelative('2026-05-03T09:05:00', now)).toBe('3 мая 09:05');
expect(formatRelative('2026-11-16T09:00:00', now)).toBe('16 ноя 09:00');
expect(formatRelative('2024-11-12T09:51:00', now)).toBe('12.11.2024');
expect(formatRelative('мусор', now)).toBe('мусор');
expect(formatRelative('2025-12-31T23:00:00', { year: 2026, month: 1, day: 1, hour: 8, minute: 0, second: 0 })).toBe('вчера 23:00');
expect(formatFull('2024-11-12T09:51:00')).toBe('12 ноября 2024, 09:51');
expect(formatDate('2024-11-12T09:51:00')).toBe('12.11.2024');
```
Короткие месяцы: `янв фев мар апр мая июн июл авг сен окт ноя дек`. Полные в родительном падеже: `января … декабря`.

`plural.test.ts` (`F = ['заказ', 'заказа', 'заказов']`):
- `plural(n, F)`:
  - 1 → `заказ`, 2 → `заказа`, 5 → `заказов`;
  - 11 → `заказов`, 21 → `заказ`, 22 → `заказа`;
  - 0 → `заказов`, 1.5 → `заказа`.
- `countLabel(1234, F)` → `'1 234 заказа'`.

`search.test.ts`:
- `normalizeSearch('  ЁЛКИН\tПётр ')` → `'елкин петр'`;
- `normalizeSearch('ё')` → `'е'` (NFC, потом ё → е);
- `matchesQuery('ИВАНОВ ИВАН', 'иван ив')` → `true`;
- `matchesQuery('ИВАНОВ', 'петр')` → `false`.

`hotkeys.test.ts`:
- **`formatShortcut`:**
  - `'Mod+K'` → `'Ctrl K'` и `'⌘K'` (`mac: true`);
  - `'Mod+Enter'` → `'Ctrl Enter'` и `'⌘↩'`;
  - `'Alt+ArrowUp'` → `'Alt ↑'` и `'⌥↑'`;
  - `'Escape'` → `'Esc'`; `'N'` → `'N'`.
- **`matchShortcut`:**
  - `'N'` совпадает с `{ key: 'т', code: 'KeyN' }` (русская раскладка);
  - не совпадает с `{ key: 'n', code: 'KeyN', ctrlKey: true }`;
  - `'Mod+K'` совпадает с `{ code: 'KeyK', ctrlKey: true }` при `mac: false` и с `{ code: 'KeyK', metaKey: true }` при `mac: true`;
  - `'/'` совпадает с `{ key: '/', code: 'Slash' }` и `{ key: '.', code: 'Slash' }`;
  - `'?'` совпадает с `{ key: '?', code: 'Digit7', shiftKey: true }`.
- **`isEditableTarget`:**
  - `true` для `input[type=text]`, `textarea`, `[contenteditable=true]`;
  - `false` для `input[type=checkbox]` и `button`.
- **`registerHotkey`:**
  - не вызывает обработчик для события из `input` и при `isComposing: true`;
  - не вызывает при `layers.blocking`, если не передан `allowWithOverlay`;
  - отписка снимает обработчик.

`scanner.test.ts`:
- 12 цифр с интервалом 10 мс и `Enter` → `'201234567893'`;
- те же цифры с интервалом 100 мс → `null`;
- 5 цифр и `Enter` → `null`;
- пауза 200 мс в середине — набор начинается заново;
- `Shift` внутри набора игнорируется;
- `classifyScan`:
  - `'201234567893'` → `barcode`;
  - `'10481'` → `{ kind: 'order', id: 10481 }`;
  - `'0'` и `'ABC-123'` → `text`.

`layers.test.ts`:
- `open('panel', closeA)` и `open('popup', closeB)`:
  - `topKind === 'popup'`, `blocking === true`;
  - `Escape` на `window` не вызывает `closeA`.
- После отписки `popup`: `topKind === 'panel'`, `blocking === false`, `Escape` вызывает `closeA` один раз.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/format src/lib/design` → FAIL.

- [ ] **Step 3: Реализовать**

- **`parseMoney`:** убрать пробелы всех видов (`\s`, U+00A0, U+202F). Шаблон `^\d+([.,]\d{1,2})?$`, копейки
  целым числом, без `parseFloat`.
- **`formatMoney`:** разряды через U+202F, минус — U+2212.
- **Даты:** разбор регулярным выражением и проверка дня по длине месяца. «Вчера» считать по календарным датам, а
  не по 24 часам.
- **`plural`** — через `Intl.PluralRules('ru')`: `one` → 0, `few` → 1, `many` → 2, `other` → 1.
- **`matchShortcut`:**
  - буквы и цифры — по `code` (`KeyN`, `Digit7`);
  - `/` — `key === '/'` или `code === 'Slash'` без модификаторов;
  - `?` — `key === '?'`;
  - `Mod` — `metaKey` на macOS, иначе `ctrlKey`;
  - лишний модификатор — не совпадение.
- **`registerHotkey`:** один слушатель `keydown` на `window` в фазе всплытия и карта обработчиков; при совпадении
  `preventDefault`.
- **`installScanner`:** слушает `keydown` в фазе захвата, пропускает события из полей ввода, кормит `ScanDetector`.
- **`layers`:** стек записей `{ kind, close }` и один слушатель `Escape`. Он закрывает верхний слой, только если
  это `panel`: `popup` и `modal` закрывает сам bits-ui.

- [ ] **Step 4: Убедиться, что тесты проходят**

Run: `npm run verify` → PASS.

- [ ] **Step 5: Commit**

```bash
git add apps/ui && git commit -m "feat(ui): money, phone, name, date and plural formatting, search normalization, hotkeys, scanner, layer stack"
```

---

### Task 4: Базовые элементы и блоки

**Files:**
- Create:
  - в `src/lib/ui/`: `Button`, `IconButton`, `Kbd`, `Spinner`, `Skeleton`, `Divider`, `VisuallyHidden`, `Tooltip`,
    `Badge`, `StatusBadge`, `Avatar`, `FlagIcons`, `Card`, `Section`, `EmptyState`, `Callout`, `KeyValue`, `Timeline`
    (`.svelte`);
  - `index.ts`;
  - тесты `Button.test.ts`, `StatusBadge.test.ts`, `Avatar.test.ts`, `Callout.test.ts`, `Timeline.test.ts`;
  - `src/routes/design/sections/Basics.svelte`.
- Modify:
  - `src/routes/+layout.svelte` — `Tooltip.Provider` из bits-ui с `delayDuration={400}`;
  - `design/+page.svelte`;
  - `ru.ts` — строки отметок: «Позвонить», «Заказать запчасть», «Чёрный список».

**Interfaces:**
- Consumes: `formatShortcut` (задача 3), `statusHue` (задача 2), `displayName`, `initials`, `formatRelative`,
  `LocalDateTime` (задача 3).
- Produces (пропсы; `Snippet` и `Component` из `svelte`):
```ts
Button: { variant?: 'primary'|'secondary'|'ghost'|'danger' /*secondary*/; size?: 'sm'|'md'|'lg' /*md*/;
  type?: 'button'|'submit'; busy?: boolean; disabled?: boolean; icon?: Component; shortcut?: string; href?: string;
  onclick?: (e: MouseEvent) => void; children?: Snippet; class?: string }
IconButton: { label: string; icon: Component; size?: 'sm'|'md'; variant?: 'ghost'|'secondary'; disabled?: boolean;
  pressed?: boolean; onclick?: (e: MouseEvent) => void }       // aria-label + Tooltip с label
Kbd: { shortcut: string }
Tooltip: { text: string; side?: 'top'|'bottom'|'left'|'right'; children: Snippet }
Spinner: { size?: 'sm'|'md'; label?: string /*'Загрузка…'*/ }
Skeleton: { width?: string; height?: string; radius?: 'sm'|'md'|'full' }
Divider: { vertical?: boolean }
VisuallyHidden: { children: Snippet }
Badge: { tone?: 'neutral'|'accent'|'danger'|'warning'|'success'|'info'; children: Snippet }
StatusBadge: { name: string; color: string; size?: 'sm'|'md'; dot?: boolean /*true*/ }
Avatar: { name: string; size?: 'sm'|'md' }
FlagIcons: { needCall: boolean; needParts: boolean; blacklisted?: boolean }
Card: { padding?: 'none'|'sm'|'md'; children: Snippet }
Section: { title: string; actions?: Snippet; children: Snippet }
EmptyState: { icon?: Component; title: string; description?: string; actions?: Snippet }
Callout: { tone: 'info'|'warning'|'danger'|'success'; title?: string; children: Snippet; actions?: Snippet }
KeyValue: { items: { label: string; value: string; muted?: boolean }[] }
Timeline: { items: { id: string | number; at: string; who?: string; text: string }[]; now: LocalDateTime; emptyText?: string }
```

- [ ] **Step 1: Написать падающие тесты** (Vitest + Testing Library)

- **`Button`:**
  - `busy` → `aria-busy="true"`, клик не вызывает `onclick`, внутри `Spinner`;
  - `disabled` → `disabled`;
  - `shortcut="N"` рисует `<kbd>`;
  - `href` → ссылка `<a>`.
- **`IconButton`** имеет `aria-label`, равный `label`.
- **`StatusBadge`:**
  - `color="violet"` → класс `st-violet`, текст «Согласовано»;
  - `color="#ff0000"` → `st-gray`.
- **`Avatar`** «ИВАНОВ ИВАН» → текст «ИИ».
- **`Callout`:** `tone="danger"` → `role="alert"`, `tone="info"` → `role="status"`.
- **`Timeline`:**
  - элемент `at="2026-11-15T10:30:00"` при `now` 2026-11-15 12:00 показывает «сегодня 10:30»;
  - пустой список — `emptyText`.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/ui` → FAIL.

- [ ] **Step 3: Реализовать компоненты и `index.ts`**

- **Вид — только токены.**
  - Кнопки высотой `--control-*`.
  - `primary` — `--accent` / `--on-accent`, `danger` — `--danger` / `--on-danger`.
  - `secondary` — `--surface` с рамкой `--border-strong`, `ghost` — прозрачная, при наведении `--surface-3`.
- **`busy`** сохраняет ширину кнопки.
- **На сенсорных экранах** (`@media (pointer: coarse)`) высота кнопок и `IconButton` — не меньше 2.75rem.
- **`FlagIcons`:** иконки `phone` и `package` из Lucide, подписи — через `VisuallyHidden` и `Tooltip`.
  Чёрный список — точка `--danger`.
- **`StatusBadge`:** класс `st-${statusHue(color)}`, фон `--st-bg`, текст `--st-fg`, точка `--st-dot`.

- [ ] **Step 4: Раздел витрины «Основа»**

`Basics.svelte` с `id="basics"`. В нём:
- все варианты и размеры кнопок, `busy`, `disabled`;
- `IconButton` с подсказкой;
- метки, все статусы по умолчанию (стартовые 8 из `mw-domain`);
- аватар;
- отметки;
- карточка;
- раздел;
- пустое состояние;
- четыре `Callout`;
- `KeyValue`;
- `Timeline`.

- [ ] **Step 5: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): buttons, badges, status badge, avatar, flags, blocks, timeline"
```

---

### Task 5: Поля ввода

**Files:**
- Create:
  - в `src/lib/ui/`: `Field`, `TextInput`, `Textarea`, `MoneyInput`, `PhoneInput`, `Checkbox`, `Switch`,
    `SegmentedControl`, `ChipGroup`, `Select`, `Combobox`, `DateInput` (`.svelte`);
  - тесты `Field.test.ts`, `MoneyInput.test.ts`, `PhoneInput.test.ts`, `ChipGroup.test.ts`, `Switch.test.ts`;
  - `src/routes/design/sections/Fields.svelte`;
  - `e2e/fields.spec.ts`.
- Modify:
  - `design/+page.svelte` — в шапке витрины три `SegmentedControl`: тема «Как в системе / Светлая / Тёмная»,
    размер текста «Мельче / Обычный / Крупнее / Крупный», плотность «Компактно / Обычно»;
  - `index.ts`;
  - `ru.ts`.

**Interfaces:**
- Consumes: `parseMoney`, `formatMoney`, `phoneDigits`, `formatPhone`, `normalizeSearch` (задача 3); `prefs`
  (задача 2); `layers` (задача 3).
- Produces:
```ts
// общие пропсы полей: label?: string; hint?: string; error?: string; required?: boolean; disabled?: boolean; id?: string
Field: { label?: string; hint?: string; error?: string; required?: boolean; id: string;
  children: Snippet<[{ id: string; describedBy: string | undefined; invalid: boolean }]> }
TextInput: { value: string /*$bindable*/; type?: 'text'|'search'|'password'|'tel'|'email'; placeholder?: string;
  revealable?: boolean; prefix?: Snippet; suffix?: Snippet; size?: 'sm'|'md'; readonly?: boolean; autofocus?: boolean;
  inputmode?: string; maxlength?: number; name?: string; oninput?: (v: string) => void; onblur?: () => void; …общие }
Textarea: { value: string /*$bindable*/; minRows?: number /*2*/; maxRows?: number /*12*/; placeholder?: string; …общие }
MoneyInput: { value: number | null /*$bindable, копейки*/; nullable?: boolean /*true*/; currency?: string /*'₽'*/; placeholder?: string; …общие }
PhoneInput: { value: string /*$bindable, цифры*/; placeholder?: string; …общие }
Checkbox: { checked: boolean /*$bindable*/; label: string; hint?: string; disabled?: boolean; indeterminate?: boolean }
Switch: { checked: boolean /*$bindable*/; label: string; hint?: string; disabled?: boolean }
SegmentedControl<T extends string | number>: { value: T /*$bindable*/; options: { value: T; label: string; icon?: Component }[];
  label: string; size?: 'sm'|'md' }
ChipGroup: { selected: string[] /*$bindable*/; options: string[]; label: string; allowCustom?: boolean;
  customLabel?: string /*'+ своё'*/; oncreate?: (value: string) => void }
Select<T extends string | number>: { value: T | null /*$bindable*/; items: { value: T; label: string; disabled?: boolean }[];
  placeholder?: string /*'Выберите'*/; …общие }
Combobox: { value: string /*$bindable*/; items: { value: string; hint?: string }[]; allowCustom?: boolean /*true*/;
  placeholder?: string; emptyText?: string /*'Ничего не найдено'*/; onselect?: (v: string) => void; …общие }
DateInput: { value: string | null /*$bindable, 'YYYY-MM-DD'*/; min?: string; max?: string; …общие }
```

- [ ] **Step 1: Написать падающие тесты**

Vitest:
- **`Field`:** у поля `aria-describedby` указывает на подсказку и ошибку; при `error` — `aria-invalid="true"`;
  при `required` к подписи добавлен «*».
- **`MoneyInput`:**
  - `value=650000` без фокуса показывает «6 500» (с U+202F) и суффикс «₽»;
  - фокус → «6500»;
  - ввод «7 000,5» и уход из поля → `value === 700050`;
  - ввод «abc» и уход → `value` прежний, видна ошибка «Введите сумму, например 6 500»;
  - `nullable` и пустой ввод → `null`; без `nullable` пустой ввод → `0`.
- **`PhoneInput`:** ввод «+7 (921) 123-45-67» → `value === '79211234567'`, без фокуса показывает
  «+7 921 123-45-67».
- **`ChipGroup`:**
  - клик по «Чехол» добавляет его в `selected` (`aria-pressed="true"`), второй клик убирает;
  - «+ своё» → поле, ввод « чехол » и Enter → дубль не создаётся, «Чехол» выбран, `oncreate` не вызван;
  - ввод «Стилус» → добавлен в варианты и выбран, `oncreate('Стилус')`.
- **`Switch`:** `role="switch"`, `aria-checked` меняется по клику.

`e2e/fields.spec.ts` (на витрине):
- **`Combobox` «Бренд»** (варианты `Samsung`, `Apple`, `Xiaomi`, `Ёмкость-тест`):
  - ввод «sam» показывает только Samsung, Enter выбирает;
  - ввод «емк» находит «Ёмкость-тест»;
  - ввод «Nokia» и Tab оставляет «Nokia» (`allowCustom`).
- **`Select`:** открывается кликом, выбирается стрелками и Enter, Esc закрывает без смены.
- **`SegmentedControl`:** стрелки меняют выбор.
- **Тема:** клик «Тёмная» в шапке витрины → `html[data-theme="dark"]`, после перезагрузки тема сохраняется.
- **`DateInput`:** календарь открывается и выбор дня ставит дату в поле `дд.мм.гггг`.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/ui && npx playwright test e2e/fields.spec.ts` → FAIL.

- [ ] **Step 3: Реализовать**

- **`Field`:** id подсказки и ошибки — `${id}-hint` и `${id}-error`. Если `id` не передан, поля создают его через
  `$props.id()`.
- **`Textarea`:** авторазмер по `scrollHeight` между `minRows` и `maxRows`.
- **`MoneyInput`:** `inputmode="decimal"`; разбор — только при уходе из поля и Enter. *Изменено после финальной
  проверки:* сумма берётся при вводе, как только текст стал суммой; ошибка — при уходе из поля и Enter; значение,
  изменённое снаружи, заменяет текст; без ввода ничего не записывается; `invalid` останавливает отправку формы.
- **`Checkbox`, `Switch`, `SegmentedControl`:** нативные `input` (checkbox, `role="switch"`, radio) со своим видом.
- **`ChipGroup`:** кнопки с `aria-pressed`. Дубль ищется через `normalizeSearch`.
- **`Select`, `Combobox`, `DateInput`:** обёртки bits-ui.
  - `Combobox`: `shouldFilter` выключен, фильтр свой — `normalizeSearch` и вхождение, не больше 50 вариантов,
    подсказка варианта справа приглушённо.
  - `DateInput`: DatePicker, `locale="ru"`, преобразование `CalendarDate` ↔ `'YYYY-MM-DD'` через
    `@internationalized/date`.
  - Каждая обёртка регистрирует открытое состояние в `layers` как `popup`.
- **На сенсорных экранах:** шрифт поля 16px, высота не меньше 2.75rem (`@media (pointer: coarse)`).

- [ ] **Step 4: Раздел витрины «Поля»**

`Fields.svelte` с `id="fields"`: каждое поле в обычном виде, с подсказкой, с ошибкой, отключённым, в `size="sm"`.
Плюс примеры приёма: бренд (`Combobox`), комплектность (`ChipGroup`), предоплата (`MoneyInput`), телефон
(`PhoneInput`), дата (`DateInput`).

- [ ] **Step 5: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): form fields: text, money, phone, chips, select, combobox, date, switches"
```

---

### Task 6: Слои и уведомления

**Files:**
- Create:
  - в `src/lib/ui/`: `Popover`, `DropdownMenu`, `Dialog`, `ConfirmDialog`, `SlideOver`, `Sheet` (`.svelte`);
  - `src/lib/ui/toast/toast.svelte.ts`, `src/lib/ui/toast/Toaster.svelte`, `src/lib/ui/toast/toast.test.ts`;
  - `src/routes/design/sections/Layers.svelte`;
  - `e2e/layers.spec.ts`.
- Modify: `+layout.svelte` (`<Toaster />`), `index.ts`, `ru.ts`

**Interfaces:**
- Consumes: `layers`, `registerHotkey`, `isEditableTarget` (задача 3); `Button`, `IconButton` (задача 4);
  `PHONE_QUERY` (задача 2).
- Produces:
```ts
export type MenuItem =
  | { label: string; icon?: Component; shortcut?: string; onselect: () => void; disabled?: boolean; danger?: boolean }
  | { separator: true };
Popover: { open?: boolean /*$bindable*/; trigger: Snippet<[Record<string, unknown>]>; children: Snippet;
  side?: 'top'|'bottom'|'left'|'right'; align?: 'start'|'center'|'end'; width?: string }
DropdownMenu: { trigger: Snippet<[Record<string, unknown>]>; items: MenuItem[]; label?: string }
Dialog: { open: boolean /*$bindable*/; title: string; description?: string; size?: 'sm'|'md'|'lg' /*24/30/40rem*/;
  children: Snippet; footer?: Snippet; onclose?: () => void }
ConfirmDialog: { open: boolean /*$bindable*/; title: string; description: string; confirmLabel: string;
  tone?: 'danger'|'primary' /*danger*/; onconfirm: () => void | Promise<void> }
SlideOver: { open: boolean /*$bindable*/; title: string; width?: string /*'min(30rem, 100vw - 3rem)'*/;
  header?: Snippet; children: Snippet; footer?: Snippet; onclose?: () => void;
  onprev?: () => void; onnext?: () => void; returnFocusTo?: HTMLElement | null }
Sheet: { open: boolean /*$bindable*/; title: string; width?: string /*'min(70rem, 100vw - 4rem)'*/;
  header?: Snippet; children: Snippet; footer?: Snippet; onclose?: () => void }
// toast.svelte.ts
export interface ToastOptions { message: string; tone?: 'neutral'|'success'|'danger';
  action?: { label: string; run: () => void | Promise<void> }; duration?: number /*6000*/ }
export const toast: { readonly items: readonly (ToastOptions & { id: string })[];
  show(o: ToastOptions): string; dismiss(id: string): void;
  undoable(message: string, undo: () => void | Promise<void>): string;  // action.label = 'Отменить'
  undoLast(): boolean };                                                 // запускает action последнего с действием
```

- [ ] **Step 1: Написать падающие тесты**

`toast.test.ts` (фальшивые таймеры):
- четвёртое уведомление убирает самое старое, видно не больше 3;
- через 6000 мс уведомление исчезает;
- `undoLast()` запускает `undo` последнего `undoable`, убирает его и возвращает `true`; без таких — `false`.

`e2e/layers.spec.ts` (раздел «Слои» витрины):
- **`SlideOver`:**
  - кнопка «Открыть панель» открывает панель с заголовком; страница под ней кликабельна (кнопка-счётчик за панелью
    увеличивает число);
  - Esc закрывает, фокус возвращается на кнопку открытия.
- **Esc closes the select first, then the panel:** внутри панели открыть `Select`, первый Esc закрывает только его,
  второй — панель.
- **`SlideOver` с `onprev` и `onnext`:**
  - кнопки ‹ › и ↑↓ при фокусе на панели (не в поле) меняют номер записи на демо;
  - ↓ при фокусе в поле ввода не меняет;
  - Alt+↓ меняет.
- **`Sheet`:**
  - затемнение;
  - Tab с последнего элемента переходит на первый (фокус внутри);
  - Esc закрывает;
  - Ctrl/⌘+Enter в демо-форме вызывает «Принять».
- **`ConfirmDialog`:** кнопка подтверждения занята, пока `onconfirm` ждёт 300 мс, потом диалог закрыт.
- **Уведомление «Статус → Готов»:**
  - кнопка «Отменить» возвращает прежний статус на демо;
  - повтор, затем Ctrl/⌘+Z вне поля тоже возвращает.
- **Ширина 390×844:** `SlideOver` и `Dialog` во весь экран, у панели кнопка «Назад».

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/ui/toast && npx playwright test e2e/layers.spec.ts` → FAIL.

- [ ] **Step 3: Реализовать**

- **`Popover`, `DropdownMenu`, `Dialog`, `ConfirmDialog`, `Sheet`** — обёртки bits-ui, каждая регистрирует себя в
  `layers`.
  - `Popover` и `DropdownMenu` — как `popup`.
  - `Dialog`, `ConfirmDialog` и `Sheet` — как `modal`.
  - `Sheet` — `Dialog` bits-ui, прижатый к правому краю на всю высоту, с `--scrim` и `--shadow-lg`.
- **`SlideOver`** — своя немодальная панель.
  - `role="dialog"`, `aria-labelledby` на заголовок, без затемнения, `--z-panel`.
  - Регистрируется в `layers` как `panel`, Esc закрывает её через `layers`.
  - При открытии фокус уходит на контейнер панели (`tabindex="-1"`).
  - При закрытии фокус возвращается на `returnFocusTo` или на прежний активный элемент.
  - ↑↓ работают, когда фокус в панели и не в поле. Alt+↑/↓ работают всегда, пока панель открыта
    (`registerHotkey` с `allowInFields`).
  - Появление — `transform` за `--duration-slow`.
  - На телефоне (`PHONE_QUERY`) панель во весь экран, кнопка «Назад» вместо «Закрыть».
- **`Toaster`:**
  - область `role="status"`, `aria-live="polite"`;
  - внизу слева, на телефоне — внизу по центру над нижней навигацией;
  - таймер стоит на паузе при наведении и фокусе;
  - `registerHotkey('Mod+Z', () => toast.undoLast())` — не в полях.

- [ ] **Step 4: Раздел витрины «Слои»**

`Layers.svelte` с `id="layers"`: демо каждого слоя, на которые опираются тесты. В том числе панель записи
«№ 1…5» с ‹ ›, `Select` внутри панели, кнопка-счётчик, уведомление со статусом.

- [ ] **Step 5: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): popover, menu, dialogs, slide-over panel, sheet, undoable toasts"
```

---

### Task 7: Каркас приложения, навигация, Ctrl K

**Files:**
- Create:
  - `src/lib/ui/shell/{AppShell,TopBar,SidebarNav,BottomNav}.svelte`;
  - в `src/lib/ui/`: `Tabs`, `FilterChips`, `CommandPalette`, `ShortcutsHelp` (`.svelte`), `commands.ts` с тестом;
  - `src/routes/design/sections/Navigation.svelte`;
  - `e2e/navigation.spec.ts`.
- Modify: `index.ts`, `ru.ts`

**Interfaces:**
- Consumes: `layoutForWidth`, `PHONE_QUERY`, `RAIL_QUERY` (задача 2); `Tooltip`, `IconButton` (задача 4);
  `layers`, `formatShortcut`, `normalizeSearch` (задача 3); `Dialog` (задача 6).
- Produces:
```ts
export interface NavItem { id: string; label: string; icon: Component; href?: string; count?: number; attention?: boolean }
export interface NavSection { title?: string; items: NavItem[] }
AppShell: { topbar: Snippet; sidebar: Snippet; bottomNav?: Snippet; children: Snippet }
// AppShell кладёт в контекст getShellLayout(): () => ShellLayout (ключ — экспортируемая функция)
export function getShellLayout(): () => ShellLayout;
TopBar: { title: string; search?: Snippet; actions?: Snippet }
SidebarNav: { sections: NavSection[]; active: string; onnavigate?: (id: string) => void; footer?: Snippet }
BottomNav: { items: NavItem[]; active: string; primary?: { label: string; icon: Component; onclick: () => void };
  onnavigate?: (id: string) => void }
Tabs: { value: string /*$bindable*/; items: { value: string; label: string; count?: number }[]; content: Snippet<[string]> }
FilterChips: { items: { id: string; label: string; count?: number; color?: string }[]; selected: string[] /*$bindable*/;
  label: string }                                   // клик — только этот; Ctrl/⌘+клик — добавить или убрать
export interface CommandItem { id: string; label: string; hint?: string; icon?: Component; shortcut?: string;
  keywords?: string[]; status?: { name: string; color: string } }
export interface CommandGroup { id: string; title: string; items: CommandItem[] }
CommandPalette: { open: boolean /*$bindable*/; query: string /*$bindable*/; groups: CommandGroup[];
  placeholder?: string /*'Найти заказ, клиента или команду…'*/; loading?: boolean; emptyText?: string;
  onselect: (item: CommandItem, opts: { alt: boolean }) => void }
ShortcutsHelp: { open: boolean /*$bindable*/; groups: { title: string; items: { label: string; shortcut: string }[] }[] }
// commands.ts
export function filterCommands(items: CommandItem[], query: string): CommandItem[]; // все слова запроса в label+keywords
```

- [ ] **Step 1: Написать падающие тесты**

`commands.test.ts`:
- `filterCommands` по «прин» находит «Принять устройство»;
- по «тем» — «Сменить тему» (ключевое слово `тема`);
- по «ЁЛ» — пункт с `keywords: ['ёлка']`;
- пустой запрос возвращает всё.

`e2e/navigation.spec.ts` (раздел «Навигация» витрины, демо-каркас в рамке на всю ширину окна):
- **Ширины:**
  - 1440 → боковое меню с подписями;
  - 1024 → полоса иконок, подпись пункта в подсказке при наведении;
  - 390 → нижняя навигация, бокового меню нет.
- **`FilterChips`:**
  - клик «Готов» выбирает только его (`aria-pressed`);
  - Ctrl/⌘+клик «Диагностика» добавляет второй.
- **`CommandPalette`:**
  - Ctrl/⌘+K открывает;
  - ввод «прин» оставляет «Принять устройство»;
  - Enter вызывает `onselect`, демо пишет «Выбрано: Принять устройство»;
  - Esc закрывает.
- **`ShortcutsHelp`:** «?» открывает справку; «?» в поле ввода — не открывает.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/ui/commands.test.ts && npx playwright test e2e/navigation.spec.ts` → FAIL.

- [ ] **Step 3: Реализовать**

- **`AppShell`** — CSS grid; раскладка из `MediaQuery` (`svelte/reactivity`) с `PHONE_QUERY` и `RAIL_QUERY`.
  - Верхняя панель высотой 48px, `--z-shell`.
  - Боковое меню 232px или 56px.
  - На телефоне — нижняя навигация, закреплённая снизу, с отступом `env(safe-area-inset-bottom)`.
- **`SidebarNav`:**
  - выбранный пункт — `--accent-soft` / `--accent-text`;
  - заголовки секций — `.label-caps`;
  - счётчик — `.num`, приглушённый, у `attention` — `--warning-text`;
  - в полосе иконок — `Tooltip` с подписью и точка вместо счётчика.
- **`FilterChips`** — кнопки с `aria-pressed`; на телефоне прокручиваются вбок.
- **`CommandPalette`** — `Command` bits-ui с `shouldFilter={false}` внутри `Dialog`.
  - Ширина 40rem, отступ сверху 15vh.
  - Ctrl/⌘+Enter → `onselect(item, { alt: true })`.
  - Регистрируется в `layers` как `modal`.
- **`Tabs`** — обёртка bits-ui.

- [ ] **Step 4: Раздел витрины «Навигация»**

`Navigation.svelte` с `id="navigation"`. Демо-каркас: верхняя панель, разделы и «Требуют внимания» по спеке 6.1,
нижняя навигация. Плюс `FilterChips`, `Tabs`, кнопки открытия палитры и справки.

- [ ] **Step 5: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): app shell with responsive navigation, filter chips, tabs, command palette, shortcuts help"
```

---

### Task 8: Таблица, выделение, карточки списка

**Files:**
- Create:
  - `src/lib/ui/table/{types.ts,column-prefs.ts,selection.ts,DataTable.svelte,ColumnsMenu.svelte,SelectionBar.svelte}`;
  - тесты `column-prefs.test.ts`, `selection.test.ts`;
  - `src/lib/ui/ListCard.svelte`, `src/lib/ui/InfiniteList.svelte`;
  - `src/lib/demo/fake.ts` с тестом `fake.test.ts`;
  - `src/routes/design/sections/Table.svelte`;
  - `e2e/table.spec.ts`.
- Modify: `index.ts`, `ru.ts`

**Interfaces:**
- Consumes: `prefs` — плотность (задача 2); `Checkbox` (задача 5); `DropdownMenu`, `Popover` (задача 6);
  `StatusBadge`, `FlagIcons`, `EmptyState`, `Skeleton` (задача 4); форматирование (задача 3);
  `import type { OrderListItem, Status } from '$api/…'`.
- Produces:
```ts
// types.ts
export type RowId = string | number;
export interface Column<Row> { id: string; header: string; width: number; minWidth?: number /*60*/;
  align?: 'start'|'end'|'center'; sortable?: boolean; hiddenByDefault?: boolean; cell: Snippet<[Row]> }
export interface SortState { columnId: string; descending: boolean }
export interface ColumnPrefs { order: string[]; hidden: string[]; widths: Record<string, number> }
// column-prefs.ts
export function columnPrefsKey(storageKey: string): string;        // 'mw.table.' + storageKey
export function loadColumnPrefs(storageKey: string, columns: Column<never>[]): ColumnPrefs;
export function saveColumnPrefs(storageKey: string, prefs: ColumnPrefs): void;
// selection.ts
export function rangeSelect(ids: RowId[], anchor: RowId, target: RowId): RowId[];
DataTable<Row>: { rows: Row[]; columns: Column<Row>[]; rowId: (r: Row) => RowId; storageKey: string; ariaLabel: string;
  sort?: SortState | null; onsortchange?: (s: SortState) => void; selectable?: boolean;
  selected?: Set<RowId> /*$bindable*/; activeId?: RowId | null /*$bindable*/; onopen?: (r: Row) => void;
  loading?: boolean; empty?: Snippet; rowClass?: (r: Row) => string | undefined;
  onendreached?: () => void; flashIds?: RowId[] }
ColumnsMenu: { columns: Column<never>[]; prefs: ColumnPrefs /*$bindable*/ }
SelectionBar: { count: number; onclear: () => void; children: Snippet }
ListCard: { number: string; title: string; subtitle?: string; meta?: string; status?: { name: string; color: string };
  flags?: { needCall: boolean; needParts: boolean }; active?: boolean; onclick?: () => void }
InfiniteList<T>: { items: T[]; key: (t: T) => RowId; item: Snippet<[T]>; onendreached?: () => void; loading?: boolean; empty?: Snippet }
// fake.ts
export const DEMO_STATUSES: Status[];   // стартовые 8 (id 1–8, имена, виды и цвета как в mw-domain) + «Куплен», gray, issued (id 9)
export const DEMO_POINTS: { id: number; name: string }[];  // «Центр», «Северная», «Южная»
export function makeOrders(count: number, seed?: number): OrderListItem[]; // детерминированно (mulberry32)
export function makeDictionaries(): Record<'device_type'|'brand'|'malfunction'|'completeness'|'condition'|'master', string[]>;
```

- [ ] **Step 1: Написать падающие тесты**

`column-prefs.test.ts`:
- нет сохранённого → порядок колонок как задан, скрыты `hiddenByDefault`, ширины по умолчанию;
- сохранённое с неизвестной колонкой → она отброшена;
- новая колонка → добавлена в конец;
- ширина 10 → поднята до `minWidth`, 5000 → ограничена 800;
- мусор в хранилище → значения по умолчанию.

`selection.test.ts`:
- `rangeSelect([1,2,3,4,5], 2, 4)` → `[2,3,4]`;
- `rangeSelect([1,2,3,4,5], 4, 2)` → `[2,3,4]`;
- якоря нет в списке → `[target]`.

`fake.test.ts`:
- `makeOrders(3, 1)` дважды даёт одинаковое;
- все телефоны начинаются с `7000`;
- все имена ВЕРХНИМ регистром;
- у каждого заказа есть статус из `DEMO_STATUSES`.

`e2e/table.spec.ts` (раздел «Таблица» витрины: `DataTable` на `makeOrders(10000)`):
- в DOM меньше 100 строк `role="row"`;
- прокрутка до конца показывает последний номер;
- клик по ячейке → активная строка (`aria-selected`), на демо «Открыт № …»;
- ↓ ↓ Enter с фокусом на таблице → открыт номер на две строки ниже, активная строка видна;
- Shift+клик выделяет диапазон, `SelectionBar` показывает «Выбрано 4», «Снять выделение» обнуляет;
- клик по заголовку «Принят» → `aria-sort="descending"`, повтор → `ascending`;
- перетаскивание края заголовка «Клиент» на +80px и перезагрузка → ширина сохранилась;
- «Колонки» → скрыть «Мастер» → после перезагрузки колонки нет;
- на ширине 390 раздел показывает `ListCard` вместо таблицы.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx vitest run src/lib/ui/table src/lib/demo && npx playwright test e2e/table.spec.ts` → FAIL.

- [ ] **Step 3: Реализовать**

- **`DataTable`:**
  - виртуальные строки — `createVirtualizer` из `@tanstack/svelte-virtual`: высота строки 2rem или 2.5rem в
    пикселях по плотности, `overscan: 10`;
  - разметка `role="grid"`, `aria-rowcount`, `aria-rowindex`; шапка `role="columnheader"` с `aria-sort`;
  - фокус на контейнере, `aria-activedescendant` — активная строка;
  - клавиши ↑↓ Home End PageUp PageDown Enter и пробел (при `selectable`);
  - `onendreached`, когда последний видимый индекс ≥ `rows.length − 20`;
  - `flashIds` → `data-flash`: анимация фона `--accent-soft` за 1,2 с, при уменьшенном движении её нет;
  - шапка закреплена; изменение ширины — перетаскиванием края (pointer events), двойной клик сбрасывает ширину;
  - ширины, порядок и скрытые колонки сохраняются через `saveColumnPrefs`.
- **`ColumnsMenu`** — `Popover` со списком флажков и кнопками «выше» и «ниже».
- **`InfiniteList`** — обычный список с наблюдателем за последним элементом (`IntersectionObserver`).
- **`fake.ts`** — вымышленные списки:
  - фамилии и имена;
  - бренды и модели;
  - неисправности;
  - мастера: «АНТОН», «СЕРГЕЙ», «ИРИНА».

- [ ] **Step 4: Раздел витрины «Таблица»**

`Table.svelte` с `id="table"`.
- **Колонки** по спеке 6.1: №, Принят, Клиент, Устройство, Неисправность, Статус, Мастер, Сумма, Отметки.
- **Скрытые колонки:** Телефон, Тип, Серийный/IMEI, Точка, Выдан, Предоплата, Штрихкод.
- **Ячейки:**
  - даты — `formatRelative`;
  - имена — `displayName`;
  - «Сумма» — итог или приглушённая оценка «≈ 6 000–7 000».

- [ ] **Step 5: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): virtual data table with column prefs, selection and keyboard; list cards; demo data"
```

---

### Task 9: Фото и файлы, выбор статуса, демо-экраны

**Files:**
- Create:
  - в `src/lib/ui/`: `PhotoStrip`, `FileDrop`, `StatusPicker` (`.svelte`);
  - `src/routes/design/sections/Media.svelte`;
  - `src/routes/design/orders/+page.svelte`, `src/routes/design/intake/+page.svelte`;
  - `e2e/orders-demo.spec.ts`, `e2e/intake-demo.spec.ts`.
- Modify: `design/+page.svelte` (ссылки на демо), `index.ts`, `ru.ts`

**Interfaces:**
- Consumes: всё из задач 2–8.
- Produces:
```ts
PhotoStrip: { photos: { id: string; src: string; alt: string }[]; max?: number /*10*/; label?: string /*'Фото устройства'*/;
  onadd?: () => void; onremove?: (id: string) => void; onopen?: (id: string) => void }
FileDrop: { accept?: string /*'image/*'*/; multiple?: boolean /*true*/; onfiles: (files: File[]) => void; children?: Snippet }
StatusPicker: { statuses: { id: number; name: string; color: string }[]; value: number /*$bindable*/; label?: string /*'Статус'*/;
  disabled?: boolean; onchange?: (id: number, previous: number) => void }
```

- [ ] **Step 1: Написать падающие тесты**

`e2e/orders-demo.spec.ts` (`/design/orders`, 2000 вымышленных заказов):
- **Открытие:**
  - клик по строке открывает панель «№ …» с полями;
  - клик по другой видимой строке переключает панель.
- **Клавиатура:**
  - ↓ при фокусе на панели переходит к следующему заказу списка;
  - Esc закрывает панель, фокус возвращается в таблицу на ту же строку;
  - `/` переводит фокус в фильтр.
- **Фильтр:**
  - «иванов» оставляет только строки с «Иванов» (имена хранятся ВЕРХНИМ регистром);
  - «ёлкин» находит «ЕЛКИН».
- **typing n in the filter does not open intake:** ввод «n» в фильтре не открывает приём, а N вне поля открывает
  `/design/intake`.
- **Статус в панели:** смена через `StatusPicker` на «Готов» показывает уведомление «№ … → Готов», «Отменить»
  возвращает прежний статус в таблице и панели.
- **Правка «Выполненные работы»:** «Сохраняю…», затем через 600 мс «✓ Сохранено».
- **«Выдать»:**
  - открывает диалог: «К оплате» = итог − предоплата − скидка;
  - пустое обязательное подсвечено;
  - Ctrl/⌘+Enter выдаёт, статус «Выдан».
- **Ctrl/⌘+K:**
  - открывает палитру, ввод номера показывает этот заказ первым;
  - Enter открывает его панель.
- **Ширина 390:**
  - карточки и нижняя навигация;
  - нажатие на карточку открывает заказ на весь экран с «Назад»;
  - кнопка статуса открывает список статусов снизу.

`e2e/intake-demo.spec.ts` (`/design/intake`):
- **Открытие:** широкая панель «Новый заказ», первое поле «Телефон» в фокусе.
- **Пустая форма:**
  - Ctrl/⌘+Enter не принимает;
  - в сводке «Не заполнено: Телефон или ФИО, Тип устройства, Неисправность»;
  - у полей ошибки.
- **Телефон:** ввод «8 000 000-00-01» показывает карточку найденного вымышленного клиента.
- **Чипы:** клик по чипу «Чехол» в комплектности отмечает его.
- **Приём:** после заполнения обязательного Ctrl/⌘+Enter закрывает панель и показывает «Заказ № … принят».
- **Черновик:** Esc закрывает панель; повторное открытие восстанавливает введённое.

- [ ] **Step 2: Убедиться, что тесты падают**

Run: `npx playwright test e2e/orders-demo.spec.ts e2e/intake-demo.spec.ts` → FAIL.

- [ ] **Step 3: Компоненты**

- **`StatusPicker`:**
  - группа радио в виде меток статусов в цветах оттенка; выбранная — залита `--st-dot` и обведена;
  - стрелки меняют выбор;
  - на телефоне — кнопка с текущим статусом, по ней `Dialog` со списком снизу.
- **`PhotoStrip`:** миниатюры с кнопкой удаления, плитка «＋», счётчик «3 из 10»; при `max` плитки «＋» нет.
- **`FileDrop`:**
  - подсветка при перетаскивании;
  - вставка изображений из буфера (`paste`);
  - скрытое `input type=file` за кнопкой «Выбрать файлы».

- [ ] **Step 4: Демо `/design/orders` — главный экран по спеке 6.1, 6.2, 6.4 и 6.12**

- Состояние — локальное (`$state`), данные — `makeOrders(2000)`.
- **Автосохранение** имитируется задержкой 600 мс.
- **«Требуют внимания»** считаются по данным:
  - Позвонить;
  - Заказать запчасть;
  - Долгая диагностика — статус вида `diagnostics` и принят больше 5 дней назад;
  - Давно готовы — вид `ready` и принят больше 30 дней назад;
  - пункты с нулём не показываются.
- **Клавиши** регистрируются здесь: N, `/`, Ctrl/⌘+K, `?`, Ctrl/⌘+Z.
- **Сканер:** `installScanner` открывает заказ по штрихкоду или номеру.

- [ ] **Step 5: Демо `/design/intake` — приём по спеке 6.5**

- Раскладка `Sheet`: форма и справа сводка с прошлыми заказами.
- Черновик в `localStorage` `mw.demo.intakeDraft`.
- Справочники — из `makeDictionaries()`.
- `PhotoStrip` и `FileDrop` с объектными URL выбранных картинок.

- [ ] **Step 6: Раздел «Медиа» и ссылки**

`Media.svelte` с `id="media"`: `PhotoStrip`, `FileDrop`, `StatusPicker`. На витрине — ссылки «Демо: главный экран» и
«Демо: приём».

- [ ] **Step 7: Проверить и закоммитить**

Run: `npm run verify && npm run test:e2e` → PASS.

```bash
git add apps/ui && git commit -m "feat(ui): photo strip, file drop, status picker; orders and intake demo screens"
```

---

### Task 10: Правила, документация, дорожная карта, финальная проверка

**Files:**
- Modify:
  - `AGENTS.md`;
  - `CONTRIBUTING.md`;
  - `README.md`;
  - `docs/superpowers/plans/README.md`;
  - `docs/superpowers/specs/2026-11-11-mywork3-crossplatform-design.md` — раздел 5.0: ссылка на спеку интерфейса.

- [ ] **Step 1: `AGENTS.md` — правила интерфейса**

- **Лицензии:** «OFL-1.1 — только для шрифтов (`@fontsource*`); всё, что попадает в сборку интерфейса, — в
  `dependencies`, их лицензии проверяет `npm run licenses`».
- **Проверка интерфейса** перед коммитом: `cd apps/ui && npm run verify && npm run test:e2e`.
- **Токены:** цвета, размеры, тени — только из `src/lib/design/tokens.css`; литералов цвета вне него нет (тест
  `no-raw-colors`).
- **Svelte 5 только на рунах**; обработчики — пропсы `on…`.
- **Компоненты** — из `$lib/ui`, новые — туда же, с разделом на витрине `/design`.
- **Тексты** — по-русски, общие строки — в `$lib/i18n/ru.ts`.
- **Иконки** — `@lucide/svelte` по одной.
- **Даты из API** — местное время без смещения, только через `$lib/format/date`.
- **Деньги** — копейки, только через `$lib/format/money` и `MoneyInput`.
- **Однобуквенные клавиши** — через `registerHotkey`: в полях и при открытых слоях не срабатывают, буквы — по
  `code`.
- **Типы API** — `import type` из `$api/…`, руками не дублировать.
- **Сквозные тесты** — Chromium и WebKit, запросы по ролям и подписям.

- [ ] **Step 2: `CONTRIBUTING.md` и `README.md`**

Раздел «Интерфейс»:
- Node ≥ 22.17;
- `cd apps/ui && npm ci`;
- `npm run dev` → `http://localhost:5173/design`;
- проверки: `npm run verify`, `npm run test:e2e`;
- первый запуск — `npx playwright install chromium webkit`.

- [ ] **Step 3: Дорожная карта**

- **Строка плана 4:**
  - ссылка на этот план;
  - «Что даёт» — «Спека интерфейса; `apps/ui`: токены и темы, шрифт, настройки устройства, форматирование,
    компоненты, таблица, слои, Ctrl K, витрина `/design` и демо-экраны; CI для интерфейса»;
  - статус «выполнен».
- **Строка плана 5:** «Что даёт» — «… по спеке интерфейса, разделы 6.1–6.12».
- **«Заметки для следующих планов»:**
  - в «План 5» перенести пункты раздела 12 спеки интерфейса;
  - заметку «решить, откуда интерфейс берёт `crates/mw-api/bindings/`» заменить на «решено: псевдоним `$api`»;
  - дописать всё, что всплыло при выполнении.

- [ ] **Step 4: Финальная проверка**

Run:
- в корне worktree — `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo check --workspace && cargo test --workspace`;
- в `apps/ui` — `npm ci && npm run verify && npm run test:e2e`.

Expected: всё зелёное. Прогон Rust-тестов — без `MW_TEST_PG_URL`, если PostgreSQL не запущен: Rust в этом плане не
меняется.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md CONTRIBUTING.md README.md docs
git commit -m "docs: UI rules, contributing, roadmap: plan 4 done"
```

## После плана

- Ревью всей ветки.
- Перемотка `main` на `plan-4-design-system` после зелёных проверок.
- Удаление worktree `.superpowers/worktrees/plan-4`.
