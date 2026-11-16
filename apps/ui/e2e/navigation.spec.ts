import { expect, test, type Page } from '@playwright/test';

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

test.describe('navigation', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design');
		// Hotkeys exist only after hydration: the demo's own button reacts once it has happened.
		await page
			.locator('#navigation')
			.getByRole('tab', { name: /^История/ })
			.click();
		await expect(page.getByText('Содержимое: history')).toBeVisible();
		await page
			.locator('#navigation')
			.getByRole('tab', { name: /^Сведения/ })
			.click();
	});

	test('1440: sidebar shows labels', async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		const nav = page.locator('#navigation').getByRole('navigation', { name: 'Разделы' });
		await expect(nav.getByText('Клиенты')).toBeVisible();
		await expect(nav.getByText('Требуют внимания')).toBeVisible();
		await expect.poll(async () => (await nav.boundingBox())?.width).toBe(232);
	});

	test('1440: a saved view has «✕» that removes it', async ({ page }) => {
		await page.setViewportSize({ width: 1440, height: 900 });
		const nav = page.locator('#navigation').getByRole('navigation', { name: 'Разделы' });
		await expect(nav.getByText('Мои виды')).toBeVisible();
		await expect(nav.getByRole('link', { name: 'Готовые Samsung' })).toBeVisible();
		await nav.getByRole('button', { name: 'Удалить вид' }).first().click();
		await expect(nav.getByRole('link', { name: 'Готовые Samsung' })).toHaveCount(0);
		await expect(nav.getByRole('link', { name: 'Звонки мастера' })).toBeVisible();
	});

	test('1024: icon rail with the label in a tooltip', async ({ page }) => {
		await page.setViewportSize({ width: 1024, height: 800 });
		const nav = page.locator('#navigation').getByRole('navigation', { name: 'Разделы' });
		const link = nav.getByRole('link', { name: 'Клиенты' });
		await expect(link).toBeVisible();
		await expect.poll(async () => (await nav.boundingBox())?.width).toBe(56);
		await expect(nav.getByText('Клиенты')).toBeHidden();
		await link.hover();
		await expect(page.getByRole('tooltip', { name: 'Клиенты' })).toBeVisible();
	});

	test('390: bottom navigation, no sidebar', async ({ page }) => {
		await page.setViewportSize({ width: 390, height: 800 });
		const section = page.locator('#navigation');
		await expect(section.getByRole('navigation', { name: 'Разделы' })).toBeHidden();
		const bottom = section.getByRole('navigation', { name: 'Основная навигация' });
		await expect(bottom).toBeVisible();
		await expect(bottom.getByRole('link', { name: 'Заказы' })).toBeVisible();
	});

	test('FilterChips: click selects one, Mod+click adds', async ({ page }) => {
		const chips = page.locator('#navigation').getByRole('group', { name: 'Быстрые фильтры' });
		const ready = chips.getByRole('button', { name: /^Готов/ });
		const diag = chips.getByRole('button', { name: /^Диагностика/ });
		await ready.click();
		await expect(ready).toHaveAttribute('aria-pressed', 'true');
		await expect(diag).toHaveAttribute('aria-pressed', 'false');
		await diag.click({ modifiers: [await mod(page)] });
		await expect(diag).toHaveAttribute('aria-pressed', 'true');
		await expect(ready).toHaveAttribute('aria-pressed', 'true');
	});

	test('Tabs switch content', async ({ page }) => {
		const section = page.locator('#navigation');
		await section.getByRole('tab', { name: /^История/ }).click();
		await expect(section.getByText('Содержимое: history')).toBeVisible();
	});

	test('CommandPalette: Mod+K, filter, Enter selects, Esc closes', async ({ page }) => {
		await page.keyboard.press(`${await mod(page)}+KeyK`);
		const dialog = page.getByRole('dialog', { name: 'Найти или выполнить' });
		await expect(dialog).toBeVisible();
		const input = dialog.getByRole('combobox');
		await expect(input).toBeFocused();
		await input.fill('прин');
		await expect(dialog.getByRole('option')).toHaveCount(1);
		await expect(dialog.getByRole('option', { name: /Принять устройство/ })).toBeVisible();
		await page.keyboard.press('Enter');
		await expect(dialog).toBeHidden();
		await expect(page.getByText('Выбрано: Принять устройство')).toBeVisible();

		await page.keyboard.press(`${await mod(page)}+KeyK`);
		await expect(dialog).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
	});

	test('CommandPalette: Mod+Enter passes alt', async ({ page }) => {
		const m = await mod(page);
		await page.keyboard.press(`${m}+KeyK`);
		const dialog = page.getByRole('dialog', { name: 'Найти или выполнить' });
		await dialog.getByRole('combobox').fill('прин');
		await page.keyboard.press(`${m}+Enter`);
		await expect(page.getByText('Выбрано: Принять устройство (полная)')).toBeVisible();
	});

	test('sheet opened right after navigation keeps the caret in WebKit', async ({ page }) => {
		// N in the showcase navigates within /design and opens the sheet while the navigation goes.
		await page.keyboard.press('n');
		const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
		await expect(sheet).toBeVisible();
		const phone = sheet.getByRole('textbox', { name: 'Телефон' });
		await expect(phone).toBeFocused();
		await page.keyboard.type('абв');
		await expect(phone).toHaveValue('абв');
	});

	test('ShortcutsHelp: ? opens, ? in a field does not', async ({ page }) => {
		const field = page.locator('#navigation').getByLabel('Поле для проверки');
		await field.focus();
		await page.keyboard.type('?');
		await expect(page.getByRole('dialog', { name: 'Клавиши' })).toBeHidden();
		await expect(field).toHaveValue('?');
		await field.blur();
		await page.keyboard.press('Shift+Slash');
		await expect(page.getByRole('dialog', { name: 'Клавиши' })).toBeVisible();
	});
});
