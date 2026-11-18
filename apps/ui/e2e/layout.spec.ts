import { expect, test, type APIRequestContext, type Locator, type Page } from '@playwright/test';
import type { Client } from '$api/Client';
import type { ServicePoint } from '$api/ServicePoint';
import {
	createClient,
	createOrder,
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	patchOrder,
	statusId,
	uniq,
	uniqPhone
} from './support/api';
import { expectTidyLayout, SCREEN_CASES, type ScreenCase } from './support/layout';

// Every screen case (spec 5.3) goes through the main scenes on one page and checks the geometry; each
// scene leaves a screenshot `layout-<case>-<scene>.png` for a person to look at (not compared with a
// reference: fonts draw differently on every OS, decision 50). Server of 4173: open mode, every right.

test.describe.configure({ mode: 'parallel' });

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
	return { marker, point, full: full.id, ready: issuable.id, client };
}

// Once per worker: the scenes only look at these orders.
let seeded: Promise<Seed> | undefined;

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
		await expectTidyLayout(page, bars, { rootPx: c.rootPx });
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

async function openOrderPanel(page: Page, seed: Seed, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(seed.marker)}&chips=all`);
	await orderRow(page, id).getByRole('gridcell').nth(3).click();
	await expect(orderPanel(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	return orderPanel(page, id);
}

for (const c of SCREEN_CASES) {
	test.describe(c.name, () => {
		test.use({ viewport: c.viewport, screen: c.screen, deviceScaleFactor: c.dpr });

		test('scenes', async ({ page, request }) => {
			test.setTimeout(90_000);
			seeded ??= makeSeed(request);
			const seed = await seeded;
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

			// «Заказы»: the whole list, more rows than fit.
			await page.goto('/orders');
			await expect(grid.getByRole('row').nth(12)).toBeVisible();
			const filter = page.getByRole('searchbox', { name: 'Фильтр списка' });
			await scene(
				page,
				c,
				'orders',
				[
					banner(page),
					page.getByRole('group', { name: 'Быстрые фильтры' }),
					rowWith(page, filter, page.getByRole('button', { name: 'Фильтры' }))
				],
				pending
			);

			// The order panel over the list.
			const panel = await openOrderPanel(page, seed, seed.full);
			await scene(page, c, 'panel', [banner(page), ...panelBars(panel)], pending);

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

			// «Выдать» over the panel of a ready order.
			const readyPanel = await openOrderPanel(page, seed, seed.ready);
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
				if (c.name === '720p' || c.name === '4k') await scene(page, c, 'login', [], pending);

				await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
				await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
				await page.getByRole('button', { name: 'Войти' }).click();
				await expect(page.getByRole('button', { name: /Администратор/ })).toBeVisible();
				const token = await page.evaluate(() => localStorage.getItem('mw.session'));
				const created = await request.post('/api/v1/service-points', {
					data: { name: `ТЦ «Галерея», 2 этаж, ${uniq('Обход')}` },
					headers: { Authorization: `Bearer ${token}` }
				});
				expect(created.ok(), await created.text()).toBe(true);
				const point = ((await created.json()) as ServicePoint).id;
				await page.evaluate((id) => localStorage.setItem('mw.point', String(id)), point);
				await page.goto('/orders');
				await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
				await expect(page.getByRole('button', { name: /^Точка/ })).toBeVisible();
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
							page.getByRole('button', { name: 'Фильтры' })
						)
					],
					pending
				);
			});
		});
	});
}
