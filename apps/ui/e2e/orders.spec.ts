import { expect, test, type Page } from '@playwright/test';
import { createOrder, statusId, uniq } from './support/api';

// The orders screen on the server of 4173 (open mode). The database is shared by every test and both
// browsers: each test marks its orders with a unique model and narrows the list with «/» to it.

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowsOf = (page: Page, marker: string) =>
	gridOf(page).getByRole('row').filter({ hasText: marker });
/** The row whose «№» cell is this number: a plain text match would also find it in times and phones. */
const numberCell = (page: Page, id: number) =>
	page.getByRole('gridcell', { name: String(id), exact: true });
const filterOf = (page: Page) => page.getByRole('searchbox', { name: 'Фильтр списка' });
const chipsOf = (page: Page) => page.getByRole('group', { name: 'Быстрые фильтры' });
const chip = (page: Page, name: string) =>
	chipsOf(page).getByRole('button', { name: new RegExp(`^${name}`) });
const navOf = (page: Page) => page.getByRole('navigation', { name: 'Разделы' });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });

async function openOrders(page: Page, search = '') {
	await page.goto(`/orders${search}`);
	await expect(gridOf(page)).toBeVisible();
}

/** Narrows the list to the orders of this test. */
async function narrow(page: Page, marker: string) {
	await filterOf(page).fill(marker);
	await expect(page).toHaveURL(new RegExp(`q=${encodeURIComponent(marker)}`));
}

test('quick filters and counts follow the server', async ({ page, request }) => {
	const m = uniq('M');
	const diag = await statusId(request, 'Диагностика');
	const ready = await statusId(request, 'Готов');
	await createOrder(request, { model: m, status_id: diag });
	await createOrder(request, { model: m, status_id: diag });
	await createOrder(request, { model: m, status_id: ready });

	await openOrders(page);
	await expect(chip(page, 'В работе')).toHaveAttribute('aria-pressed', 'true');
	await narrow(page, m);
	await expect(rowsOf(page, m)).toHaveCount(2);

	await chip(page, 'Готов').click();
	await expect(rowsOf(page, m)).toHaveCount(1);
	await expect(chip(page, 'Готов')).toHaveAttribute('aria-pressed', 'true');
	await expect(chip(page, 'В работе')).toHaveAttribute('aria-pressed', 'false');

	await chip(page, 'Диагностика').click({ modifiers: [await mod(page)] });
	await expect(rowsOf(page, m)).toHaveCount(3);
	await expect(page).toHaveURL(new RegExp(`chips=${ready}%2C${diag}|chips=${ready},${diag}`));

	const count = Number(await chip(page, 'Готов').locator('.count').textContent());
	expect(count).toBeGreaterThanOrEqual(1);
});

test('filters menu narrows by brand ignoring case', async ({ page, request }) => {
	const m = uniq('M');
	const diag = await statusId(request, 'Диагностика');
	await createOrder(request, { model: m, brand: 'samsung', status_id: diag });
	await createOrder(request, { model: m, brand: 'Samsung', status_id: diag });
	await createOrder(request, { model: m, brand: 'Apple', status_id: diag });

	await openOrders(page);
	await narrow(page, m);
	await expect(rowsOf(page, m)).toHaveCount(3);

	await page.getByRole('button', { name: /^Фильтры/ }).click();
	const menu = page.getByRole('dialog', { name: 'Фильтры' });
	await menu.getByRole('combobox', { name: 'Бренд' }).fill('SAMSUNG');
	await menu.getByRole('button', { name: 'Применить' }).click();
	await expect(menu).toBeHidden();
	await expect(rowsOf(page, m)).toHaveCount(2);
	await expect(page.getByText('Бренд: SAMSUNG', { exact: true })).toBeVisible();

	await page.getByRole('button', { name: 'Убрать «Бренд: SAMSUNG»' }).click();
	await expect(rowsOf(page, m)).toHaveCount(3);
	await expect(page.getByText('Бренд: SAMSUNG', { exact: true })).toHaveCount(0);
});

test('sorting by device and hiding a column survive a reload', async ({ page, request }) => {
	const m = uniq('M');
	const diag = await statusId(request, 'Диагностика');
	for (const brand of ['Bbb', 'aaa', 'Ccc']) {
		await createOrder(request, { model: m, brand, status_id: diag });
	}
	await openOrders(page);
	await narrow(page, m);
	const header = gridOf(page).getByRole('columnheader', { name: 'Устройство' });
	await header.getByRole('button').click();
	await expect(header).toHaveAttribute('aria-sort', 'descending');
	await expect(rowsOf(page, m).first()).toContainText('Ccc');
	await header.getByRole('button').click();
	await expect(header).toHaveAttribute('aria-sort', 'ascending');
	await expect(rowsOf(page, m).first()).toContainText('aaa');
	await expect(rowsOf(page, m).last()).toContainText('Ccc');
	await expect(page).toHaveURL(/sort=device&desc=0/);

	await gridOf(page).getByRole('button', { name: 'Колонки' }).click();
	await page.getByRole('checkbox', { name: 'Мастер' }).uncheck();
	await page.keyboard.press('Escape');
	await expect(gridOf(page).getByRole('columnheader', { name: 'Мастер' })).toHaveCount(0);

	await page.reload();
	await expect(rowsOf(page, m)).toHaveCount(3);
	await expect(rowsOf(page, m).first()).toContainText('aaa');
	await expect(gridOf(page).getByRole('columnheader', { name: 'Устройство' })).toHaveAttribute(
		'aria-sort',
		'ascending'
	);
	await expect(gridOf(page).getByRole('columnheader', { name: 'Мастер' })).toHaveCount(0);
	await expect(filterOf(page)).toHaveValue(m);
});

test('«Позвонить» in «Требуют внимания» shows orders that need a call', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const call = await createOrder(request, { model: m, need_call: true });
	await createOrder(request, { model: m, need_call: false });
	await openOrders(page);
	await navOf(page)
		.getByRole('link', { name: /Позвонить/ })
		.click();
	await expect(page).toHaveURL(/attention=call/);
	await narrow(page, m);
	await expect(rowsOf(page, m)).toHaveCount(1);
	await expect(rowsOf(page, m)).toContainText(String(call.id));
	// The quick filters do not decide here.
	await expect(chipsOf(page).getByRole('button', { pressed: true })).toHaveCount(0);

	// A chip leaves «Требуют внимания».
	await chip(page, 'Все').click();
	await expect(page).not.toHaveURL(/attention=/);
	await expect(rowsOf(page, m)).toHaveCount(2);
});

test('back button restores the previous view', async ({ page, request }) => {
	const m = uniq('M');
	await createOrder(request, { model: m, status_id: await statusId(request, 'Готов') });
	await openOrders(page);
	await chip(page, 'Готов').click();
	await expect(chip(page, 'Готов')).toHaveAttribute('aria-pressed', 'true');
	await chip(page, 'Все').click();
	await expect(chip(page, 'Все')).toHaveAttribute('aria-pressed', 'true');
	await page.goBack();
	await expect(chip(page, 'Готов')).toHaveAttribute('aria-pressed', 'true');
	await expect(chip(page, 'Все')).toHaveAttribute('aria-pressed', 'false');
	await page.goBack();
	await expect(chip(page, 'В работе')).toHaveAttribute('aria-pressed', 'true');
});

test('the list filter: «/» focuses it, typing narrows, back brings it back', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await openOrders(page);
	await gridOf(page).focus();
	await page.keyboard.press('/');
	await expect(filterOf(page)).toBeFocused();
	await page.keyboard.type(m);
	// Once typing pauses the text goes to the address in place of the current entry.
	await expect(page).toHaveURL(new RegExp(`/orders\\?q=${m}$`));
	await expect(rowsOf(page, m)).toHaveCount(1);
	await expect(rowsOf(page, m)).toContainText(String(o.id));
	await chip(page, 'Все').click();
	await expect(chip(page, 'Все')).toHaveAttribute('aria-pressed', 'true');
	await page.goBack();
	await expect(filterOf(page)).toHaveValue(m);
	await expect(chip(page, 'В работе')).toHaveAttribute('aria-pressed', 'true');
});

test('typing n in the list filter does not open intake', async ({ page }) => {
	await openOrders(page);
	const filter = filterOf(page);
	await filter.focus();
	await page.keyboard.press('n');
	await expect(filter).toHaveValue('n');
	await expect(page.getByRole('dialog', { name: 'Новый заказ' })).toHaveCount(0);
});

test('a row opens its order in the panel and stays marked', async ({ page, request }) => {
	const m = uniq('M');
	const first = await createOrder(request, { model: m });
	const second = await createOrder(request, { model: m });
	await openOrders(page);
	await narrow(page, m);
	const row = rowsOf(page, m).filter({ has: numberCell(page, first.id) });
	await row.getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, first.id)).toBeVisible();
	await expect(row).toHaveAttribute('aria-current', 'true');
	// The panel goes along the list: newest first, so the older order is next after the newer one.
	expect(second.id).toBeGreaterThan(first.id);
	await page.getByRole('button', { name: 'Предыдущая запись' }).click();
	await expect(panelOf(page, second.id)).toBeVisible();
	await expect(rowsOf(page, m).filter({ has: numberCell(page, second.id) })).toHaveAttribute(
		'aria-current',
		'true'
	);
});

test('a client from Ctrl K shows all their orders under a chip', async ({ page, request }) => {
	const marker = uniq('a').split('-')[1]!;
	const mine = await createOrder(request, {
		clientName: `ТЕСТОВА ${marker.toUpperCase()}`,
		model: uniq('M'),
		status_id: await statusId(request, 'Готов')
	});
	await createOrder(request, { model: uniq('M') });
	await openOrders(page);
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
	await palette.getByRole('combobox').fill(marker);
	await palette.getByRole('option', { name: /^Тестова/ }).click();
	await expect(page).toHaveURL(new RegExp(`client=${mine.client_id}`));
	await expect(page.getByText(/^Клиент: Тестова/)).toBeVisible();
	// Every order of the client, not only those in work.
	await expect(chip(page, 'Все')).toHaveAttribute('aria-pressed', 'true');
	await expect(
		gridOf(page)
			.getByRole('row')
			.filter({ has: numberCell(page, mine.id) })
	).toHaveCount(1);
	// The header and the one order of the client.
	await expect(gridOf(page).getByRole('row')).toHaveCount(2);
	await page.getByRole('button', { name: /^Убрать «Клиент: / }).click();
	await expect(page).not.toHaveURL(/client=/);
});

test('nothing found offers to reset the filters', async ({ page }) => {
	await openOrders(page);
	await narrow(page, uniq('нет-такого'));
	await expect(page.getByText('Ничего не нашлось')).toBeVisible();
	await page.getByRole('button', { name: 'Сбросить фильтры' }).click();
	await expect(filterOf(page)).toHaveValue('');
	await expect(page).toHaveURL(/\/orders$/);
	await expect(chip(page, 'В работе')).toHaveAttribute('aria-pressed', 'true');
});

test('a saved view appears in «Мои виды», opens and can be removed', async ({ page, request }) => {
	const m = uniq('M');
	await createOrder(request, { model: m, status_id: await statusId(request, 'Готов') });
	await openOrders(page);
	await chip(page, 'Готов').click();
	await narrow(page, m);
	await page.getByRole('button', { name: /^Фильтры/ }).click();
	await page.getByRole('button', { name: 'Сохранить вид…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Сохранить вид' });
	await dialog.getByRole('textbox', { name: 'Название' }).fill('Готовые для проверки');
	await dialog.getByRole('button', { name: 'Сохранить' }).click();
	await expect(dialog).toBeHidden();

	const link = navOf(page).getByRole('link', { name: 'Готовые для проверки' });
	await expect(link).toBeVisible();
	await expect(link).toHaveAttribute('aria-current', 'page');
	await chip(page, 'Все').click();
	await expect(link).not.toHaveAttribute('aria-current', 'page');
	await link.click();
	await expect(chip(page, 'Готов')).toHaveAttribute('aria-pressed', 'true');
	await expect(filterOf(page)).toHaveValue(m);
	await expect(rowsOf(page, m)).toHaveCount(1);

	await page.reload();
	await navOf(page).getByRole('button', { name: 'Удалить вид' }).click();
	await expect(link).toHaveCount(0);
	expect(await page.evaluate(() => localStorage.getItem('mw.views'))).toBe('[]');
});

test('saving a view applies the filters being edited', async ({ page, request }) => {
	const m = uniq('M');
	const brand = uniq('B');
	const diag = await statusId(request, 'Диагностика');
	await createOrder(request, { model: m, brand, status_id: diag });
	await createOrder(request, { model: m, brand: 'Apple', status_id: diag });
	await openOrders(page);
	await narrow(page, m);
	await expect(rowsOf(page, m)).toHaveCount(2);

	// A brand is chosen, «Применить» is not pressed: saving the view applies it.
	await page.getByRole('button', { name: /^Фильтры/ }).click();
	const menu = page.getByRole('dialog', { name: 'Фильтры' });
	await menu.getByRole('combobox', { name: 'Бренд' }).fill(brand);
	await menu.getByRole('button', { name: 'Сохранить вид…' }).click();
	const dialog = page.getByRole('dialog', { name: 'Сохранить вид' });
	await dialog.getByRole('textbox', { name: 'Название' }).fill('Один бренд');
	await dialog.getByRole('button', { name: 'Сохранить' }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByText(`Бренд: ${brand}`, { exact: true })).toBeVisible();
	await expect(rowsOf(page, m)).toHaveCount(1);

	// The saved view brings the brand back from another view.
	await navOf(page)
		.getByRole('link', { name: /^Заказы/ })
		.click();
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByText(`Бренд: ${brand}`, { exact: true })).toHaveCount(0);
	await navOf(page).getByRole('link', { name: 'Один бренд' }).click();
	await expect(page).toHaveURL(new RegExp(`brand=${brand}`));
	await expect(page.getByText(`Бренд: ${brand}`, { exact: true })).toBeVisible();
	await expect(rowsOf(page, m)).toHaveCount(1);
	await expect(rowsOf(page, m)).toContainText(brand);
});

test.describe('on a phone', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('phone layout shows cards', async ({ page, request }) => {
		const m = uniq('M');
		const o = await createOrder(request, { model: m });
		await page.goto('/orders');
		await expect(gridOf(page)).toHaveCount(0);
		await narrow(page, m);
		const card = page.getByRole('button', { name: new RegExp(`№ ${o.id}(?!\\d)`) });
		await expect(card).toBeVisible();
		await expect(card).toContainText(m);
		await card.click();
		await expect(panelOf(page, o.id)).toBeVisible();
	});
});
