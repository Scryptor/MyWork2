import { expect, test, type APIRequestContext, type Page, type Route } from '@playwright/test';
import type { Order } from '$api/Order';
import { createOrder, patchOrder, statusId, uniq } from './support/api';

// The order panel on the server of 4173 (open mode: every permission). Each test makes its own orders
// with a unique model and opens them from the list narrowed to that model.

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, id: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
const statusesOf = (page: Page, id: number) =>
	panelOf(page, id).getByRole('radiogroup', { name: 'Статус' });

async function fetchOrder(request: APIRequestContext, id: number): Promise<Order> {
	const response = await request.get(`/api/v1/orders/${id}`);
	expect(response.ok()).toBe(true);
	return (await response.json()) as Order;
}

/** Opens the list narrowed to the marker (all statuses) and the order's panel from its row. */
async function openPanel(page: Page, marker: string, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	await rowOf(page, id).getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, id)).toBeVisible();
	// The fields are there once the order has come from the server.
	await expect(panelOf(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
}

/** PATCH requests of one order. */
const patchesOf = (id: number) => (url: URL) => url.pathname === `/api/v1/orders/${id}`;
const isPatch = (route: Route) => route.request().method() === 'PATCH';

test('edits save by themselves and survive a reload', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m, malfunction: 'Не включается' });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	const field = panel.getByRole('textbox', { name: 'Неисправность' });
	await expect(field).toHaveValue('Не включается');
	await field.fill('Не включается!');
	await expect(panel.getByText('Сохраняю…')).toBeVisible();
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	expect((await fetchOrder(request, o.id)).malfunction).toBe('Не включается!');

	await page.reload();
	await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, o.id).getByRole('textbox', { name: 'Неисправность' })).toHaveValue(
		'Не включается!'
	);
});

test('typing during a slow save keeps the text', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	const field = panel.getByRole('textbox', { name: 'Выполненные работы' });

	// The first save takes 1.5 s, the next one 1 s: the panel gets the old answer while typing went on.
	const delays = [1500, 1000];
	const patches = patchesOf(o.id);
	await page.route(patches, async (route) => {
		if (isPatch(route)) await new Promise((r) => setTimeout(r, delays.shift() ?? 0));
		await route.continue();
	});
	const firstAnswer = page.waitForResponse(
		(r) => r.request().method() === 'PATCH' && patches(new URL(r.url()))
	);
	await field.click();
	await page.keyboard.type('не за');
	await page.waitForTimeout(700);
	await page.keyboard.type('ряжается');
	await firstAnswer;
	await page.waitForTimeout(200);
	// The answer about «не за» has come; the field keeps what was typed after it.
	expect(await field.inputValue()).toBe('не заряжается');
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	await expect(field).toHaveValue('не заряжается');
	expect((await fetchOrder(request, o.id)).work_done).toBe('не заряжается');
});

test('offline save shows retry and retry saves', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	const field = panel.getByRole('textbox', { name: 'Выполненные работы' });

	const patches = patchesOf(o.id);
	await page.route(patches, (route) =>
		isPatch(route) ? route.abort('internetdisconnected') : route.continue()
	);
	await field.fill('Замена разъёма');
	await expect(panel.getByText('Не сохранено')).toBeVisible();
	await expect(field).toHaveValue('Замена разъёма');

	// Closing tries once more; it fails, so the panel stays with the text.
	await page.keyboard.press('Escape');
	await expect(panel.getByText('Не сохранено')).toBeVisible();
	await expect(panel).toBeVisible();
	await expect(field).toHaveValue('Замена разъёма');

	await page.unroute(patches);
	await panel.getByRole('button', { name: 'Повторить' }).click();
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	expect((await fetchOrder(request, o.id)).work_done).toBe('Замена разъёма');
});

test('conflict callout lets me take the new value', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m, malfunction: 'Не включается' });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	const field = panel.getByRole('textbox', { name: 'Неисправность' });
	const comment = panel.getByRole('textbox', { name: 'Комментарий (только для сервиса)' });

	// Another computer changes the same field and another one.
	await patchOrder(request, o.id, {
		malfunction: 'Не включается после падения',
		internal_comment: 'Клиент торопится'
	});
	await field.fill('Не включается, разбит экран');
	const callout = panel.getByText('Заказ изменили на другом компьютере');
	await expect(callout).toBeVisible();
	// What only the other computer changed is taken at once.
	await expect(comment).toHaveValue('Клиент торопится');
	await expect(field).toHaveValue('Не включается, разбит экран');

	await panel.getByRole('button', { name: 'Взять новое' }).click();
	await expect(field).toHaveValue('Не включается после падения');
	await expect(callout).toBeHidden();
	await expect(panel.getByText('Не сохранено')).toHaveCount(0);
	expect((await fetchOrder(request, o.id)).malfunction).toBe('Не включается после падения');

	// «Оставить моё» sends own text over the new version.
	await patchOrder(request, o.id, { malfunction: 'Снова чужое' });
	await field.fill('Моё окончательное');
	await expect(callout).toBeVisible();
	await panel.getByRole('button', { name: 'Оставить моё' }).click();
	await expect(callout).toBeHidden();
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	expect((await fetchOrder(request, o.id)).malfunction).toBe('Моё окончательное');
});

test('status change shows a toast and undo restores it', async ({ page, request }) => {
	const m = uniq('M');
	const diag = await statusId(request, 'Диагностика');
	const ready = await statusId(request, 'Готов');
	const o = await createOrder(request, { model: m, status_id: diag });
	await openPanel(page, m, o.id);
	const statuses = statusesOf(page, o.id);
	await expect(statuses.getByRole('radio', { name: 'Диагностика' })).toBeChecked();

	await statuses.getByRole('radio', { name: 'Готов' }).check();
	await expect(toastsOf(page)).toContainText(`№ ${o.id} → Готов`);
	await expect(statuses.getByRole('radio', { name: 'Готов' })).toBeChecked();
	await expect(rowOf(page, o.id)).toContainText('Готов');
	expect((await fetchOrder(request, o.id)).status_id).toBe(ready);

	await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
	await expect(statuses.getByRole('radio', { name: 'Диагностика' })).toBeChecked();
	await expect(rowOf(page, o.id)).toContainText('Диагностика');
	expect((await fetchOrder(request, o.id)).status_id).toBe(diag);
});

test('Alt+↓ moves to the next order and money typed does not leak into it', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const older = await createOrder(request, { model: m });
	const newer = await createOrder(request, { model: m });
	await patchOrder(request, older.id, { final_price: 120000 });
	// Newest first: the older order is next after the newer one.
	await openPanel(page, m, newer.id);
	const total = panelOf(page, newer.id).getByRole('textbox', { name: 'Итог' });
	await total.click();
	await page.keyboard.type('7777');
	await page.keyboard.press('Alt+ArrowDown');

	await expect(panelOf(page, older.id)).toBeVisible();
	const nextTotal = panelOf(page, older.id).getByRole('textbox', { name: 'Итог' });
	await expect(nextTotal).toBeFocused();
	await expect(nextTotal).toHaveValue('1200');
	await page.keyboard.press('Tab');
	await expect(nextTotal).toHaveValue(/^1\s200$/);
	expect((await fetchOrder(request, newer.id)).final_price).toBe(777700);
	expect((await fetchOrder(request, older.id)).final_price).toBe(120000);
	await expect(rowOf(page, newer.id)).toContainText(/7\s777\s₽/);

	// Alt+↑ goes back and shows what was typed there.
	await page.keyboard.press('Alt+ArrowUp');
	await expect(panelOf(page, newer.id).getByRole('textbox', { name: 'Итог' })).toHaveValue(
		/^7\s777$/
	);
});

test('issue fills required fields in the dialog, issues and undo brings it back', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const ready = await statusId(request, 'Готов');
	const issued = await statusId(request, 'Выдан');
	const o = await createOrder(request, { model: m, status_id: ready, prepayment: 50000 });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	await panel.getByRole('button', { name: /^Выдать/ }).click();

	const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${o.id}` });
	await expect(dialog).toBeVisible();
	const work = dialog.getByRole('textbox', { name: /^Выполненные работы/ });
	await expect(work).toHaveAttribute('aria-invalid', 'true');
	await expect(dialog.getByRole('combobox', { name: 'Гарантия' })).toHaveValue('30 дней');
	// Not everything is filled: Ctrl/⌘+Enter does not issue.
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(dialog).toBeVisible();

	await work.fill('Замена аккумулятора');
	await dialog.getByRole('combobox', { name: /^Мастер/ }).fill('Тестов');
	const price = dialog.getByRole('textbox', { name: /^Итоговая цена/ });
	await price.click();
	await price.fill('3500');
	await expect(dialog.getByText(/^К оплате/)).toHaveText(/^К оплате\s3\s000\s₽$/);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(dialog).toBeHidden();
	await expect(toastsOf(page)).toContainText(`№ ${o.id} → Выдан`);
	await expect(panel.getByRole('button', { name: 'Ещё: Выдан' })).toBeVisible();
	await expect(panel.getByRole('button', { name: /^Выдать/ })).toHaveCount(0);
	const after = await fetchOrder(request, o.id);
	expect(after.status_id).toBe(issued);
	expect(after.issued_at).not.toBeNull();
	expect(after.work_done).toBe('Замена аккумулятора');
	expect(after.master).toBe('Тестов');
	expect(after.final_price).toBe(350000);
	expect(after.warranty).toBe('30 дней');

	await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
	await expect(statusesOf(page, o.id).getByRole('radio', { name: 'Готов' })).toBeChecked();
	await expect.poll(async () => (await fetchOrder(request, o.id)).issued_at).toBeNull();
	expect((await fetchOrder(request, o.id)).status_id).toBe(ready);
});

test('an issued order goes back to work with the status it had before', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const ready = await statusId(request, 'Готов');
	const o = await createOrder(request, { model: m, status_id: ready, master: 'Тестов' });
	const filled = await patchOrder(request, o.id, { work_done: 'Чистка' });
	const issuedResponse = await request.post(
		`/api/v1/orders/${o.id}/issue?version=${filled.version}`,
		{ headers: { 'Content-Type': 'application/json' } }
	);
	expect(issuedResponse.ok(), await issuedResponse.text()).toBe(true);

	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	await expect(statusesOf(page, o.id)).toHaveAttribute('aria-disabled', 'true');
	await panel.getByRole('button', { name: 'Ещё действия' }).click();
	await page.getByRole('menuitem', { name: 'Вернуть в работу…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Вернуть в работу' });
	await expect(dialog.getByRole('combobox', { name: 'Статус' })).toHaveText('Готов');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(dialog).toBeHidden();
	await expect(toastsOf(page)).toContainText(`№ ${o.id} → Готов`);
	await expect(statusesOf(page, o.id).getByRole('radio', { name: 'Готов' })).toBeChecked();
	expect((await fetchOrder(request, o.id)).status_id).toBe(ready);
});

test('Esc closes an open menu first, then the panel', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	await panel.getByRole('button', { name: 'Ещё действия' }).click();
	const menu = page.getByRole('menu');
	await expect(menu).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(menu).toBeHidden();
	await expect(panel).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(panel).toBeHidden();
});

test.describe('on a phone', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('phone panel calls and changes status', async ({ page, request }) => {
		const m = uniq('M');
		const diag = await statusId(request, 'Диагностика');
		const o = await createOrder(request, { model: m, status_id: diag, phone: '+79005550101' });
		await page.goto(`/orders?q=${encodeURIComponent(m)}`);
		await page.getByRole('button', { name: new RegExp(`№ ${o.id}(?!\\d)`) }).click();
		const panel = panelOf(page, o.id);
		await expect(panel).toBeVisible();
		await expect.poll(() => panel.boundingBox()).toEqual({ x: 0, y: 0, width: 390, height: 844 });
		await expect(panel.getByRole('button', { name: 'Назад' })).toBeVisible();

		const call = panel.getByRole('link', { name: /Позвонить/ });
		await expect(call).toHaveAttribute('href', 'tel:+79005550101');
		await expect(panel.getByRole('link', { name: '+7 900 555-01-01' })).toHaveAttribute(
			'href',
			'tel:+79005550101'
		);

		const button = panel.getByRole('button', { name: /^Статус/ });
		await expect(button).toContainText('Диагностика');
		await button.click();
		const list = page.getByRole('dialog', { name: 'Статус' });
		await expect(list).toBeVisible();
		await list.getByRole('button', { name: 'Готов', exact: true }).click();
		await expect(list).toBeHidden();
		await expect(button).toContainText('Готов');
		await expect(toastsOf(page)).toContainText(`№ ${o.id} → Готов`);
		expect((await fetchOrder(request, o.id)).status_id).toBe(await statusId(request, 'Готов'));
	});
});
