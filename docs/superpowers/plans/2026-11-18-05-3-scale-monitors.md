# План 5.3. Масштаб и разные мониторы — план реализации

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** интерфейс плотнее по умолчанию, сам подбирает размер под экран и ни на одном экране от 1280×720 до 4K
(включая Windows 125/150 %) ничего не наезжает и не уезжает за край; внешний вид этого компьютера — в «Настройках».

**Architecture:** размер интерфейса — по-прежнему корневой `font-size = 16px × масштаб`, всё остальное в `rem`.
Новое: масштаб «Авто» по разрешению экрана, ширины раскладки считаются в «ширине при масштабе 1»
(`ширина окна / масштаб`) и отдаются атрибутом `data-width` у `<html>` и реактивным `display`. Чистая логика —
в `display.ts`, её повторяет встроенный скрипт `app.html` до первой отрисовки; тест сверяет их. Новые токены
делают всё плотнее; сквозной тест обходит экраны на десятке размеров в Chromium и WebKit и проверяет геометрию.

**Tech Stack:** SvelteKit 3 SPA, Svelte 5 (руны), Vitest, Playwright (Chromium, WebKit) на настоящем `mw-server`.

**Spec:** `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` (разделы 3.3–3.6, 5, 6.8, 8, 10, 11);
заметки «План 5.3» в `docs/superpowers/plans/README.md`.

## Global Constraints

- Цвета, размеры, тени — только из `apps/ui/src/lib/design/tokens.css` (тест `no-raw-colors`).
- Svelte 5 только на рунах; импорты `#lib/...` с явными расширениями; типы API — `import type` из `$api/...`.
- Тексты по-русски, общие строки — в `#lib/i18n/ru.ts`; иконки — `@lucide/svelte`, по одной.
- Однобуквенные и прочие клавиши — через `registerHotkey`; Ctrl/⌘ — `Mod`.
- Сквозные тесты — Chromium и WebKit, на настоящем `mw-server` (`e2e/support/server.mjs`), данные через API с `uniq()`.
- Перед каждым коммитом зелёное: `cd apps/ui && npm run verify && npm run test:e2e`. Rust в плане не меняется.
- Коммиты: `feat(ui): …` / `fix(ui): …` / `docs: …` по-русски, последняя строка
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Шаги масштаба: `0.9 | 1 | 1.1 | 1.25 | 1.5` («90 %», «100 %», «110 %», «125 %», «150 %») и `'auto'` («Авто»),
  по умолчанию `'auto'`.
- Полосы ширины по `layoutWidth = innerWidth / масштаб`: `xs` < 600, `sm` < 900, `md` < 1024, `lg` < 1280,
  `xl` ≥ 1280. Раскладка каркаса: `xs` → `phone`, `sm`/`md`/`lg` → `rail`, `xl` → `full`.

## Review Focus

1. Настройки, сохранённые до 5.3 (`{"theme":"dark","scale":1}` без `v`), — тема остаётся, размер становится «Авто»;
   мусор в `mw.prefs` — значения по умолчанию без ошибки; встроенный скрипт и `display` дают одно и то же (без мигания).
2. Окно перетащили на другой монитор, сменилось разрешение или DPR, изменили размер окна — масштаб и полосы
   меняются сразу, без перезагрузки; смена масштаба сама не вызывает новой смены (нет петли).
3. Ручной крупный размер в маленьком окне (150 % при 1366×700) — каркас уходит в узкую полосу, а не вылезает за край.
4. Дробный корневой размер (90 %, 110 %) и DPR 1.25/1.5 — строки таблицы целых пикселей, виртуальный список не
   «плывёт», в конце списка нет пустой полосы.
5. Ctrl/⌘ + / − / 0 — в поле ввода, при открытом окне, с цифровой клавиатуры, в русской раскладке; на пределе
   шагов размер не уходит дальше.

---

### Task 1: Размер «Авто» и ширины по масштабу — модель

**Files:**
- Create: `apps/ui/src/lib/design/display.ts` (чистая логика, без рун)
- Create: `apps/ui/src/lib/design/display.test.ts`
- Modify: `apps/ui/src/lib/design/prefs.svelte.ts`, `apps/ui/src/lib/design/prefs.test.ts`,
  `apps/ui/src/lib/design/prefs-effect.svelte.test.ts`
- Modify: `apps/ui/src/app.html` (встроенный скрипт), `apps/ui/src/lib/design/app-html.test.ts`
- Modify: `apps/ui/src/lib/design/breakpoints.ts` (границы полос; `*_QUERY` пока остаются — их убирает Task 3)
- Modify: `apps/ui/src/routes/design/+page.svelte` (переключатель размера: «Авто» и пять шагов)
- Modify: `docs/superpowers/specs/2026-11-15-ui-design-system-design.md` — разделы 3.5 и 3.6

**Interfaces:**
- Produces (`display.ts`):
  ```ts
  export type UiScale = 0.9 | 1 | 1.1 | 1.25 | 1.5;
  export type ScalePref = 'auto' | UiScale;
  export type WidthBand = 'xs' | 'sm' | 'md' | 'lg' | 'xl';
  export const UI_SCALES: readonly UiScale[]; // по возрастанию
  export interface DisplayEnv { systemDark: boolean; screenWidth: number; screenHeight: number; windowWidth: number }
  export interface Display {
    theme: 'light' | 'dark'; density: Density; scale: UiScale;
    layoutWidth: number; band: WidthBand; layout: ShellLayout;
  }
  export function autoScale(screenWidth: number, screenHeight: number): UiScale;
  export function stepScale(current: UiScale, dir: 1 | -1): UiScale; // упирается в края
  export function bandForWidth(layoutWidth: number): WidthBand;
  export function computeDisplay(p: Prefs, env: DisplayEnv): Display;
  /** data-theme, data-density, data-width (= band), style --ui-scale. */
  export function applyDisplay(d: Display, root: HTMLElement): void;
  ```
- Produces (`prefs.svelte.ts`):
  ```ts
  export interface Prefs { theme: ThemePref; scale: ScalePref; density: Density }
  export const PREFS_VERSION = 2;
  export const DEFAULT_PREFS: Prefs; // { theme: 'system', scale: 'auto', density: 'comfortable' }
  export function parsePrefs(raw: string | null): Prefs;
  export const prefs: { readonly value: Prefs; readonly resolvedTheme: 'light' | 'dark';
    set(patch: Partial<Prefs>): void; init(): () => void };
  export const display: { readonly value: Display; readonly phone: boolean; // band xs
    readonly narrow: boolean; /* xs|sm */ readonly cardColumns: boolean; /* lg|xl */ };
  ```
  `ThemePref`, `Density` и `Prefs` объявляются в `display.ts` и реэкспортируются из `prefs.svelte.ts` (иначе
  модули импортируют друг друга); `ShellLayout` — из `breakpoints.ts`.

Правила:
- `autoScale`: `e = min(screenWidth, screenHeight × 16 / 9)`; `e < 2200` → 1; `< 2880` → 1.1; `< 3400` → 1.25;
  иначе 1.5. Нечисловое или ≤ 0 → 1. Масштаб «Авто» зависит от **экрана**, а не окна: размер точки экрана
  связан с разрешением монитора, а изменение размера окна не должно менять размер текста.
- `computeDisplay`: `scale = p.scale === 'auto' ? autoScale(...) : p.scale`; `layoutWidth = windowWidth / scale`.
- Хранение: `mw.prefs` = `{"v":2,"theme":…,"scale":…,"density":…}`. Без `v: 2` поле `scale` не читается
  (до 5.3 почти все сохранили `1` вместе с темой — иначе «Авто» никому не включится).
- `prefs.set` и события окружения применяют `display` к `<html>` синхронно. Окружение: `resize` и `focus` окна,
  `screen` `change` (где есть), смена `(resolution: …dppx)` через `matchMedia` (переподписка на новое значение),
  смена системной темы. Несколько событий подряд — одно обновление на кадр (`requestAnimationFrame`).

- [ ] **Step 1: Тесты чистой логики** (`display.test.ts`)

  ```ts
  it.each([
    [1280, 720, 1], [1728, 1117, 1], [1920, 1080, 1], [2048, 1152, 1], [2560, 1440, 1.1],
    [3440, 1440, 1.1], [3008, 1692, 1.25], [3840, 2160, 1.5], [0, 0, 1], [NaN, NaN, 1]
  ])('autoScale(%i, %i) = %f', (w, h, s) => expect(autoScale(w, h)).toBe(s));
  it('stepScale stops at the ends', () => {
    expect(stepScale(1, 1)).toBe(1.1); expect(stepScale(1.25, -1)).toBe(1.1);
    expect(stepScale(1.5, 1)).toBe(1.5); expect(stepScale(0.9, -1)).toBe(0.9);
  });
  it.each([[599, 'xs'], [600, 'sm'], [899, 'sm'], [900, 'md'], [1023, 'md'], [1024, 'lg'], [1279, 'lg'], [1280, 'xl']])
    ('bandForWidth(%i) = %s', (w, b) => expect(bandForWidth(w)).toBe(b));
  it('auto on a 4K screen in a half-width window', () => {
    const d = computeDisplay({ theme: 'system', scale: 'auto', density: 'comfortable' },
      { systemDark: true, screenWidth: 3840, screenHeight: 2160, windowWidth: 1920 });
    expect(d).toMatchObject({ theme: 'dark', scale: 1.5, layoutWidth: 1280, band: 'xl', layout: 'full' });
  });
  it('manual 125 % in a 1366 window is a rail', () => {
    const d = computeDisplay({ theme: 'light', scale: 1.25, density: 'compact' },
      { systemDark: false, screenWidth: 1366, screenHeight: 768, windowWidth: 1366 });
    expect(d).toMatchObject({ scale: 1.25, band: 'md', layout: 'rail', density: 'compact' });
  });
  it('applyDisplay sets the root', () => { /* data-theme, data-density, data-width, --ui-scale */ });
  ```

- [ ] **Step 2: Тесты хранения** (`prefs.test.ts`): `parsePrefs(null)` → `DEFAULT_PREFS`;
  `'{"theme":"dark","scale":1}'` → `{ theme: 'dark', scale: 'auto', density: 'comfortable' }`;
  `'{"v":2,"scale":1.25}'` → `scale: 1.25`; `'{"v":2,"scale":0.9375}'` → `'auto'`;
  `'{"v":2,"scale":"auto","density":"compact"}'` → как записано; `'{bad'` → `DEFAULT_PREFS`.
  `prefs.set({ scale: 1.1 })` пишет в хранилище объект с `v: 2`.

- [ ] **Step 3: Тесты живого обновления** (`prefs-effect.svelte.test.ts`): после `prefs.init()` и
  `prefs.set({ scale: 1.25 })` у `<html>` сразу `--ui-scale: 1.25` и `data-width` по `innerWidth / 1.25`;
  подменённые `innerWidth`/`screen.width` + событие `resize` → после кадра `data-width` и `--ui-scale` новые;
  `display.phone` / `narrow` / `cardColumns` следуют за полосой; функция, которую вернул `init`, снимает все
  подписки (повторное событие ничего не меняет).

- [ ] **Step 4: Тест встроенного скрипта** (`app-html.test.ts`): для матрицы
  (`raw` из Step 2) × (`systemDark` true/false) × (экран 1920×1080 окно 1920, экран 3840×2160 окно 1920,
  экран 1366×768 окно 1366) атрибуты `<html>` после скрипта равны
  `applyDisplay(computeDisplay(parsePrefs(raw), env))`; при бросающем `localStorage` — значения по умолчанию.

- [ ] **Step 5: Запустить — тесты падают** (`cd apps/ui && npx vitest run src/lib/design`).

- [ ] **Step 6: Реализовать** `display.ts`, новые `prefs.svelte.ts` и скрипт `app.html` (повторяет
  `parsePrefs` + `computeDisplay` + `applyDisplay` на ES5, без импорта). `breakpoints.ts`: экспорт границ полос
  `BAND_LIMITS = { xs: 600, sm: 900, md: 1024, lg: 1280 } as const`, `bandForWidth` берёт их.
  `/design`: переключатель размера «Авто · 90 % · 100 % · 110 % · 125 % · 150 %».

- [ ] **Step 7: Спека.** 3.6 — новые шаги, «Авто» и правило выбора, `v: 2`; 3.5 — полосы считаются по
  `ширина окна / масштаб`, таблица полос.

- [ ] **Step 8: Проверить** `cd apps/ui && npm run verify && npm run test:e2e` — зелёное.

- [ ] **Step 9: Commit** `feat(ui): размер «Авто» по экрану и ширины раскладки с учётом масштаба`.

---

### Task 2: Плотнее по умолчанию

**Files:**
- Modify: `apps/ui/src/lib/design/tokens.css`, `apps/ui/src/lib/design/tokens.test.ts`
- Create: `apps/ui/src/lib/ui/table/sizes.ts` (`ROW_REM`), тест сверки в `tokens.test.ts`
- Modify: `apps/ui/src/lib/ui/table/DataTable.svelte` (берёт `ROW_REM` из `sizes.ts`)
- Modify отступы (только перечисленные файлы): `SlideOver.svelte`, `Sheet.svelte`, `dialog.css`, `Section.svelte`,
  `Card.svelte`, `field.css`, `shell/SidebarNav.svelte`, `shell/TopBar.svelte`, `FilterChips.svelte`,
  `ChipGroup.svelte`, `src/lib/app/page-card.css`, `src/lib/intake/IntakeSheet.svelte`, `src/lib/orders/OrderPanel.svelte`,
  `src/lib/orders/OrderFields.svelte`, `src/lib/clients/ClientPanel.svelte`
- Modify: спека, разделы 3.3, 3.4, решение 3 в разделе 11 (пометка «заменено решением 48») и новое решение 48

**Interfaces:**
- Produces: `export const ROW_REM: Record<Density, number> = { comfortable: 2, compact: 1.75 }` в `sizes.ts`.

Новые значения токенов (при масштабе 1):

| Токен | Было | Стало |
|---|---|---|
| `--text-xs` / `-sm` / `-md` / `-lg` / `-xl` / `-2xl` | 12 / 13 / 14 / 16 / 20 / 24 px | 0.6875 / 0.75 / 0.8125 / 0.9375 / 1.125 / 1.375rem (11 / 12 / 13 / 15 / 18 / 22 px) |
| `--control-sm` / `-md` / `-lg` | 1.75 / 2.125 / 2.5rem | 1.5 / 1.875 / 2.25rem (24 / 30 / 36 px) |
| `--row-height` обычно / компактно | 2.5 / 2rem | 2 / 1.75rem (32 / 28 px) |
| `--topbar-height` | 3rem | 2.75rem |
| `--sidebar-width` | 14.5rem | 13rem |
| `--rail-width` | 3.5rem | 3rem |

`pointer: coarse` (2.75rem) не меняется. Ширина панели заказа и клиента — `min(28rem, 100vw − 3rem)`, широкой
панели приёма — `min(64rem, 100vw − 4rem)` (значения по умолчанию в `SlideOver`/`Sheet`).

Отступы в перечисленных файлах — на шаг меньше: поля слоёв и карточек по горизонтали `--space-5` → `--space-4`,
по вертикали `--space-4` → `--space-3`; промежутки между разделами `--space-6` → `--space-5`, `--space-5` →
`--space-4`; подпись → поле `--space-1-5` → `--space-1`. Промежутки внутри строк кнопок и чипов не меняются.

- [ ] **Step 1: Тесты.** `tokens.test.ts`: значения таблицы выше; `ROW_REM` совпадает с `--row-height` обеих
  плотностей в `tokens.css` (разбор файла). Запустить — падают.
- [ ] **Step 2: Токены, `sizes.ts`, `DataTable` на `ROW_REM`.** Тесты зелёные.
- [ ] **Step 3: Отступы** по правилу выше.
- [ ] **Step 4: Снимки до и после** (не в репозиторий): 1728×1050, «Заказы» с открытой панелью и открытый приём —
  в Chromium. При 1728×1050 и данных из 40 заказов видно не меньше 26 строк таблицы (было около 21). Приложить пути к снимкам в отчёт.
- [ ] **Step 5: Спека** 3.3, 3.4, решение 48: «Плотнее по умолчанию: основной текст 13px, строки 32/28px, поля
  30px — по просьбе автора („масштаб очень крупный“); крупнее — шагами размера».
- [ ] **Step 6: Проверить** `npm run verify && npm run test:e2e` — зелёное (e2e, завязанные на прежние размеры, —
  поправить ожидания, не обходя проверку).
- [ ] **Step 7: Commit** `feat(ui): плотнее по умолчанию — текст 13px, строки 32px, поля 30px`.

---

### Task 3: Ширины по масштабу во всех экранах; иконки и таблица растут вместе с текстом

**Files:**
- Modify (JS-ширины → `display`): `src/lib/ui/shell/AppShell.svelte`, `Dialog.svelte`, `ConfirmDialog.svelte`,
  `Sheet.svelte`, `SlideOver.svelte`, `StatusPicker.svelte`, `toast/Toaster.svelte`, `src/lib/app/AppTopBar.svelte`,
  `src/lib/clients/ClientPanel.svelte`, `src/lib/orders/OrderPanel.svelte`, `src/routes/(app)/orders/+page.svelte`,
  `src/routes/(app)/orders/[id]/+page.svelte`, `src/routes/design/sections/Table.svelte`
- Modify (CSS `@media` по ширине → `data-width`): `IntakeSheet.svelte`, `IntakeAside.svelte`, `ClientStep.svelte`,
  `OrderFields.svelte`, `IssueDialog.svelte`, `ClientOrders.svelte`, `ClientFields.svelte`, `ScanChoiceDialog.svelte`,
  `orders/[id]/+page.svelte`
- Modify: `src/lib/design/breakpoints.ts` (убрать `PHONE_QUERY`, `RAIL_QUERY`, `CARD_COLUMNS_QUERY`, `layoutForWidth`)
- Modify: `src/lib/design/base.css` (иконки в `rem`), `src/lib/ui/table/DataTable.svelte`
- Create: `src/lib/design/no-width-media.test.ts`
- Create: `e2e/scale.spec.ts`
- Modify: `AGENTS.md` (правило), спека 3.5

**Interfaces:**
- Consumes: `display.phone`, `display.narrow`, `display.cardColumns`, `display.value.layout`, `display.value.scale`
  (Task 1), `ROW_REM` (Task 2).

Замена CSS:
- `@media (max-width: 599px) { .x {…} }` → `:global([data-width='xs']) .x {…}`;
- `@media (max-width: 899px)` → `:global(:is([data-width='xs'], [data-width='sm'])) .x`;
- `@media (min-width: 1024px)` → `:global(:is([data-width='lg'], [data-width='xl'])) .x`.
`hover: hover`, `pointer: coarse`, `prefers-*` остаются медиазапросами.

Иконки: Lucide ставит `width`/`height` в пикселях, и при масштабе они не растут. В `base.css` —
`.lucide[width='14']` → `0.875rem`, `16` → `1rem`, `18` → `1.125rem`, `20` → `1.25rem`, `32` → `2rem`
(ширина и высота). Компоненты не меняются.

`DataTable`: высота строки в пикселях `rowPx = Math.round(ROW_REM[density] × 16 × display.value.scale)`, она же —
`height` строк в DOM (через стиль) и размер строки виртуального списка; ширины колонок умножаются на
`display.value.scale`. Чтение `getComputedStyle` убрать.

- [ ] **Step 1: Тест-линтер** `no-width-media.test.ts`: в `src/**/*.{svelte,css}` нет `@media` с `width`, в
  `src/**/*.{svelte,ts}` нет `new MediaQuery(` (кроме этого теста). Запустить — падает (перечисляет файлы).
- [ ] **Step 2: Тест таблицы** (`DataTable.test.ts`): при `scale` 1.1 и «обычно» строка `height: 35px`
  (2 × 17.6 = 35.2 → 35), при 0.9 и «компактно» — `25px` (25.2 → 25); позиция 10-й строки = 9 × `rowPx`.
- [ ] **Step 3: Перевести** все файлы из списка; `AppShell` берёт раскладку из `display.value.layout`.
  Тесты из Step 1–2 зелёные, `npm run verify` зелёное.
- [ ] **Step 4: e2e** `scale.spec.ts` (оба движка):
  - экран 3840×2160, окно 1920×1010, «Авто» → `getComputedStyle(html).fontSize === '24px'`, боковое меню полное
    (видна подпись «Заказы»), иконка «Заказы» в меню 24×24;
  - экран и окно 1366×700, размер 150 % (через `localStorage` `{"v":2,"scale":1.5}` до загрузки) → меню узкой
    полосой (подписи скрыты, у пунктов есть `aria-label`), у страницы нет горизонтальной прокрутки
    (`scrollWidth <= innerWidth`);
  - то же окно, событие смены размера окна до 1920×1010 → меню снова полное без перезагрузки.
  Экран задаётся опцией контекста Playwright `screen`.
- [ ] **Step 5: AGENTS.md** (раздел «Интерфейс»): «Ширины раскладки — только `display` (`#lib/design/prefs.svelte.js`)
  и `:global([data-width=…])`; медиазапросы по ширине и `MediaQuery` запрещены (тест `no-width-media`). Иконки
  растут с масштабом через `base.css`». Спека 3.5 — то же.
- [ ] **Step 6: Проверить** `npm run verify && npm run test:e2e` — зелёное.
- [ ] **Step 7: Commit** `feat(ui): раскладка и иконки следуют за масштабом`.

---

### Task 4: «Настройки → Внешний вид», клавиши размера

**Files:**
- Create: `src/routes/(app)/settings/+layout.svelte` (подразделы слева, содержимое справа; на `phone` — список,
  затем раздел с «←»), `src/routes/(app)/settings/+page.ts` (переход на `/settings/appearance`),
  `src/routes/(app)/settings/appearance/+page.svelte`
- Create: `src/lib/settings/sections.ts`
- Modify: `src/lib/app/AppSidebar.svelte` (внизу «Настройки»), `src/lib/ui/shell/SidebarNav.svelte` (нижний раздел,
  если его ещё нет), `src/routes/(app)/+layout.svelte` (команды Ctrl K, клавиши, «Ещё» на телефоне),
  `src/lib/design/hotkeys.svelte.ts` (+ тест), `src/lib/ui/ShortcutsHelp` (через список `SHORTCUTS`),
  `src/lib/i18n/ru.ts`
- Create: `e2e/settings.spec.ts`
- Modify: спека 6.8 и 8

**Interfaces:**
- Consumes: `prefs.set`, `display.value.scale`, `UI_SCALES`, `stepScale` (Task 1).
- Produces: `export const SETTINGS_SECTIONS: readonly { id: string; label: string; icon: Component; href: string }[]`
  — сейчас один раздел `appearance`; план 5.4 добавляет свои.

Раздел «Внешний вид» — карточка с подписью «Только на этом компьютере», каждое поле применяется сразу:
- «Тема» — `SegmentedControl`: «Как в системе», «Светлая», «Тёмная»;
- «Размер» — `SegmentedControl`: «Авто», «90 %», «100 %», «110 %», «125 %», «150 %»; под ним при «Авто»:
  «Подбирается по экрану: сейчас 110 %»;
- «Плотность таблиц» — «Обычная», «Компактная».

Клавиши (работают в полях и поверх окон, `allowInFields` + `allowWithOverlay`): `Mod+Plus` — крупнее,
`Mod+Minus` — мельче (от текущего действующего масштаба, `stepScale`), `Mod+0` — «Авто». После каждого нажатия —
уведомление «Размер 110 %» или «Размер: авто (110 %)». Браузерное увеличение этими клавишами заменяется размером
программы (решение 49). `matchShortcut`: ключ `Plus` — `code` `Equal` или `NumpadAdd` при любом Shift; `Minus` —
`Minus` или `NumpadSubtract`; `0` — `Digit0` или `Numpad0`; `formatShortcut` пишет `+` и `−`.

Ctrl K, группа «Команды»: «Настройки», «Внешний вид», «Крупнее», «Мельче», «Размер по экрану».

- [ ] **Step 1: Тесты клавиш** (`hotkeys.test.ts`): `Mod+Plus` срабатывает на `{ code: 'Equal', key: '=' }`,
  `{ code: 'Equal', key: '+', shiftKey: true }`, `{ code: 'NumpadAdd', key: '+' }` с Ctrl (ПК) / ⌘ (Mac) и не
  срабатывает без модификатора; `Mod+Minus` — `Minus`/`NumpadSubtract`; `Mod+0` — `Digit0`/`Numpad0`;
  `formatShortcut('Mod+Plus', { mac: false })` = `'Ctrl +'`. Запустить — падают.
- [ ] **Step 2: Реализовать** `matchShortcut`/`formatShortcut`, клавиши и команды в `(app)/+layout.svelte`,
  строки в `ru.ts`, строки «Крупнее / мельче / по экрану» в справке клавиш.
- [ ] **Step 3: e2e** `settings.spec.ts` (оба движка, сервер 4173):
  - «Настройки» в боковом меню → `/settings/appearance`, заголовок «Внешний вид», «Только на этом компьютере»;
  - «Тёмная» → `html[data-theme=dark]`; «125 %» → `fontSize` `20px`; перезагрузка — сохранилось;
  - «Авто» → `16px` при окне 1280×720, подпись «сейчас 100 %»;
  - фокус в поле поиска, Ctrl/⌘ + `=` → `17.6px` и уведомление «Размер 110 %»; ⌘/Ctrl + `0` → «Размер: авто (100 %)»;
  - пять раз Ctrl/⌘ + `=` → `24px`, дальше не растёт;
  - «Компактная» → строка таблицы «Заказы» высотой 28px;
  - Ctrl K «Внешний вид» открывает раздел.
- [ ] **Step 4: Телефон** — в окне «Ещё» вместо переключателя темы ссылка «Настройки» (тема — в настройках).
- [ ] **Step 5: Спека** 6.8 — «Внешний вид» сделан в 5.3 (`/settings/appearance`), остальные подразделы — план 5.4;
  раздел 8 — Ctrl/⌘ + / − / 0; решение 49 «Клавиши размера заменяют браузерное увеличение: раскладка тогда
  остаётся по масштабу, а размер запоминается на компьютере».
- [ ] **Step 6: Проверить** `npm run verify && npm run test:e2e` — зелёное.
- [ ] **Step 7: Commit** `feat(ui): настройки «Внешний вид» и клавиши размера`.

---

### Task 5: Обход экранов на разных размерах

**Files:**
- Create: `e2e/support/layout.ts`, `e2e/layout.spec.ts`
- Modify: `.github/workflows/ci.yml` (задание `ui-e2e`: выгрузка снимков)
- Modify: файлы интерфейса, где проверка нашла поломки (каждая правка — с причиной в отчёте)

**Interfaces:**
- Produces (`e2e/support/layout.ts`):
  ```ts
  export interface ScreenCase { name: string; screen: { width: number; height: number };
    viewport: { width: number; height: number }; dpr: number; scale?: number /* ручной; нет — «Авто» */; rootPx: number }
  export const SCREEN_CASES: readonly ScreenCase[];
  /** Fails with a list of offenders: overflow, overlaps, cut labels. */
  export async function expectTidyLayout(page: Page, bars: Locator[]): Promise<void>;
  ```

Размеры (окно = экран без рамки и панели задач, около 70px по высоте):

| name | экран | окно | DPR | размер | корень |
|---|---|---|---|---|---|
| `720p` | 1280×720 | 1280×650 | 1 | Авто | 16px |
| `768p` | 1366×768 | 1366×700 | 1 | Авто | 16px |
| `900p` | 1440×900 | 1440×830 | 1 | Авто | 16px |
| `mbp16` | 1728×1117 | 1728×1050 | 2 | Авто | 16px |
| `1080p` | 1920×1080 | 1920×1010 | 1 | Авто | 16px |
| `1080p-half` | 1920×1080 | 960×1010 | 1 | Авто | 16px |
| `1440p` | 2560×1440 | 2560×1370 | 1 | Авто | 17.6px |
| `4k` | 3840×2160 | 3840×2090 | 1 | Авто | 24px |
| `win125` | 1536×864 | 1536×794 | 1.25 | Авто | 16px |
| `win150` | 1280×720 | 1280×650 | 1.5 | Авто | 16px |
| `768p-150` | 1366×768 | 1366×700 | 1 | 150 % | 24px |
| `720p-90` | 1280×720 | 1280×650 | 1 | 90 % | 14.4px |

Сцены на каждом размере (одна страница, по очереди): «Заказы» (не меньше 40 заказов, созданных через API);
открытая панель заказа; карточка `/orders/<номер>`; открытый приём (N) с заполненным клиентом; «Клиенты» с открытой
панелью клиента; «Настройки → Внешний вид»; окно «Выдать»; Ctrl K. Вход (`/login`, сервер 4174) — на `720p` и `4k`.

`expectTidyLayout` проверяет и при нарушении перечисляет элементы (роль, имя, прямоугольник):
1. у страницы нет прокрутки: `scrollWidth <= innerWidth`, `scrollHeight <= innerHeight`;
2. ни одна видимая кнопка, ссылка, поле, вкладка, пункт меню и окно не выходит за правый или левый край окна и
   своего ближайшего обрезающего предка, кроме предков с горизонтальной прокруткой (таблица);
3. в каждой переданной полосе (`bars`: верхняя панель, шапки и подвалы панелей, приёма и окон, строка фильтров)
   видимые элементы управления не пересекаются и целиком внутри полосы по высоте;
4. подписи кнопок, вкладок и полей не обрезаны: `scrollWidth <= clientWidth + 1` (многоточие в ячейках таблицы —
   норма, их не проверять);
5. `getComputedStyle(html).fontSize` равен `rootPx` случая.

Каждая сцена сохраняет снимок `testInfo.outputPath('layout-<case>-<scene>.png')`; снимки не сравниваются с
эталоном (шрифты на разных ОС рисуются по-разному — решение 50), их смотрят глазами. В CI задание `ui-e2e`
выгружает `apps/ui/test-results` артефактом `ui-e2e-results` (`if: always()`, хранить 7 дней).

- [ ] **Step 1: Помощник и спека** `layout.spec.ts` — по тесту на случай и проект (`test.describe.configure({ mode: 'parallel' })`);
  контекст: `viewport`, `screen`, `deviceScaleFactor`; ручной размер — через `localStorage` до загрузки.
- [ ] **Step 2: Запустить** `npx playwright test e2e/layout.spec.ts` — записать все найденные нарушения.
- [ ] **Step 3: Исправить** нарушения в интерфейсе (не ослабляя проверку; исключение — только с причиной в коде
  помощника и в отчёте). Типичное: верхняя панель на `720p` (поиск сжимается, подписи кнопок уходят в иконки с
  подсказкой), подвал приёма, окна по высоте на 650px (тело прокручивается, подвал виден).
- [ ] **Step 4: Проверить глазами** снимки `720p`, `mbp16`, `1080p-half`, `4k`, `768p-150` в обоих движках;
  пути к ним — в отчёт.
- [ ] **Step 5: CI** — шаг выгрузки артефакта.
- [ ] **Step 6: Проверить** `npm run verify && npm run test:e2e` — зелёное; время всего `test:e2e` в отчёт.
- [ ] **Step 7: Commit** `test(ui): обход экранов на разных размерах и мониторах`.
