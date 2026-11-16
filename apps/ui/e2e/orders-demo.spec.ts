import { expect, test, type Page } from '@playwright/test';

// /design/orders shows makeOrders(2000): fictional orders 10001…12000 (seed 1). The default quick filter
// «В работе» lists open and diagnostics orders, newest first: № 11999, 11998, 11997 are «Диагностика».
const FIRST = 11999;
const SECOND = 11998;
const THIRD = 11997;
// «Готов», final price 4 050 ₽, prepayment 2 300 ₽.
const READY = 11944;
// The newest order, «Готов»: first under the quick filter «Готов».
const NEWEST_READY = 12000;

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, n: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(n), exact: true }) });
const bodyRows = (page: Page) => gridOf(page).locator('[role="row"]:not([aria-rowindex="1"])');
const panelOf = (page: Page) => page.getByRole('dialog', { name: /^№ \d+/ });
const titleOf = (page: Page) => panelOf(page).getByRole('heading', { level: 2 });
const filterOf = (page: Page) => page.getByRole('searchbox', { name: 'Фильтр списка' });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
const paletteOf = (page: Page) => page.getByRole('dialog', { name: 'Найти или выполнить' });

/** Opens an order with a click on its «Клиент» cell (left of the panel, which covers the right side). */
async function openRow(page: Page, n: number) {
	await rowOf(page, n).getByRole('gridcell').nth(3).click();
	await expect(titleOf(page)).toHaveText(new RegExp(`^№ ${n} · `));
}

test.describe('orders demo', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design/orders');
		await expect(rowOf(page, FIRST)).toBeVisible();
	});

	test('a click on a row opens its panel with fields; a click on another visible row switches it', async ({
		page
	}) => {
		await openRow(page, FIRST);
		const panel = panelOf(page);
		await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
		await expect(panel.getByRole('textbox', { name: 'Выполненные работы' })).toBeVisible();
		await expect(panel.getByRole('combobox', { name: 'Мастер' })).toBeVisible();
		await expect(panel.getByRole('textbox', { name: 'Итог' })).toBeVisible();
		await expect(panel.getByRole('radiogroup', { name: 'Статус' })).toBeVisible();
		await expect(rowOf(page, FIRST)).toHaveAttribute('aria-current', 'true');

		await openRow(page, THIRD);
		await expect(panel).toHaveCount(1);
		await expect(rowOf(page, THIRD)).toHaveAttribute('aria-current', 'true');
	});

	test('↓ in the panel opens the next order of the list; Esc closes it, focus returns to that row', async ({
		page
	}) => {
		await openRow(page, FIRST);
		await expect(panelOf(page)).toBeFocused();
		await page.keyboard.press('ArrowDown');
		await expect(titleOf(page)).toHaveText(new RegExp(`^№ ${SECOND} · `));

		await page.keyboard.press('Escape');
		await expect(panelOf(page)).toBeHidden();
		await expect(gridOf(page)).toBeFocused();
		await expect(rowOf(page, SECOND)).toHaveAttribute('aria-current', 'true');
	});

	test('/ moves focus to the filter', async ({ page }) => {
		await page.keyboard.press('/');
		await expect(filterOf(page)).toBeFocused();
		await expect(filterOf(page)).toHaveValue('');
	});

	test('the filter: «иванов» keeps only Ивановы (stored in capitals), «ёлкин» finds ЕЛКИН', async ({
		page
	}) => {
		const filter = filterOf(page);
		const rows = bodyRows(page);
		await filter.fill('иванов');
		await expect(rows.first()).toContainText('Иванов');
		await expect
			.poll(async () => (await rows.allInnerTexts()).filter((t) => !t.includes('Иванов')))
			.toEqual([]);
		await expect(rowOf(page, FIRST)).toHaveCount(0);

		await filter.fill('ёлкин');
		await expect(rows.first()).toContainText('Елкин');
		await expect
			.poll(async () => (await rows.allInnerTexts()).filter((t) => !t.includes('Елкин')))
			.toEqual([]);
	});

	test('typing n in the filter does not open intake; N outside a field does', async ({ page }) => {
		const filter = filterOf(page);
		await filter.focus();
		await page.keyboard.press('n');
		await expect(filter).toHaveValue('n');
		await expect(page).toHaveURL(/\/design\/orders$/);

		await filter.blur();
		await page.keyboard.press('n');
		await expect(page).toHaveURL(/\/design\/intake$/);
		const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
		await expect(sheet).toBeVisible();
		// Typing goes straight into the focused first field (WebKit lost it after a navigation).
		const phone = sheet.getByRole('textbox', { name: 'Телефон' });
		await expect(phone).toBeFocused();
		await page.keyboard.type('70000000001');
		await expect(phone).toHaveValue('70000000001');
	});

	test('status in the panel: «Готов» shows «№ … → Готов», «Отменить» restores the table and the panel', async ({
		page
	}) => {
		await openRow(page, FIRST);
		const statuses = panelOf(page).getByRole('radiogroup', { name: 'Статус' });
		await expect(statuses.getByRole('radio', { name: 'Диагностика' })).toBeChecked();

		await statuses.getByRole('radio', { name: 'Готов' }).check();
		await expect(toastsOf(page)).toContainText(`№ ${FIRST} → Готов`);
		await expect(statuses.getByRole('radio', { name: 'Готов' })).toBeChecked();
		await expect(rowOf(page, FIRST)).toContainText('Готов');

		await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
		await expect(statuses.getByRole('radio', { name: 'Диагностика' })).toBeChecked();
		await expect(rowOf(page, FIRST)).toContainText('Диагностика');
		await expect(rowOf(page, FIRST)).not.toContainText('Готов');
	});

	test('status picker: arrows change the status', async ({ page }) => {
		await openRow(page, FIRST);
		const statuses = panelOf(page).getByRole('radiogroup', { name: 'Статус' });
		await statuses.getByRole('radio', { name: 'Диагностика' }).focus();
		await page.keyboard.press('ArrowRight');
		await expect(statuses.getByRole('radio', { name: 'Согласование с клиентом' })).toBeChecked();
		await expect(toastsOf(page)).toContainText(`№ ${FIRST} → Согласование с клиентом`);
		await expect(titleOf(page)).toHaveText(new RegExp(`^№ ${FIRST} · `));
		await expect(rowOf(page, FIRST)).toContainText('Согласование с клиентом');
	});

	test('editing «Выполненные работы»: «Сохраняю…», then «✓ Сохранено» 600 ms after the last edit', async ({
		page
	}) => {
		await openRow(page, FIRST);
		const panel = panelOf(page);
		const work = panel.getByRole('textbox', { name: 'Выполненные работы' });
		await work.focus();
		const typed = Date.now();
		await work.fill('Замена разъёма зарядки');
		await expect(panel.getByText('Сохраняю…')).toBeVisible();
		await expect(panel.getByText('✓ Сохранено')).toBeVisible();
		expect(Date.now() - typed).toBeGreaterThanOrEqual(600);
		await expect(panel.getByText('Сохраняю…')).toHaveCount(0);
		await expect(work).toHaveValue('Замена разъёма зарядки');
	});

	test('«Выдать»: «К оплате» = итог − предоплата − скидка, empty required is marked, Ctrl/⌘+Enter issues, «Отменить» gives everything back', async ({
		page
	}) => {
		await page
			.getByRole('group', { name: 'Быстрые фильтры' })
			.getByRole('button', { name: /^Готов/ })
			.click();
		await filterOf(page).fill(String(READY));
		await openRow(page, READY);
		const panel = panelOf(page);
		const statuses = panel.getByRole('radiogroup', { name: 'Статус' });
		const panelWork = panel.getByRole('textbox', { name: 'Выполненные работы' });
		const panelTotal = panel.getByRole('textbox', { name: 'Итог' });
		const panelDiscount = panel.getByRole('textbox', { name: 'Скидка' });
		const workBefore = await panelWork.inputValue();
		await expect(panelTotal).toHaveValue(/^4\s050$/);
		await expect(panelDiscount).toHaveValue('0');
		await expect(rowOf(page, READY)).toContainText(/4\s050\s₽/);
		await panel.getByRole('button', { name: /^Выдать/ }).click();

		const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${READY}` });
		await expect(dialog).toBeVisible();
		const toPay = dialog.getByText(/^К оплате/);
		await expect(toPay).toHaveText(/^К оплате\s1\s750\s₽$/);
		const discount = dialog.getByRole('textbox', { name: 'Скидка' });
		await discount.fill('250');
		await discount.press('Tab');
		await expect(toPay).toHaveText(/^К оплате\s1\s500\s₽$/);
		const price = dialog.getByRole('textbox', { name: /^Итоговая цена/ });
		// Focus first: MoneyInput turns «4 050» into «4050» on focus, which would undo fill()'s select-all.
		await price.click();
		await price.fill('5000');
		await price.press('Tab');
		await expect(toPay).toHaveText(/^К оплате\s2\s450\s₽$/);

		const work = dialog.getByRole('textbox', { name: /^Выполненные работы/ });
		await work.fill('');
		await expect(work).toHaveAttribute('aria-invalid', 'true');
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(dialog).toBeVisible();

		await work.fill('Замена аккумулятора');
		await expect(work).not.toHaveAttribute('aria-invalid', 'true');
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(dialog).toBeHidden();
		await expect(statuses.getByRole('radio', { name: 'Выдан' })).toBeChecked();
		await expect(panelWork).toHaveValue('Замена аккумулятора');
		await expect(panelTotal).toHaveValue(/^5\s000$/);
		await expect(rowOf(page, READY)).toContainText('Выдан');
		await expect(rowOf(page, READY)).toContainText(/5\s000\s₽/);
		await expect(toastsOf(page)).toContainText(`№ ${READY} → Выдан`);

		// «Отменить» gives back the status and everything the dialog filled in.
		await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
		await expect(statuses.getByRole('radio', { name: 'Готов' })).toBeChecked();
		await expect(panelWork).toHaveValue(workBefore);
		await expect(panelTotal).toHaveValue(/^4\s050$/);
		await expect(panelDiscount).toHaveValue('0');
		await expect(rowOf(page, READY)).toContainText('Готов');
		await expect(rowOf(page, READY)).toContainText(/4\s050\s₽/);
		await expect(rowOf(page, READY)).not.toContainText('Выдан');
	});

	test('an issued status picked in the panel opens «Выдать»: the status stays until the dialog issues; arrows onto it do not issue', async ({
		page
	}) => {
		await openRow(page, FIRST);
		const statuses = panelOf(page).getByRole('radiogroup', { name: 'Статус' });
		await statuses.getByRole('radio', { name: 'Выдан' }).click();
		const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${FIRST}` });
		await expect(dialog).toBeVisible();
		await expect(dialog.getByRole('textbox', { name: /^Выполненные работы/ })).toHaveAttribute(
			'aria-invalid',
			'true'
		);
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
		await expect(statuses.getByRole('radio', { name: 'Диагностика' })).toBeChecked();
		await expect(statuses.getByRole('radio', { name: 'Выдан' })).not.toBeChecked();
		await expect(rowOf(page, FIRST)).toContainText('Диагностика');
		await expect(toastsOf(page)).not.toContainText('→ Выдан');

		// → from «Готов» lands on «Выдан»: the dialog again, the order stays «Готов».
		await page.keyboard.press('Escape');
		await page
			.getByRole('group', { name: 'Быстрые фильтры' })
			.getByRole('button', { name: /^Готов/ })
			.click();
		await openRow(page, NEWEST_READY);
		await statuses.getByRole('radio', { name: 'Готов' }).focus();
		await page.keyboard.press('ArrowRight');
		const again = page.getByRole('dialog', { name: `Выдача заказа № ${NEWEST_READY}` });
		await expect(again).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(again).toBeHidden();
		await expect(statuses.getByRole('radio', { name: 'Готов' })).toBeChecked();
		await expect(statuses.getByRole('radio', { name: 'Выдан' })).not.toBeChecked();
		await expect(rowOf(page, NEWEST_READY)).toContainText('Готов');
		await expect(rowOf(page, NEWEST_READY)).not.toContainText('Выдан');
		await expect(toastsOf(page)).not.toContainText('→ Выдан');
	});

	test('a scanned barcode opens its order; an unknown barcode is reported', async ({ page }) => {
		// A scanner types fast and ends with Enter: synthetic key events keep the burst timing exact.
		const scan = (code: string) =>
			page.evaluate((text) => {
				for (const key of [...text, 'Enter']) {
					window.dispatchEvent(new KeyboardEvent('keydown', { key, cancelable: true }));
				}
			}, code);
		await scan('200000010500');
		await expect(titleOf(page)).toHaveText(/^№ 10500 · /);
		await page.keyboard.press('Escape');
		await expect(panelOf(page)).toBeHidden();

		await scan('200000099999');
		await expect(toastsOf(page)).toContainText('Заказ со штрихкодом 200000099999 не найден');
		await expect(panelOf(page)).toBeHidden();
	});

	test('Ctrl/⌘+K: a number shows that order first, Enter opens its panel', async ({ page }) => {
		await page.keyboard.press(`${await mod(page)}+KeyK`);
		const palette = paletteOf(page);
		await expect(palette).toBeVisible();
		await palette.getByRole('combobox').fill('10500');
		const first = palette.getByRole('option').first();
		await expect(first).toContainText('№ 10500');
		await expect(first).toHaveAttribute('aria-selected', 'true');
		await page.keyboard.press('Enter');
		await expect(palette).toBeHidden();
		await expect(titleOf(page)).toHaveText(/^№ 10500 · /);
	});

	test('Ctrl/⌘+K opens empty with «Недавние», also after a closed search', async ({ page }) => {
		const m = await mod(page);
		await openRow(page, FIRST);
		await page.keyboard.press('Escape');
		await expect(panelOf(page)).toBeHidden();

		await page.keyboard.press(`${m}+KeyK`);
		const palette = paletteOf(page);
		await expect(palette.getByText('Недавние')).toBeVisible();
		await expect(palette.getByRole('option', { name: new RegExp(`№ ${FIRST}\\b`) })).toBeVisible();
		await palette.getByRole('combobox').fill('иванов');
		await expect(palette.getByText('Недавние')).toHaveCount(0);
		await page.keyboard.press('Escape');
		await expect(palette).toBeHidden();

		await page.keyboard.press(`${m}+KeyK`);
		await expect(palette.getByRole('combobox')).toHaveValue('');
		await expect(palette.getByText('Недавние')).toBeVisible();
		await expect(palette.getByRole('option').first()).toHaveAttribute('aria-selected', 'true');
		await expect(palette.getByRole('option').first()).toContainText(`№ ${FIRST}`);
	});
});

test.describe('orders demo on a phone', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('390: cards and bottom navigation; a card opens the order full screen with «Назад»; the status button lists statuses from the bottom', async ({
		page
	}) => {
		await page.goto('/design/orders');
		const card = page.getByRole('button', { name: new RegExp(`^№ ${FIRST}\\b`) });
		await expect(card).toBeVisible();
		await expect(page.getByRole('grid')).toHaveCount(0);
		const bottom = page.getByRole('navigation', { name: 'Основная навигация' });
		await expect(bottom).toBeVisible();
		await expect(bottom.locator(':scope > a, :scope > button')).toHaveText([
			/Заказы/,
			/Клиенты/,
			/Принять/,
			/Скан/,
			/Ещё/
		]);

		await card.click();
		const panel = panelOf(page);
		await expect(titleOf(page)).toHaveText(new RegExp(`^№ ${FIRST} · `));
		await expect.poll(() => panel.boundingBox()).toEqual({ x: 0, y: 0, width: 390, height: 844 });
		await expect(panel.getByRole('button', { name: 'Назад' })).toBeVisible();

		const button = panel.getByRole('button', { name: /^Статус/ });
		await expect(button).toContainText('Диагностика');
		await button.click();
		const list = page.getByRole('dialog', { name: 'Статус' });
		await expect(list).toBeVisible();
		await expect
			.poll(async () => {
				const box = await list.boundingBox();
				return box ? { top: box.y > 200, bottom: Math.round(box.y + box.height) } : null;
			})
			.toEqual({ top: true, bottom: 844 });

		await list.getByRole('button', { name: 'Готов', exact: true }).click();
		await expect(list).toBeHidden();
		await expect(button).toContainText('Готов');
		await expect(toastsOf(page)).toContainText(`№ ${FIRST} → Готов`);
	});

	test('390: «Выдан» from the status list opens «Выдать»; «⋯» at the top gives an issued order back to work', async ({
		page
	}) => {
		await page.goto('/design/orders');
		await page.getByRole('button', { name: new RegExp(`^№ ${FIRST}\\b`) }).click();
		const panel = panelOf(page);
		const button = panel.getByRole('button', { name: /^Статус/ });
		await button.click();
		await page
			.getByRole('dialog', { name: 'Статус' })
			.getByRole('button', { name: 'Выдан', exact: true })
			.click();
		const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${FIRST}` });
		await expect(dialog).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
		await expect(button).toContainText('Диагностика');

		await button.click();
		await page
			.getByRole('dialog', { name: 'Статус' })
			.getByRole('button', { name: 'Выдан', exact: true })
			.click();
		await expect(dialog).toBeVisible();
		await dialog.getByRole('textbox', { name: /^Выполненные работы/ }).fill('Замена разъёма');
		await dialog.getByRole('combobox', { name: /^Мастер/ }).click();
		await page.getByRole('option', { name: 'Антон' }).click();
		await dialog.getByRole('textbox', { name: /^Итоговая цена/ }).fill('3500');
		await dialog.getByRole('button', { name: /^Выдать/ }).click();
		await expect(dialog).toBeHidden();
		await expect(button).toContainText('Выдан');
		await expect(button).toBeDisabled();
		await expect(toastsOf(page)).toContainText(`№ ${FIRST} → Выдан`);

		await panel.getByRole('button', { name: 'Ещё действия' }).click();
		await page.getByRole('menuitem', { name: 'Вернуть в работу' }).click();
		await expect(button).toContainText('Принят в работу');
		await expect(button).toBeEnabled();
		await expect(toastsOf(page)).toContainText(`№ ${FIRST} → Принят в работу`);
	});
});
