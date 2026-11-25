import { statSync } from 'node:fs';
import {
	expect,
	test as base,
	type APIRequestContext,
	type Page,
	type PlaywrightWorkerArgs
} from '@playwright/test';
import type { OrderReport } from '$api/OrderReport';
import {
	createOrder,
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	patchOrder,
	sessionToken,
	statusId,
	uniq
} from './support/api';
import { downloadName } from './support/download';
import { adminOrigin } from './support/fresh';

// «Отчёты»: the orders A, Б, В of `report_sums_issued_orders` (crates/mw-core/tests/reports.rs) made through
// the API under a brand of their own, then the screen: the tiles, the table, the grouping, the address and
// the file of Excel. A server of this spec's own per engine: «Обязательно при выдаче» is relaxed on it so
// that Б is issued without a master, and no other spec shares the setting.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(adminOrigin('reports', browserName))
});
test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

async function signIn(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

async function issue(api: APIRequestContext, id: number, version: number) {
	const issued = await api.post(`/api/v1/orders/${id}/issue?version=${version}`, {
		headers: { 'Content-Type': 'application/json' }
	});
	expect(issued.ok(), await issued.text()).toBe(true);
}

/** А: a master, a part of 2 × 50 ₽ and costs of 200 ₽, 1 000 ₽ with 100 ₽ off; Б: «Без ремонта», no master, 500 ₽; В: not issued. */
async function makeOrders(api: APIRequestContext, brand: string, master: string) {
	const ready = await statusId(api, 'Готов');
	const relaxed = await api.patch('/api/v1/settings', { data: { issue_required: [] } });
	expect(relaxed.ok(), await relaxed.text()).toBe(true);

	const item = await api.post('/api/v1/stock', {
		data: {
			name: `Дисплей ${uniq('Д')}`,
			device_type: '',
			part_type: '',
			brand: '',
			model: '',
			color: '',
			note: '',
			quantity: 10,
			price: 5_000,
			restock_threshold: 0
		}
	});
	expect(item.ok(), await item.text()).toBe(true);
	const stockId = ((await item.json()) as { id: number }).id;

	const a = await createOrder(api, { brand, master, status_id: ready });
	const part = await api.post(`/api/v1/orders/${a.id}/parts`, {
		data: { stock_item_id: stockId, quantity: 2, request_key: uniq('k') }
	});
	expect(part.ok(), await part.text()).toBe(true);
	const filledA = await patchOrder(api, a.id, {
		final_price: 100_000,
		discount: 10_000,
		extra_costs: 20_000,
		work_done: 'Замена дисплея'
	});
	await issue(api, a.id, filledA.version);

	const b = await createOrder(api, { brand, status_id: ready });
	const filledB = await patchOrder(api, b.id, { final_price: 50_000, work_done: 'Без ремонта' });
	await issue(api, b.id, filledB.version);

	const c = await createOrder(api, { brand, status_id: ready });
	await patchOrder(api, c.id, { final_price: 70_000 });
}

/** A sum of kopecks as the screen writes it («1 400 ₽»); the spaces are any of the white ones. */
function money(kopecks: number): RegExp {
	const rub = Math.floor(Math.abs(kopecks) / 100);
	const frac = Math.abs(kopecks) % 100;
	const whole = String(rub).replace(/\B(?=(\d{3})+(?!\d))/g, '\\s');
	return new RegExp(
		`^${kopecks < 0 ? '−' : ''}${whole}${frac ? `,${String(frac).padStart(2, '0')}` : ''}\\s₽$`
	);
}

async function withOrders(
	page: Page,
	playwright: PlaywrightWorkerArgs['playwright'],
	run: (api: APIRequestContext, brand: string, master: string) => Promise<void>
) {
	await signIn(page);
	const token = await sessionToken(page);
	const api = await playwright.request.newContext({
		baseURL: adminOrigin('reports', test.info().project.name),
		extraHTTPHeaders: token ? { Authorization: `Bearer ${token}` } : {}
	});
	try {
		const brand = uniq('Бренд');
		const master = uniq('Мастер');
		await makeOrders(api, brand, master);
		await run(api, brand, master);
	} finally {
		await api.dispose();
	}
}

test('the report counts the issued orders: tiles, table, grouping, address and file', async ({
	page,
	playwright
}) => {
	await withOrders(page, playwright, async (api, brand, master) => {
		await page.goto('/orders');
		await page
			.getByRole('navigation', { name: 'Разделы' })
			.getByRole('link', { name: /^Отчёты/ })
			.click();
		await expect(page).toHaveURL(/\/reports/);

		await page.getByRole('radio', { name: 'Всё время' }).click();
		await page.getByRole('combobox', { name: /^Бренд/ }).fill(brand);
		await page.getByRole('combobox', { name: /^Бренд/ }).press('Enter');
		await expect(page).toHaveURL(/period=all/);
		await expect(page).toHaveURL(/b=/);

		// The numbers the server counted (the master's percent is the setting of the base).
		const sums = (await (
			await api.get(`/api/v1/reports/orders?brand=${encodeURIComponent(brand)}`)
		).json()) as OrderReport;
		expect(sums.totals.orders).toBe(2);

		const tiles = page.getByRole('group', { name: 'Итоги' });
		const tile = (name: string) => tiles.getByText(name, { exact: true }).locator('xpath=..');
		await expect(tile('Заказов')).toContainText('2');
		await expect(tile('Заказов')).toContainText('без ремонта: 1');
		await expect(tile('Выручка').locator('.value')).toHaveText(money(140_000));
		await expect(tile('Выручка')).toContainText('скидки: 100 ₽');
		await expect(tile('Затраты').locator('.value')).toHaveText(money(30_000));
		await expect(tile('Прибыль').locator('.value')).toHaveText(money(110_000));
		await expect(tile('Оплата мастерам').locator('.value')).toHaveText(
			money(sums.totals.master_pay)
		);
		await expect(tile('Оплата мастерам')).toContainText(`${sums.master_percent} % прибыли`);

		const table = page.getByRole('table', { name: 'Итоги по группам' });
		await expect(table.getByRole('row')).toHaveCount(4);
		await expect(table.getByRole('row', { name: new RegExp(master) })).toBeVisible();
		await expect(table.getByRole('row', { name: /Без мастера/ })).toBeVisible();
		const total = table.getByRole('row', { name: /Итого/ });
		await expect(total).toBeVisible();
		await expect(total.getByRole('cell').nth(0)).toHaveText('2');

		await page.getByRole('combobox', { name: 'Группировать' }).click();
		await page.getByRole('option', { name: 'По брендам' }).click();
		await expect(page).toHaveURL(/by=brand/);
		await expect(table.getByRole('row')).toHaveCount(3);
		await expect(table.getByRole('row', { name: new RegExp(brand) })).toBeVisible();

		// The address holds the whole view: a reload shows the same.
		await expect(page).toHaveURL(/period=all/);
		await page.reload();
		await expect(page.getByRole('radio', { name: 'Всё время' })).toBeChecked();
		await expect(page.getByRole('combobox', { name: /^Бренд/ })).toHaveValue(brand);
		await expect(page.getByRole('combobox', { name: 'Группировать' })).toContainText('По брендам');
		await expect(
			page
				.getByRole('table', { name: 'Итоги по группам' })
				.getByRole('row', { name: new RegExp(brand) })
		).toBeVisible();

		const [download] = await Promise.all([
			page.waitForEvent('download'),
			page.getByRole('button', { name: 'В Excel' }).click()
		]);
		expect(downloadName(download)).toBe('Отчёт по заказам за всё время.xlsx');
		const path = await download.path();
		expect(statSync(path).size).toBeGreaterThan(0);
	});
});

test('«Свой период» starts from the days of the period before it, and empty after «Всё время»', async ({
	page
}) => {
	await signIn(page);
	await page.goto('/reports');
	await page.getByRole('radio', { name: 'Свой период' }).click();
	await expect(page).toHaveURL(/period=custom&from=\d{4}-\d{2}-01&to=\d{4}-\d{2}-\d{2}/);
	await expect(page.getByLabel('С', { exact: true })).toHaveValue(/^01\.\d{2}\.\d{4}$/);
	await expect(page.getByLabel('По', { exact: true })).not.toHaveValue('');

	await page.getByRole('radio', { name: 'Всё время' }).click();
	await page.getByRole('radio', { name: 'Свой период' }).click();
	await expect(page).toHaveURL(/period=custom$/);
	await expect(page.getByLabel('С', { exact: true })).toHaveValue('');
	await expect(page.getByLabel('По', { exact: true })).toHaveValue('');
});

test('the transfers tab counts a move between two points and opens the order', async ({
	page,
	playwright
}) => {
	await signIn(page);
	const token = await sessionToken(page);
	const api = await playwright.request.newContext({
		baseURL: adminOrigin('reports', test.info().project.name),
		extraHTTPHeaders: token ? { Authorization: `Bearer ${token}` } : {}
	});
	try {
		const point = async (name: string) => {
			const made = await api.post('/api/v1/service-points', { data: { name } });
			expect(made.ok(), await made.text()).toBe(true);
			return ((await made.json()) as { id: number }).id;
		};
		const first = uniq('Точка');
		const second = uniq('Точка');
		const firstId = await point(first);
		const secondId = await point(second);
		const order = await createOrder(api, { service_point_id: firstId });
		await patchOrder(api, order.id, { service_point_id: secondId });

		// A whole load: the points made above are in the catalogs.
		await page.goto('/reports');
		await page.getByRole('tab', { name: 'Перемещения' }).click();
		await expect(page).toHaveURL(/tab=moves/);
		await page.getByRole('radio', { name: 'Всё время' }).click();
		await page.getByRole('combobox', { name: 'Точка', exact: true }).click();
		await page.getByRole('option', { name: first }).click();
		await expect(page).toHaveURL(/period=all/);
		await expect(page).toHaveURL(/p=/);

		// The orders' own filters are not on this tab.
		await expect(page.getByRole('combobox', { name: /^Бренд/ })).toHaveCount(0);

		const routes = page.getByRole('table', { name: 'Маршруты' });
		const route = routes.getByRole('row', { name: new RegExp(`${first} → ${second}`) });
		await expect(route).toBeVisible();
		await expect(route.getByRole('cell')).toHaveText('1');

		const list = page.getByRole('table', { name: 'Перемещения' });
		await list.getByRole('button', { name: `№ ${order.id}` }).click();
		await expect(
			page.getByRole('dialog', { name: new RegExp(`^№ ${order.id}(?!\\d)`) })
		).toBeVisible();

		// The tab stays in the address while the panel is open.
		await expect(page).toHaveURL(/tab=moves/);
	} finally {
		await api.dispose();
	}
});
