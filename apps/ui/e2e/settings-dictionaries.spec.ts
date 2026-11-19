import { expect, test, type Page } from '@playwright/test';
import { addDictionaryItem, createOrder, fetchOrder, uniq } from './support/api';

// «Настройки → Справочники»: the values of the ten dictionaries — counts, order, rename, archive, merge.
// The dictionaries of the shared servers are global: every value here carries `uniq()`.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

async function openBrands(page: Page) {
	await page.goto('/settings/dictionaries?kind=brand');
	await expect(page.getByRole('heading', { name: 'Справочники' })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Добавить значение' })).toBeVisible();
}

const rowOf = (page: Page, text: string) =>
	page.getByRole('main').getByRole('listitem').filter({ hasText: text });

async function add(page: Page, value: string) {
	const field = page.getByRole('textbox', { name: 'Добавить значение' });
	await field.fill(value);
	await field.press('Enter');
}

test('«Привести написание в заказах» makes the orders spell the value as the dictionary does', async ({
	page,
	request
}) => {
	const id = uniq();
	await addDictionaryItem(request, 'brand', `Delta-${id}`);
	// Typed in another case and with a stray space: the same value for the program, another text in the order.
	const order = await createOrder(request, { brand: `delta-${id} ` });
	const kept = await createOrder(request, { brand: `Delta-${id}` });
	await openBrands(page);
	const row = rowOf(page, `Delta-${id}`);
	await expect(row).toContainText('в 2 заказах');
	await row.getByRole('button', { name: 'Привести написание в заказах' }).click();
	await expect(page.getByText('Приведено: изменено 1 заказ')).toBeVisible();
	expect((await fetchOrder(request, order.id)).brand).toBe(`Delta-${id}`);
	const after = await fetchOrder(request, kept.id);
	expect(after.brand).toBe(`Delta-${id}`);
	expect(after.version).toBe(kept.version);
	// A value that no order uses has nothing to align.
	await addDictionaryItem(request, 'brand', `Epsilon-${id}`);
	await page.reload();
	await expect(rowOf(page, `Epsilon-${id}`)).toBeVisible();
	await expect(
		rowOf(page, `Epsilon-${id}`).getByRole('button', { name: 'Привести написание в заказах' })
	).toHaveCount(0);
});

test('a duplicate in another case is refused, counts and rename follow the orders, merge keeps one', async ({
	page,
	request
}) => {
	const id = uniq();
	await openBrands(page);
	await add(page, `Alpha-${id}`);
	await expect(rowOf(page, `Alpha-${id}`)).toBeVisible();
	await add(page, `Beta-${id}`);
	await expect(rowOf(page, `Beta-${id}`)).toBeVisible();

	await add(page, `alpha-${id}`);
	await expect(page.getByText('Такое значение уже есть — объедините их')).toBeVisible();

	const order = await createOrder(request, { brand: `alpha-${id}` });
	await page.reload();
	await expect(rowOf(page, `Alpha-${id}`)).toContainText('в 1 заказе');

	// Rename with the orders.
	await rowOf(page, `Alpha-${id}`)
		.getByRole('button', { name: `Alpha-${id}`, exact: true })
		.click();
	const edit = page.getByRole('textbox', { name: `Значение „Alpha-${id}“` });
	await edit.fill(`Gamma-${id}`);
	await edit.press('Enter');
	const dialog = page.getByRole('dialog', {
		name: `Переименовать „Alpha-${id}“ в „Gamma-${id}“`
	});
	await expect(dialog.getByRole('checkbox', { name: 'И в 1 заказе' })).toBeChecked();
	await dialog.getByRole('button', { name: 'Переименовать' }).click();
	await expect(page.getByText('Переименовано, изменено 1 заказ')).toBeVisible();
	await expect(rowOf(page, `Gamma-${id}`)).toContainText('в 1 заказе');
	expect((await fetchOrder(request, order.id)).brand).toBe(`Gamma-${id}`);

	// Merge, keeping Beta.
	await rowOf(page, `Gamma-${id}`).getByRole('checkbox').check();
	await rowOf(page, `Beta-${id}`).getByRole('checkbox').check();
	await page.getByRole('button', { name: 'Объединить выбранные' }).click();
	const merge = page.getByRole('dialog', { name: 'Объединить значения' });
	await merge.getByRole('radio', { name: new RegExp(`^Beta-${id}`) }).check();
	await expect(merge.getByRole('checkbox', { name: 'Заменить и в заказах' })).toBeChecked();
	await merge.getByRole('button', { name: 'Объединить', exact: true }).click();
	await expect(page.getByText('Объединено, изменено 1 заказ')).toBeVisible();
	await expect(rowOf(page, `Beta-${id}`)).toHaveCount(1);
	await expect(rowOf(page, `Beta-${id}`)).toContainText('в 1 заказе');
	await expect(rowOf(page, `Gamma-${id}`)).toHaveCount(0);
	expect((await fetchOrder(request, order.id)).brand).toBe(`Beta-${id}`);
});

test('while a rename runs, Esc and a click outside do not close its window', async ({
	page,
	request
}) => {
	const id = uniq();
	await addDictionaryItem(request, 'brand', `Slow-${id}`);
	await createOrder(request, { brand: `Slow-${id}` });
	await openBrands(page);
	await expect(rowOf(page, `Slow-${id}`)).toContainText('в 1 заказе');

	// The answer waits until the test lets it go.
	let release!: () => void;
	const held = new Promise<void>((r) => (release = r));
	let arrived!: () => void;
	const sent = new Promise<void>((r) => (arrived = r));
	await page.route('**/api/v1/dictionary-items/*/rename', async (route) => {
		arrived();
		await held;
		await route.continue();
	});

	await rowOf(page, `Slow-${id}`)
		.getByRole('button', { name: `Slow-${id}`, exact: true })
		.click();
	const edit = page.getByRole('textbox', { name: `Значение „Slow-${id}“` });
	await edit.fill(`Fast-${id}`);
	await edit.press('Enter');
	const dialog = page.getByRole('dialog', { name: `Переименовать „Slow-${id}“ в „Fast-${id}“` });
	await dialog.getByRole('button', { name: 'Переименовать' }).click();
	await sent;

	// A dismissible dialog would close a frame or two later: wait for them, then look.
	const frames = () =>
		page.evaluate(
			() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(null))))
		);
	await page.keyboard.press('Escape');
	await frames();
	await expect(dialog).toHaveAttribute('data-state', 'open');
	await page.mouse.click(5, 5);
	await frames();
	await expect(dialog).toHaveAttribute('data-state', 'open');
	await expect(dialog.getByRole('button', { name: 'Отмена' })).toBeDisabled();

	release();
	await expect(dialog).toBeHidden();
	await expect(page.getByText('Переименовано, изменено 1 заказ')).toBeVisible();
	await expect(rowOf(page, `Fast-${id}`)).toContainText('в 1 заказе');
});

test('an unused value is deleted after confirmation; Esc cancels an edit', async ({ page }) => {
	const id = uniq();
	await openBrands(page);
	await add(page, `Zeta-${id}`);
	const row = rowOf(page, `Zeta-${id}`);
	await row.getByRole('button', { name: `Zeta-${id}`, exact: true }).click();
	await page.getByRole('textbox', { name: `Значение „Zeta-${id}“` }).fill('Другое');
	await page.keyboard.press('Escape');
	await expect(row).toContainText(`Zeta-${id}`);
	await row.getByRole('button', { name: 'Удалить' }).click();
	await page
		.getByRole('alertdialog', { name: `Удалить „Zeta-${id}“?` })
		.getByRole('button', { name: 'Удалить' })
		.click();
	await expect(rowOf(page, `Zeta-${id}`)).toHaveCount(0);
});

test('an archived brand leaves the intake suggestions and comes back with «Вернуть»', async ({
	page,
	request
}) => {
	const id = uniq();
	const brand = `Arch-${id}`;
	const keep = `Archx-${id}`;
	await addDictionaryItem(request, 'brand', keep);
	await openBrands(page);
	await add(page, brand);
	await expect(rowOf(page, brand)).toBeVisible();

	const suggests = async () => {
		await page.goto('/orders');
		await page.getByRole('main').click();
		await page.keyboard.press('n');
		const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
		const field = sheet.getByRole('combobox', { name: /^Бренд/ });
		await field.fill(`Arch`);
		// The suggestions are loaded once the value that stays shows up.
		await expect(page.getByRole('option', { name: keep, exact: true })).toBeVisible();
		return page.getByRole('option', { name: brand, exact: true });
	};

	await openBrands(page);
	await rowOf(page, brand).getByRole('button', { name: 'В архив' }).click();
	await expect(page.getByRole('heading', { name: 'Архив' })).toBeVisible();
	await expect(await suggests()).toHaveCount(0);

	await openBrands(page);
	await rowOf(page, brand).getByRole('button', { name: 'Вернуть' }).click();
	// Back among the live values (the archive of the shared server may hold the values of other specs).
	await expect(rowOf(page, brand).getByRole('button', { name: 'В архив' })).toBeVisible();
	await expect(await suggests()).toBeVisible();
});

test('dragging by the handle changes the order and it survives a reload', async ({
	page,
	request
}) => {
	const id = uniq();
	await addDictionaryItem(request, 'warranty', `Первая-${id}`);
	await addDictionaryItem(request, 'warranty', `Вторая-${id}`);
	await page.goto('/settings/dictionaries?kind=warranty');
	await page.getByRole('textbox', { name: 'Найти' }).fill(id);
	const rows = page.getByRole('main').getByRole('listitem').filter({ hasText: id });
	await expect(rows).toHaveCount(2);
	await expect(rows.first()).toContainText(`Первая-${id}`);
	const handle = rows.last().getByRole('button', { name: `Переместить „Вторая-${id}“` });
	const from = await handle.boundingBox();
	const to = await rows.first().boundingBox();
	if (!from || !to) throw new Error('no boxes');
	// The list shows the new order at once; the reload waits for the server to have taken it.
	const saved = page.waitForResponse(
		(r) =>
			r.url().endsWith('/api/v1/dictionaries/warranty/order') &&
			r.request().method() === 'PUT' &&
			r.ok(),
		{ timeout: 10_000 }
	);
	await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
	await page.mouse.down();
	await page.mouse.move(from.x + from.width / 2, to.y + 2, { steps: 8 });
	await page.mouse.up();
	await expect(rows.first()).toContainText(`Вторая-${id}`);
	await saved;

	await page.reload();
	await page.getByRole('textbox', { name: 'Найти' }).fill(id);
	await expect(rows.first()).toContainText(`Вторая-${id}`);
	await expect(rows.last()).toContainText(`Первая-${id}`);
});
