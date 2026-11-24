import { expect, test, type APIRequestContext, type Locator, type Page } from '@playwright/test';
import type { Client } from '$api/Client';
import type { ImportReportView } from '$api/ImportReportView';
import type { LegacyImportItem } from '$api/LegacyImportItem';
import type { ServerInfo } from '$api/ServerInfo';
import type { ServicePoint } from '$api/ServicePoint';
import {
	createClient,
	createOrder,
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	patchOrder,
	sessionToken,
	statusId,
	uniq,
	uniqPhone
} from './support/api';
import {
	expectTidyLayout,
	SCREEN_CASES,
	scrollBodiesToEnd,
	type ScreenCase
} from './support/layout';

// Every screen case (spec 5.3) goes through the main scenes on one page and checks the geometry; each
// scene leaves a screenshot `layout-<case>-<scene>.png` for a person to look at (not compared with a
// reference: fonts draw differently on every OS, decision 50). Server of 4173: open mode, every right.

test.describe.configure({ mode: 'parallel' });

// The program writes the keys as on the system: «⌘K» on a Mac, «Ctrl K» elsewhere, which is longer. The
// sweep goes with the longer one everywhere (as CI on Linux does), so a Mac does not hide a cut label.
test.beforeEach(async ({ page }) => {
	await page.addInitScript(() =>
		Object.defineProperty(Navigator.prototype, 'platform', {
			get: () => 'Linux x86_64',
			configurable: true
		})
	);
});

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

/**
 * The data of the scenes: a list longer than any window, an order with every field, one to issue. The
 * orders are at a point of their own, chosen on the device: the list shows only them (not other tests'
 * orders), and the top bar carries the point's button with a long name.
 */
interface Seed {
	marker: string;
	point: number;
	full: number;
	ready: number;
	client: Client;
}

// Fictional people of different name lengths.
const NAMES = [
	'Константинопольская Анастасия Владимировна',
	'Ли Ан',
	'Тестов Пётр Сергеевич',
	'Обходчикова-Широкова Екатерина',
	'Семёнов Аркадий',
	'Примерова Ольга Ивановна',
	'Длинноименный Вячеслав Александрович',
	'Ким Юна'
];
const DEVICES = [
	['Смартфон', 'Samsung', 'Galaxy A54'],
	['Ноутбук', 'Lenovo', 'IdeaPad 5 Pro 16ACH6'],
	['Планшет', 'Apple', 'iPad Air'],
	['Телевизор', 'LG', 'OLED55C3RLA'],
	['Смарт-часы', 'Huawei', 'Watch GT 4'],
	['Игровая приставка', 'Sony', 'PlayStation 5']
] as const;
const MALFUNCTIONS = [
	'Не включается',
	'Разбит экран, после падения не реагирует на касания в нижней части',
	'Не заряжается',
	'Сильно греется под нагрузкой и выключается через десять минут работы',
	'Нет звука'
];

async function inBatches<T>(n: number, make: (i: number) => Promise<T>): Promise<T[]> {
	const out: T[] = [];
	for (let i = 0; i < n; i += 8) {
		const batch = Array.from({ length: Math.min(8, n - i) }, (_, k) => make(i + k));
		out.push(...(await Promise.all(batch)));
	}
	return out;
}

async function makeSeed(request: APIRequestContext): Promise<Seed> {
	const marker = uniq('Обход');
	const ready = await statusId(request, 'Готов');
	const created = await request.post('/api/v1/service-points', {
		data: { name: `ТЦ «Галерея», 2 этаж, ${marker}` }
	});
	expect(created.ok(), await created.text()).toBe(true);
	const point = ((await created.json()) as ServicePoint).id;
	const clients = await inBatches(NAMES.length, (i) =>
		createClient(request, { full_name: NAMES[i]!, phones: [{ phone: uniqPhone(), note: '' }] })
	);
	// 40 in work (the list by default) and 6 ready.
	await inBatches(46, (i) => {
		const [device_type, brand, model] = DEVICES[i % DEVICES.length]!;
		return createOrder(request, {
			client: { existing: clients[i % clients.length]!.id },
			service_point_id: point,
			device_type,
			brand,
			model: `${model} ${marker}`,
			malfunction: MALFUNCTIONS[i % MALFUNCTIONS.length]!,
			prepayment: i % 3 === 0 ? 150000 : 0,
			estimate_from: i % 4 === 0 ? 450000 : null,
			need_call: i % 5 === 0,
			need_parts: i % 7 === 0,
			status_id: i >= 40 ? ready : null
		});
	});
	const client = clients[0]!;
	const issuable = await createOrder(request, {
		client: { existing: client.id },
		service_point_id: point,
		device_type: 'Смартфон',
		brand: 'Xiaomi',
		model: `Redmi Note 13 Pro ${marker}`,
		malfunction: 'Не работает кнопка питания',
		status_id: ready,
		estimate_from: 405000,
		prepayment: 50000
	});
	const full = await createOrder(request, {
		client: { existing: client.id },
		service_point_id: point,
		device_type: 'Ноутбук',
		brand: 'ASUS',
		model: `ROG Strix G16 G614JV ${marker}`,
		serial: 'SN-TEST-0000-1234-5678',
		color: 'Серый металлик',
		device_password: '1234',
		completeness: 'Зарядное устройство, сумка, мышь',
		device_condition: 'Потёртости на крышке, скол на углу',
		malfunction:
			'После обновления BIOS не включается, индикатор питания мигает оранжевым, вентиляторы не крутятся',
		internal_comment: 'Клиент просит позвонить перед заменой материнской платы',
		master: 'Тестов',
		estimate_from: 1200000,
		estimate_to: 1850000,
		prepayment: 300000,
		warranty: '90 дней',
		need_call: true,
		need_parts: true
	});
	await patchOrder(request, full.id, {
		work_done: 'Диагностика, прошивка BIOS программатором, чистка системы охлаждения'
	});
	// A long name in the rows of «Статусы» and «Справочники»: archived at once, so no picker of another
	// spec shows it, while the archive of the screen does.
	const status = await request.post('/api/v1/statuses', {
		data: {
			name: `Согласование стоимости ремонта с клиентом по телефону, ${marker}`,
			kind: 'diagnostics',
			color: 'violet'
		}
	});
	expect(status.ok(), await status.text()).toBe(true);
	const statusIdOf = ((await status.json()) as { id: number }).id;
	await request.patch(`/api/v1/statuses/${statusIdOf}`, { data: { archived: true } });
	const value = await request.post('/api/v1/dictionaries/device_type', {
		data: { value: `Многофункциональное устройство печати и сканирования ${marker}` }
	});
	expect(value.ok(), await value.text()).toBe(true);
	const valueId = ((await value.json()) as { id: number }).id;
	await request.patch(`/api/v1/dictionary-items/${valueId}`, { data: { archived: true } });
	return { marker, point, full: full.id, ready: issuable.id, client };
}

// Once per worker: the scenes only look at these orders. A rejected seed is forgotten, so the next test
// seeds again instead of failing on the same rejection.
let seeded: Promise<Seed> | undefined;
function seedOnce(request: APIRequestContext): Promise<Seed> {
	seeded ??= makeSeed(request).catch((e: unknown) => {
		seeded = undefined;
		throw e;
	});
	return seeded;
}

/** No requests in flight for a moment, fonts loaded, transitions done: the screen is still. */
async function settle(page: Page, pending: () => number) {
	for (let quiet = 0, waited = 0; quiet < 3 && waited < 100; waited++) {
		quiet = pending() === 0 ? quiet + 1 : 0;
		await page.waitForTimeout(50);
	}
	await page.evaluate(async () => {
		await document.fonts.ready;
		const finite = document.getAnimations().filter((a) => {
			const end = a.effect?.getComputedTiming().endTime;
			return typeof end === 'number' && Number.isFinite(end);
		});
		await Promise.all(finite.map((a) => a.finished.catch(() => undefined)));
		await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));
	});
}

function watchRequests(page: Page): () => number {
	let pending = 0;
	page.on('request', () => pending++);
	page.on('requestfinished', () => pending--);
	page.on('requestfailed', () => pending--);
	return () => pending;
}

/** Screenshot first (also for a failing scene), then the five checks. */
async function scene(
	page: Page,
	c: ScreenCase,
	name: string,
	bars: Locator[],
	pending: () => number
) {
	await test.step(name, async () => {
		// Off every control: no hover, no tooltip in the picture.
		await page.mouse.move(0, c.viewport.height - 1);
		await settle(page, pending);
		await page.screenshot({ path: test.info().outputPath(`layout-${c.name}-${name}.png`) });
		await expectTidyLayout(page, bars, { rootPx: c.rootPx, band: c.band });
		// The bodies of panels and dialogs that scroll: once more at their end, where the last fields are.
		if ((await scrollBodiesToEnd(page)) > 0) {
			await settle(page, pending);
			await expectTidyLayout(page, bars, {
				rootPx: c.rootPx,
				band: c.band,
				label: ' [scrolled to the end]'
			});
		}
	});
}

const banner = (page: Page) => page.getByRole('banner');
/** The innermost block that holds all the given controls: a row of controls without a role of its own. */
const rowWith = (page: Page, ...controls: Locator[]) =>
	controls.reduce((row, has) => row.filter({ has }), page.getByRole('main').locator('div')).last();
// Panels and dialogs: their header and footer have no role, so their own classes find them.
const panelBars = (panel: Locator) => [
	panel.locator(':scope > .head'),
	panel.locator(':scope > .foot')
];
const dialogBars = (dialog: Locator) => [
	dialog.locator(':scope > .mw-dialog-head'),
	dialog.locator(':scope > .mw-dialog-foot')
];
const orderPanel = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const orderRow = (page: Page, id: number) =>
	page
		.getByRole('grid', { name: 'Заказы' })
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
/** A phone shows the list as cards: buttons named by the number. */
const orderCard = (page: Page, id: number) =>
	page.getByRole('main').getByRole('button', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const bottomNav = (page: Page) => page.getByRole('navigation', { name: 'Основная навигация' });

async function openOrderPanel(page: Page, c: ScreenCase, seed: Seed, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(seed.marker)}&chips=all`);
	if (c.phone) await orderCard(page, id).click();
	else await orderRow(page, id).getByRole('gridcell').nth(3).click();
	await expect(orderPanel(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	return orderPanel(page, id);
}

/** The intake sheet just opened: the client found by the phone and chosen, then the sheet closed. */
async function intakeScene(page: Page, c: ScreenCase, seed: Seed, pending: () => number) {
	const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
	await expect(sheet.getByRole('textbox', { name: 'Телефон' })).toBeFocused();
	await page.keyboard.type(seed.client.phones[0]!.phone);
	await sheet
		.getByRole('list', { name: 'Найденные клиенты' })
		.getByRole('listitem')
		.filter({ hasText: seed.client.full_name })
		.getByRole('button', { name: 'Это он' })
		.click();
	await expect(sheet.getByRole('region', { name: 'Клиент заказа' })).toContainText(
		seed.client.full_name
	);
	await scene(page, c, 'intake', dialogBars(sheet), pending);
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
}

// The server of the sweep is set up and has no import (an import there could only replace its data): the
// past imports and a report are answers made up here, with long paths and many numbers.
const LONG_PATH =
	'C:\\Users\\Администратор\\Documents\\Сервисный центр «Галерея»\\MyWork2 (копия от 01.11.2026)';
const SAMPLE_IMPORTS: LegacyImportItem[] = [
	{
		id: 9002,
		imported_at: '2026-11-19T14:05:09',
		source_path: LONG_PATH,
		mode: 'replace',
		orders: 10482,
		warnings: 314
	},
	{
		id: 9001,
		imported_at: '2026-11-18T09:30:00',
		source_path: 'D:\\MyWork2',
		mode: 'into_empty',
		orders: 10479,
		warnings: 0
	}
];
const ids = (from: number, n: number) => Array.from({ length: n }, (_, i) => from + i * 7);
const LONG_NOTE =
	'Клиента с таким номером нет в ClientsMap, он восстановлен из полей заказа — длинное пояснение, которое должно переноситься по словам и не раздвигать строку';
const SAMPLE_REPORT: ImportReportView = {
	id: 9002,
	imported_at: '2026-11-19T14:05:09',
	source_path: LONG_PATH,
	mode: 'replace',
	orders: 10482,
	clients: 6311,
	users: 4,
	stock_items: 0,
	files: { copied: 3214, bytes: 1_932_735_283, missing: 12 },
	groups: [
		{
			code: 'date_unparsed',
			severity: 'warning',
			title: 'Не разобрана дата',
			count: 390,
			order_ids: ids(10001, 150),
			entries: [
				{
					source: 'HistoryBD',
					key: '88412',
					field: 'DATA',
					value: '31-02-2016 25:61',
					message: 'Дата записи не разобрана: взята дата приёма заказа или время импорта.'
				},
				{ source: 'StatesMap', key: '9021', field: 'date', value: 'вчера', message: '' }
			],
			entries_count: 240
		},
		{
			code: 'barcode_duplicate',
			severity: 'warning',
			title: 'Одинаковый штрихкод у нескольких заказов',
			count: 277,
			order_ids: ids(10003, 40),
			entries: [],
			entries_count: 0
		},
		{
			code: 'client_missing',
			severity: 'warning',
			title: 'Клиент не найден — восстановлен из заказа',
			count: 3,
			order_ids: [],
			entries: [
				{
					source: 'Catalog',
					key: '10005',
					field: 'ClientId',
					value: '4021',
					message: LONG_NOTE
				},
				{ source: 'Catalog', key: '10012', field: 'ClientId', value: '4033', message: '' }
			],
			entries_count: 3
		},
		{
			code: 'phone_split',
			severity: 'info',
			title: 'Телефоны разложены по номерам',
			count: 812,
			order_ids: ids(10010, 12),
			entries: [],
			entries_count: 0
		},
		{
			code: 'status_custom',
			severity: 'info',
			title: 'Добавлен статус из старой базы',
			count: 2,
			order_ids: [],
			entries: [
				{
					source: 'Catalog',
					key: 'Согласование с клиентом',
					field: 'Status_remonta',
					value: 'Согласование с клиентом',
					message: ''
				}
			],
			entries_count: 2
		}
	],
	warnings_text: 'Отчёт',
	files_pending: true
};

/** «Импорт из MyWork 2»: the past imports, then the report of one with its first group open. */
async function importScenes(page: Page, c: ScreenCase, bars: Locator[], pending: () => number) {
	await page.route('**/api/v1/legacy-imports', (route) => route.fulfill({ json: SAMPLE_IMPORTS }));
	await page.route('**/api/v1/legacy-imports/9002', (route) =>
		route.fulfill({ json: SAMPLE_REPORT })
	);
	await page.goto('/settings/import');
	await expect(page.getByRole('heading', { name: 'Импорт из MyWork 2' })).toBeVisible();
	await expect(page.getByRole('list', { name: 'Прошлые импорты' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Перенести снова' })).toBeVisible();
	await scene(page, c, 'settings-import', bars, pending);

	await page.goto('/settings/import/9002');
	await expect(page.getByRole('heading', { name: 'Отчёт о переносе' })).toBeVisible();
	await page.getByRole('button', { name: /^Не разобрана дата/ }).click();
	await expect(page.getByRole('button', { name: /^Показать все/ })).toBeVisible();
	await expect(page.getByRole('list', { name: 'Записи: Не разобрана дата' })).toBeVisible();
	await expect(page.getByText('Показаны первые 2 из 240', { exact: false })).toBeVisible();
	await page.getByRole('button', { name: /^Клиент не найден/ }).click();
	await expect(
		page.getByRole('list', { name: 'Записи: Клиент не найден — восстановлен из заказа' })
	).toBeVisible();
	await scene(page, c, 'import-report', bars, pending);
	await page.unroute('**/api/v1/legacy-imports');
	await page.unroute('**/api/v1/legacy-imports/9002');
}

/**
 * The first run, outside the shell: the server's answer says the base is new (nothing is sent), the choice
 * and the form of an empty base. Last of the scenes: the answer is put back after it.
 */
async function setupScenes(page: Page, c: ScreenCase, pending: () => number) {
	const info = '**/api/v1/server-info';
	await page.route(info, async (route) => {
		const response = await route.fetch();
		const body = (await response.json()) as ServerInfo;
		await route.fulfill({ response, json: { ...body, setup_required: true } });
	});
	await page.goto('/setup');
	await expect(page.getByRole('heading', { name: 'Первый запуск' })).toBeVisible();
	await scene(page, c, 'setup', [], pending);
	await page.getByRole('button', { name: 'Начать с пустой базы' }).click();
	await expect(page.getByRole('textbox', { name: /^Название сервиса/ })).toBeVisible();
	await page.getByRole('textbox', { name: 'Мастера' }).fill('Константинопольский Вячеслав');
	await page.getByRole('textbox', { name: 'Мастера' }).press('Enter');
	await scene(page, c, 'setup-form', [], pending);
	await page.unroute(info);
}

/**
 * The rows of statuses, points and dictionaries, and the row of their column titles: nothing in a row runs
 * over its neighbour or out of the row. The form of a new status is a row of the same columns.
 */
const catalogRows = (page: Page) => [
	page.getByRole('main').getByRole('listitem'),
	page.getByRole('main').locator('.catalog-head')
];

/** «Пользователи»: long names and logins, no sign-in, no password, a disabled one; then the edit dialog. */
async function usersScenes(page: Page, c: ScreenCase, bars: Locator[], pending: () => number) {
	const long = 'Константинопольский-Вячеславович '.repeat(3).trim();
	const user = (id: number, over: Record<string, unknown>) => ({
		id,
		login: `login${id}`,
		display_name: `Пользователь ${id}`,
		role_id: 1,
		active: true,
		can_sign_in: true,
		is_master: false,
		has_password: true,
		created_at: '2026-01-01T00:00:00',
		version: 1,
		...over
	});
	const users = [
		user(1, { display_name: long, login: long.replace(/ /g, ''), is_master: true }),
		user(2, { login: '', can_sign_in: false, is_master: true, has_password: false }),
		user(3, { has_password: false }),
		user(4, { active: false })
	];
	const roles = [{ id: 1, name: long, permissions: [], version: 1 }];
	await page.route('**/api/v1/users', (route) => route.fulfill({ json: users }));
	await page.route('**/api/v1/roles', (route) => route.fulfill({ json: roles }));
	await page.goto('/settings/users');
	await expect(page.getByRole('heading', { name: 'Пользователи' })).toBeVisible();
	await expect(page.getByRole('list', { name: 'Отключённые' })).toBeVisible();
	await scene(page, c, 'users', [...bars, page.getByRole('main').getByRole('listitem')], pending);
	await page.getByRole('button', { name: 'Изменить: Пользователь 3' }).click();
	const dialog = page.getByRole('dialog', { name: 'Пользователь' });
	await expect(dialog.getByText('Пароля нет', { exact: false })).toBeVisible();
	await scene(page, c, 'users-edit', [], pending);
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await page.unroute('**/api/v1/users');
	await page.unroute('**/api/v1/roles');
}

/** «Роли»: a long name, no users, many users, the rights in groups with their hints; then the edit dialog. */
async function rolesScenes(page: Page, c: ScreenCase, bars: Locator[], pending: () => number) {
	const long = 'Константинопольский-Вячеславович '.repeat(3).trim();
	const roles = [
		{ id: 1, name: long, permissions: ['orders_edit', 'users_manage'], version: 1 },
		{ id: 2, name: 'Мастер', permissions: ['orders_edit'], version: 1 },
		{ id: 3, name: 'Пустая', permissions: [], version: 1 }
	];
	const users = [1, 2, 3].map((id) => ({ id, role_id: id === 3 ? 1 : 2 }));
	await page.route('**/api/v1/users', (route) => route.fulfill({ json: users }));
	await page.route('**/api/v1/roles', (route) => route.fulfill({ json: roles }));
	await page.goto('/settings/roles');
	await expect(page.getByRole('heading', { name: 'Роли' })).toBeVisible();
	await expect(page.getByRole('list', { name: 'Роли' })).toBeVisible();
	await scene(page, c, 'roles', [...bars, page.getByRole('main').getByRole('listitem')], pending);
	await page.getByRole('button', { name: 'Изменить: Мастер' }).click();
	const dialog = page.getByRole('dialog', { name: 'Роль', exact: true });
	await expect(dialog.getByRole('checkbox', { name: 'Изменение заказов' })).toBeChecked();
	await scene(page, c, 'roles-edit', [], pending);
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await page.unroute('**/api/v1/users');
	await page.unroute('**/api/v1/roles');
}

/** «Настройки → Общие», «Заказы» and «О программе»: each once its fields are filled from the server. */
async function serverSettingsScenes(
	page: Page,
	c: ScreenCase,
	bars: Locator[],
	pending: () => number
) {
	await page.goto('/settings/general');
	await expect(page.getByRole('heading', { name: 'Общие' })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Следующий номер заказа' })).toHaveValue(/^\d+$/);
	await scene(page, c, 'general', bars, pending);

	await page.goto('/settings/orders');
	await expect(page.getByRole('heading', { name: 'Заказы' })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Долгая диагностика, дней' })).toHaveValue(
		/^\d+$/
	);
	await scene(page, c, 'orders-settings', bars, pending);

	await page.goto('/settings/documents');
	await expect(page.getByRole('heading', { name: 'Документы и реквизиты' })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Условия приёма', exact: true })).toBeEnabled();
	await scene(page, c, 'documents-settings', bars, pending);

	await page.goto('/settings/print');
	await expect(page.getByRole('heading', { name: 'Печать', exact: true })).toBeVisible();
	await expect(page.getByRole('radiogroup', { name: 'Размер наклейки' })).toBeVisible();
	await scene(page, c, 'print-settings', bars, pending);

	await page.goto('/settings/dictionaries?kind=device_type');
	await expect(page.getByRole('heading', { name: 'Справочники' })).toBeVisible();
	// Loaded: the values with their counts (the archived one of the seed is there).
	await expect(page.getByRole('main').getByRole('listitem').first()).toBeVisible();
	await scene(page, c, 'dictionaries', [...bars, ...catalogRows(page)], pending);

	await page.goto('/settings/statuses');
	await expect(page.getByRole('heading', { name: 'Статусы' })).toBeVisible();
	await expect(page.getByRole('main').getByRole('listitem').first()).toBeVisible();
	await scene(
		page,
		c,
		'statuses',
		[...bars, ...catalogRows(page), page.getByRole('main').locator('.catalog-add')],
		pending
	);

	await page.goto('/settings/points');
	await expect(page.getByRole('heading', { name: 'Точки' })).toBeVisible();
	await expect(page.getByRole('main').getByRole('listitem').first()).toBeVisible();
	await scene(page, c, 'points', [...bars, ...catalogRows(page)], pending);

	await usersScenes(page, c, bars, pending);
	await rolesScenes(page, c, bars, pending);

	await page.goto('/settings/about');
	await expect(page.getByRole('heading', { name: 'О программе' })).toBeVisible();
	await expect(page.getByRole('button', { name: 'Скопировать' })).toBeVisible();
	await scene(page, c, 'about', bars, pending);

	await importScenes(page, c, bars, pending);
}

/**
 * The template editor (read only: the template is not touched). Wide and medium: the three parts at once.
 * Narrow (the segments «Блоки · Свойства · Превью»): each part in its turn.
 */
async function templateEditorScenes(
	page: Page,
	c: ScreenCase,
	bars: Locator[],
	pending: () => number
) {
	await page.goto('/templates/intake_act');
	await expect(
		page
			.getByRole('region', { name: 'Блоки' })
			.getByRole('button', { name: 'Страница', exact: true })
	).toBeVisible();
	const segments = page.getByRole('radiogroup', { name: 'Часть редактора' });
	if ((await segments.count()) === 0) {
		await expect(
			page.getByRole('region', { name: 'Превью' }).getByRole('img').first()
		).toBeVisible();
		await scene(page, c, 'template-editor', bars, pending);
		return;
	}
	await scene(page, c, 'template-editor-blocks', bars, pending);
	// The segments' radios sit under their labels, so they are clicked with force.
	await segments.getByRole('radio', { name: 'Превью' }).click({ force: true });
	await expect(page.getByRole('region', { name: 'Превью' }).getByRole('img').first()).toBeVisible();
	await scene(page, c, 'template-editor-preview', bars, pending);
	await segments.getByRole('radio', { name: 'Свойства' }).click({ force: true });
	await expect(page.getByRole('region', { name: 'Свойства' })).toBeVisible();
	await scene(page, c, 'template-editor-properties', bars, pending);
}

/** A phone after the list and the panel: «Настройки» (the list, then «Внешний вид») and the intake. */
async function phoneScenes(page: Page, c: ScreenCase, seed: Seed, pending: () => number) {
	await page.goto('/settings');
	const sections = page.getByRole('navigation', { name: 'Разделы настроек' });
	await expect(sections.getByRole('link', { name: 'Внешний вид' })).toBeVisible();
	await scene(page, c, 'settings', [banner(page), bottomNav(page)], pending);

	await sections.getByRole('link', { name: 'Внешний вид' }).click();
	await expect(page.getByRole('heading', { name: 'Внешний вид' })).toBeVisible();
	await scene(page, c, 'appearance', [banner(page), bottomNav(page)], pending);
	await serverSettingsScenes(page, c, [banner(page), bottomNav(page)], pending);

	// Intake from «Принять» of the bottom navigation.
	await page.goto('/orders');
	await expect(orderCard(page, seed.full)).toBeAttached();
	await bottomNav(page)
		.getByRole('button', { name: /Принять/ })
		.click();
	await intakeScene(page, c, seed, pending);

	await setupScenes(page, c, pending);
}

/** The thumb of a switch sits in the middle of its track: the same gap above, below and on the side. */
async function expectThumbCentred(page: Page, label: string) {
	const sw = page.getByRole('switch', { name: label });
	await expect(sw).toBeEnabled();
	for (const checked of [false, true]) {
		// The state of the input only, without an event: the setting on the server stays as it is.
		await sw.evaluate((el, on) => ((el as HTMLInputElement).checked = on), checked);
		const g = await sw.evaluate(async (el) => {
			// The thumb slides: measure it where it stops.
			await Promise.all(el.parentElement!.getAnimations({ subtree: true }).map((a) => a.finished));
			const track = el.parentElement!.querySelector('.track')!.getBoundingClientRect();
			const thumb = el.parentElement!.querySelector('.thumb')!.getBoundingClientRect();
			return {
				top: thumb.top - track.top,
				bottom: track.bottom - thumb.bottom,
				left: thumb.left - track.left,
				right: track.right - thumb.right
			};
		});
		const on = checked ? 'on' : 'off';
		expect(
			Math.abs(g.top - g.bottom),
			`${on}: above and below ${JSON.stringify(g)}`
		).toBeLessThanOrEqual(0.5);
		const side = checked ? g.right : g.left;
		expect(
			Math.abs(side - g.top),
			`${on}: side and above ${JSON.stringify(g)}`
		).toBeLessThanOrEqual(0.5);
	}
}

for (const scale of [1, 1.25]) {
	test(`the thumb of a switch is centred in its track at ${scale}`, async ({ page }) => {
		await page.addInitScript(
			(v) => localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale: v })),
			scale
		);
		await page.goto('/settings/orders');
		await expectThumbCentred(page, 'Проверять обязательные поля при приёме');
	});
}

test.describe('a touch screen', () => {
	test.use({ hasTouch: true, isMobile: true });
	test('the thumb of a switch is centred in its track', async ({ page }) => {
		await page.goto('/settings/orders');
		await expectThumbCentred(page, 'Проверять обязательные поля при приёме');
	});
});

/** The search of the top bar: in the middle of the window when there is room, never over its neighbours. */
async function expectSearchPlace(page: Page, c: ScreenCase) {
	const r = await page.getByRole('banner').evaluate((bar) => {
		const rect = (el: Element | null) => el!.getBoundingClientRect();
		const search = rect(bar.querySelector('.find'));
		const title = rect(bar.querySelector('.title'));
		const actions = rect(bar.querySelector('.actions'));
		return {
			centre: (search.left + search.right) / 2,
			window: window.innerWidth / 2,
			overTitle: search.left < title.right - 0.5,
			overActions: search.right > actions.left + 0.5
		};
	});
	expect(r.overTitle, 'the search runs over the title').toBe(false);
	expect(r.overActions, 'the search runs over the actions').toBe(false);
	if (['md', 'lg', 'xl'].includes(c.band))
		expect(
			Math.abs(r.centre - r.window),
			'the search is in the middle of the window'
		).toBeLessThanOrEqual(1);
}

for (const c of SCREEN_CASES) {
	test.describe(c.name, () => {
		test.use({
			viewport: c.viewport,
			screen: c.screen,
			deviceScaleFactor: c.dpr,
			...(c.touch ? { hasTouch: true, isMobile: true } : {})
		});

		test('scenes', async ({ page, request }) => {
			test.setTimeout(90_000);
			const seed = await seedOnce(request);
			// Before the program starts: the manual size of the case and the point of this device.
			await page.addInitScript(
				({ scale, point }) => {
					if (scale !== null) localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale }));
					localStorage.setItem('mw.point', String(point));
				},
				{ scale: c.scale ?? null, point: seed.point }
			);
			const pending = watchRequests(page);
			const grid = page.getByRole('grid', { name: 'Заказы' });
			const filter = page.getByRole('searchbox', { name: 'Фильтр списка' });
			const listBars = () => [
				banner(page),
				page.getByRole('group', { name: 'Быстрые фильтры' }),
				rowWith(page, filter, page.getByRole('button', { name: 'Фильтры', exact: true })),
				...(c.phone ? [bottomNav(page)] : [])
			];

			// «Заказы»: the whole list, more rows than fit (on a phone, cards).
			await page.goto('/orders');
			if (c.phone)
				await expect(
					page.getByRole('main').getByRole('button', { name: /^№ \d/ }).nth(5)
				).toBeVisible();
			else await expect(grid.getByRole('row').nth(12)).toBeVisible();
			await scene(page, c, 'orders', listBars(), pending);
			await expectSearchPlace(page, c);
			// A finger cannot hover: the card's whole title is on the screen, not only in its hint.
			if (c.touch) {
				const title = page.getByRole('main').locator('.card .title').first();
				await expect(title).toBeVisible();
				const cut = await title.evaluate((el) => el.scrollWidth > el.clientWidth + 1);
				expect(cut, 'the title of the card is cut on a touch screen').toBe(false);
				// Nor has it keys: no shortcut hints (the top bar's search shows its whole label).
				await expect(page.locator('kbd:visible')).toHaveCount(0);
			}

			// The order panel over the list (on a phone, the whole screen).
			const panel = await openOrderPanel(page, c, seed, seed.full);
			await scene(
				page,
				c,
				'panel',
				[...(c.phone ? [] : [banner(page)]), ...panelBars(panel)],
				pending
			);

			if (c.phone) {
				await phoneScenes(page, c, seed, pending);
				return;
			}

			// The full card.
			await page.goto(`/orders/${seed.full}`);
			const card = page.getByRole('main');
			await expect(card.getByRole('heading', { name: `№ ${seed.full}`, level: 1 })).toBeVisible();
			await expect(card.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
			await scene(
				page,
				c,
				'card',
				[
					banner(page),
					card
						.locator('div')
						.filter({ has: page.getByRole('heading', { level: 1 }) })
						.last()
				],
				pending
			);

			// Intake (N) with the client found by the phone and chosen.
			await page.goto('/orders');
			await expect(grid).toBeVisible();
			await page.keyboard.press('n');
			await intakeScene(page, c, seed, pending);

			// «Клиенты» with the client's panel.
			await page.goto(`/clients?client=${seed.client.id}`);
			const clientPanel = page.getByRole('dialog', { name: seed.client.full_name });
			await expect(clientPanel.getByRole('textbox', { name: 'ФИО' })).toHaveValue(
				seed.client.full_name
			);
			await scene(
				page,
				c,
				'clients',
				[
					banner(page),
					...panelBars(clientPanel),
					rowWith(page, filter, page.getByRole('group', { name: 'Быстрые фильтры' }))
				],
				pending
			);

			// «Настройки → Внешний вид».
			await page.goto('/settings/appearance');
			await expect(page.getByRole('heading', { name: 'Внешний вид' })).toBeVisible();
			await scene(page, c, 'appearance', [banner(page)], pending);
			await serverSettingsScenes(page, c, [banner(page)], pending);
			await templateEditorScenes(page, c, [banner(page)], pending);

			// «Выдать» over the panel of a ready order.
			const readyPanel = await openOrderPanel(page, c, seed, seed.ready);
			await readyPanel.getByRole('button', { name: /^Выдать/ }).click();
			const issue = page.getByRole('dialog', { name: `Выдача заказа № ${seed.ready}` });
			await expect(issue.getByRole('textbox', { name: /^Итоговая цена/ })).toBeVisible();
			await scene(page, c, 'issue', dialogBars(issue), pending);
			await page.keyboard.press('Escape');
			await expect(issue).toBeHidden();

			// Ctrl K with the orders found.
			await page.goto('/orders');
			await expect(grid).toBeVisible();
			await page.keyboard.press(`${await mod(page)}+KeyK`);
			const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
			await palette.getByRole('combobox').fill(seed.marker);
			await expect(palette.getByRole('option').nth(3)).toBeVisible();
			await scene(page, c, 'palette', [], pending);
			await page.keyboard.press('Escape');

			await setupScenes(page, c, pending);
		});

		test.describe('signed in', () => {
			// The server of 4174 has an administrator: it asks to sign in, and the top bar then carries the
			// user's menu as well as the point (the widest top bar there is).
			test.use({ baseURL: 'http://127.0.0.1:4174' });

			test('login and the top bar', async ({ page, request }) => {
				if (c.scale !== undefined)
					await page.addInitScript(
						(scale) => localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale })),
						c.scale
					);
				const pending = watchRequests(page);
				await page.goto('/login');
				await expect(page.getByRole('button', { name: 'Войти' })).toBeVisible();
				if (c.name === '720p' || c.name === '4k' || c.phone)
					await scene(page, c, 'login', [], pending);

				await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
				await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
				await page.getByRole('button', { name: 'Войти' }).click();
				// Signed in: the user's menu in the top bar; a phone keeps it in «Ещё».
				if (c.phone) await expect(bottomNav(page)).toBeVisible();
				else await expect(page.getByRole('button', { name: /Администратор/ })).toBeVisible();
				const token = await sessionToken(page);
				const created = await request.post('/api/v1/service-points', {
					data: { name: `ТЦ «Галерея», 2 этаж, ${uniq('Обход')}` },
					headers: { Authorization: `Bearer ${token}` }
				});
				expect(created.ok(), await created.text()).toBe(true);
				const point = ((await created.json()) as ServicePoint).id;
				await page.evaluate((id) => localStorage.setItem('mw.point', String(id)), point);
				await page.goto('/orders');
				// A phone keeps the point and the user's menu in «Ещё»: its top bar is the same signed in.
				if (c.phone) {
					await expect(bottomNav(page).getByRole('button', { name: /Ещё/ })).toBeVisible();
				} else {
					await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
					await expect(page.getByRole('button', { name: /^Точка/ })).toBeVisible();
				}
				await scene(
					page,
					c,
					'signed-in',
					[
						banner(page),
						page.getByRole('group', { name: 'Быстрые фильтры' }),
						rowWith(
							page,
							page.getByRole('searchbox', { name: 'Фильтр списка' }),
							page.getByRole('button', { name: 'Фильтры', exact: true })
						),
						...(c.phone ? [bottomNav(page)] : [])
					],
					pending
				);
			});
		});
	});
}
