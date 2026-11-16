import { expect, test, type Page } from '@playwright/test';
import type { OrderPage } from '$api/OrderPage';
import { createOrder, uniq } from './support/api';

// The server on 4173 runs without sign-in: the shell opens straight away.

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

/** A barcode scanner: characters 5 ms apart, then Enter. */
async function scan(page: Page, code: string) {
	await page.keyboard.type(code, { delay: 5 });
	await page.keyboard.press('Enter');
}

const number = (id: number) => new RegExp(`№ ${id}(?!\\d)`);
const paletteOf = (page: Page) => page.getByRole('dialog', { name: 'Найти или выполнить' });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: `№ ${id}`, exact: true });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
const navOf = (page: Page) => page.getByRole('navigation', { name: 'Разделы' });

async function openOrders(page: Page) {
	await page.goto('/orders');
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
}

test('root opens the orders screen with the shell', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL(/\/orders$/);
	await expect(navOf(page)).toContainText('Заказы');
	await expect(page.getByRole('button', { name: /Принять/ })).toBeVisible();
	// Without sign-in there is no user menu, and /login is not needed.
	await expect(page.getByRole('button', { name: /Пользователь/ })).toHaveCount(0);
	await page.goto('/login');
	await expect(page).toHaveURL(/\/orders$/);
});

test('«Требуют внимания» links to the orders that need a call', async ({ page, request }) => {
	await createOrder(request, { model: uniq('M'), need_call: true });
	await openOrders(page);
	const call = navOf(page).getByRole('link', { name: /Позвонить/ });
	await expect(call).toHaveAttribute('href', '/orders?attention=call');
	await call.click();
	await expect(page).toHaveURL(/\/orders\?attention=call$/);
	await expect(call).toHaveAttribute('aria-current', 'page');
});

test('Ctrl K finds an order by number and Enter opens its panel', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openOrders(page);
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const palette = paletteOf(page);
	await expect(palette).toBeVisible();
	await palette.getByRole('combobox').fill(String(o.id));
	const first = palette.getByRole('option').first();
	await expect(first).toContainText(number(o.id));
	await expect(first).toHaveAttribute('aria-selected', 'true');
	await page.keyboard.press('Enter');
	await expect(palette).toBeHidden();
	await expect(panelOf(page, o.id)).toBeVisible();

	// The order is now among «Недавние» of an empty palette.
	await page.keyboard.press('Escape');
	await expect(panelOf(page, o.id)).toBeHidden();
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	await expect(palette.getByText('Недавние')).toBeVisible();
	await expect(palette.getByRole('option', { name: number(o.id) })).toBeVisible();
});

test('a scanned barcode opens its order', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openOrders(page);
	await scan(page, o.barcode);
	await expect(panelOf(page, o.id)).toBeVisible();
});

test('barcode with two orders asks which one', async ({ page, request }) => {
	// PATCH /orders has no barcode: the shared code is a stubbed answer of the lookup.
	const m = uniq('M');
	await createOrder(request, { model: m });
	await createOrder(request, { model: m });
	const page2 = (await (
		await request.get(`/api/v1/orders?query=${encodeURIComponent(m)}`)
	).json()) as OrderPage;
	expect(page2.items).toHaveLength(2);
	const [first, second] = page2.items as [OrderPage['items'][0], OrderPage['items'][0]];
	await page.route('**/api/v1/orders/by-barcode*', (route) => route.fulfill({ json: page2.items }));

	await openOrders(page);
	await scan(page, '012345678905');
	const dialog = page.getByRole('dialog', { name: /Несколько заказов/ });
	await expect(dialog).toBeVisible();
	await expect(dialog.getByRole('button', { name: number(first.id) })).toBeVisible();
	await expect(dialog.getByRole('button', { name: number(second.id) })).toBeVisible();
	await expect(dialog.getByRole('button', { name: number(first.id) })).toBeFocused();
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(dialog).toBeHidden();
	await expect(panelOf(page, second.id)).toBeVisible();
});

test('unknown barcode shows a toast', async ({ page }) => {
	await openOrders(page);
	await scan(page, '199999999990');
	await expect(toastsOf(page)).toContainText('Заказ со штрихкодом 199999999990 не найден');
});

test('scan into a focused field stays in the field', async ({ page }) => {
	await openOrders(page);
	// The list filter comes with the orders screen; any text field of the page behaves the same.
	await page.evaluate(() => {
		const input = document.createElement('input');
		input.setAttribute('aria-label', 'Поле для проверки');
		document.querySelector('main')?.append(input);
	});
	const field = page.getByRole('textbox', { name: 'Поле для проверки' });
	await field.focus();
	await scan(page, '199999999991');
	await expect(field).toHaveValue('199999999991');

	// The same scan outside the field is looked up: only that one is reported.
	await page.locator('main').focus();
	await scan(page, '199999999992');
	await expect(toastsOf(page)).toContainText('Заказ со штрихкодом 199999999992 не найден');
	await expect(toastsOf(page)).not.toContainText('199999999991');
	await expect(page.getByRole('dialog', { name: /^№/ })).toHaveCount(0);
});
