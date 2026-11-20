import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import type { User } from '$api/User';
import { createOrder, fetchOrder, masterId, uniq } from './support/api';

// The master of an order is a user marked «Мастер» (plan 5.8): the card, the filter, a disabled master.
// The server of 4173 runs in open mode (every permission). Each test makes its own masters and orders.

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, id: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });

async function openPanel(page: Page, marker: string, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	await rowOf(page, id).getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, id)).toBeVisible();
	await expect(panelOf(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
}

/** The master is disabled: a user who left; his old orders keep him. */
async function disable(request: APIRequestContext, id: number) {
	const users = (await (await request.get('/api/v1/users')).json()) as User[];
	const user = users.find((u) => u.id === id)!;
	const r = await request.patch(`/api/v1/users/${id}`, {
		params: { version: user.version },
		data: { active: false }
	});
	expect(r.ok()).toBe(true);
}

test('the master is changed in the card from the list of masters', async ({ page, request }) => {
	const m = uniq('M');
	const first = uniq('Мастер');
	const second = uniq('Мастер');
	await masterId(request, first);
	await masterId(request, second);
	const o = await createOrder(request, { model: m, master: first });
	await openPanel(page, m, o.id);
	const field = panelOf(page, o.id).getByRole('combobox', { name: 'Мастер' });
	await expect(field).toContainText(first);
	await field.click();
	await page.getByRole('option', { name: second, exact: true }).click();
	await expect.poll(async () => (await fetchOrder(request, o.id)).master).toBe(second);
});

test('a disabled master stays in his order with a mark and is not offered', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const gone = uniq('Уволен');
	const other = uniq('Мастер');
	const goneId = await masterId(request, gone);
	await masterId(request, other);
	const o = await createOrder(request, { model: m, master: gone });
	await disable(request, goneId);
	await openPanel(page, m, o.id);
	const field = panelOf(page, o.id).getByRole('combobox', { name: 'Мастер' });
	await expect(field).toContainText(`${gone} (отключён)`);
	await field.click();
	await expect(page.getByRole('option', { name: other, exact: true })).toBeVisible();
	await page.getByRole('option', { name: `${gone} (отключён)` }).click({ force: true });
	await expect.poll(async () => (await fetchOrder(request, o.id)).master).toBe(gone);
});

test('the filter «Мастер» picks the orders of one master user', async ({ page, request }) => {
	const m = uniq('M');
	const mine = uniq('Мастер');
	const theirs = uniq('Мастер');
	await masterId(request, mine);
	await masterId(request, theirs);
	const a = await createOrder(request, { model: m, master: mine });
	const b = await createOrder(request, { model: m, master: theirs });
	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await expect(rowOf(page, a.id)).toBeVisible();
	await expect(rowOf(page, b.id)).toBeVisible();

	await page.getByRole('button', { name: /^Фильтры/ }).click();
	const menu = page.getByRole('dialog', { name: 'Фильтры' });
	await menu.getByRole('combobox', { name: 'Мастер' }).click();
	await page.getByRole('option', { name: mine, exact: true }).click();
	await menu.getByRole('button', { name: 'Применить' }).click();
	await expect(rowOf(page, a.id)).toBeVisible();
	await expect(rowOf(page, b.id)).toHaveCount(0);
	await expect(page.getByText(`Мастер: ${mine}`, { exact: true })).toBeVisible();
	await expect(page).toHaveURL(/master_id=\d+/);

	await page.getByRole('button', { name: `Убрать «Мастер: ${mine}»` }).click();
	await expect(rowOf(page, b.id)).toBeVisible();
});

test('a saved address with the master as text still filters', async ({ page, request }) => {
	const m = uniq('M');
	const mine = uniq('Мастер');
	const theirs = uniq('Мастер');
	await masterId(request, mine);
	await masterId(request, theirs);
	const a = await createOrder(request, { model: m, master: mine });
	const b = await createOrder(request, { model: m, master: theirs });
	await page.goto(
		`/orders?q=${encodeURIComponent(m)}&chips=all&master=${encodeURIComponent(mine)}`
	);
	await expect(rowOf(page, a.id)).toBeVisible();
	await expect(rowOf(page, b.id)).toHaveCount(0);
	await expect(page.getByText(`Мастер: ${mine}`, { exact: true })).toBeVisible();
});
