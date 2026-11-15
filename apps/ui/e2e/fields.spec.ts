import { expect, test } from '@playwright/test';

test.describe('fields', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design');
	});

	test('combobox filters case-insensitively and with ё/е', async ({ page }) => {
		const brand = page.getByRole('combobox', { name: 'Бренд' });
		await brand.fill('sam');
		await expect(page.getByRole('option')).toHaveCount(1);
		await expect(page.getByRole('option', { name: /Samsung/ })).toBeVisible();
		await page.keyboard.press('Enter');
		await expect(brand).toHaveValue('Samsung');

		await brand.fill('емк');
		await expect(page.getByRole('option', { name: /Ёмкость-тест/ })).toBeVisible();
	});

	test('combobox keeps a custom value on Tab', async ({ page }) => {
		const brand = page.getByRole('combobox', { name: 'Бренд' });
		await brand.fill('Nokia');
		await page.keyboard.press('Tab');
		await expect(brand).toHaveValue('Nokia');
	});

	test('select opens by click, picks with arrows and Enter, Esc cancels', async ({ page }) => {
		const select = page.getByRole('combobox', { name: 'Тип устройства' });
		await expect(select).toContainText('Выберите');
		await select.click();
		await expect(page.getByRole('listbox')).toBeVisible();
		await page.keyboard.press('ArrowDown');
		await page.keyboard.press('Enter');
		await expect(page.getByRole('listbox')).toBeHidden();
		await expect(select).not.toContainText('Выберите');
		const chosen = await select.textContent();
		await select.click();
		await page.keyboard.press('ArrowDown');
		await page.keyboard.press('Escape');
		await expect(page.getByRole('listbox')).toBeHidden();
		await expect(select).toHaveText(chosen ?? '');
	});

	test('segmented control arrows change the selection', async ({ page }) => {
		const group = page.getByRole('radiogroup', { name: 'Срочность' });
		const first = group.getByRole('radio').first();
		await first.focus();
		await page.keyboard.press('ArrowRight');
		await expect(group.getByRole('radio').nth(1)).toBeChecked();
	});

	test('dark theme from the header persists', async ({ page }) => {
		await page.getByRole('radio', { name: 'Тёмная' }).check({ force: true });
		await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
		await page.reload();
		await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	});

	test('date input: calendar pick fills dd.mm.yyyy', async ({ page }) => {
		const date = page.getByRole('textbox', { name: 'Дата приёма' });
		const field = page.locator('div.field').filter({ has: date });
		await field.getByRole('button', { name: 'Открыть календарь' }).click();
		await page.getByRole('gridcell').getByText('15', { exact: true }).first().click();
		await expect(date).toHaveValue(/^15\.\d\d\.\d{4}$/);
	});
});

test('the gallery renders without console errors', async ({ page }) => {
	const errors: string[] = [];
	page.on('pageerror', (e) => errors.push(e.message));
	page.on('console', (m) => m.type() === 'error' && errors.push(m.text()));
	await page.goto('/design', { waitUntil: 'networkidle' });
	await page.getByRole('combobox', { name: 'Тип устройства' }).click();
	await page.keyboard.press('ArrowDown');
	await page.keyboard.press('Enter');
	await expect(page.getByRole('listbox')).toBeHidden();
	expect(errors).toEqual([]);
});

test.describe('coarse pointer', () => {
	test.use({ hasTouch: true, isMobile: true, viewport: { width: 390, height: 844 } });

	test('tap targets are at least 44px and inputs 16px', async ({ page }) => {
		await page.goto('/design', { waitUntil: 'networkidle' });
		expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
		const height = async (loc: ReturnType<typeof page.locator>) =>
			(await loc.first().boundingBox())?.height ?? 0;
		const calendar = page.getByRole('button', { name: 'Открыть календарь' });
		const reveal = page.getByRole('button', { name: 'Показать пароль' });
		const checkbox = page.locator('label.row').filter({ hasText: 'Согласен' });
		const segment = page.getByRole('radiogroup', { name: 'Срочность' }).locator('label');
		for (const loc of [calendar, reveal, checkbox, segment]) {
			await loc.first().scrollIntoViewIfNeeded();
			expect(await height(loc)).toBeGreaterThanOrEqual(44);
		}
		await calendar.first().click();
		const popup = page.locator('.mw-popup.cal');
		await expect(popup).toBeVisible();
		for (const loc of [
			popup.locator('.cal-nav').first(),
			popup.locator('.cal-nav').last(),
			popup.locator('.cal-day:not([data-outside-month])')
		]) {
			expect(await height(loc)).toBeGreaterThanOrEqual(44);
		}
		await page.keyboard.press('Escape');
		const font = (loc: ReturnType<typeof page.locator>) =>
			loc.first().evaluate((e) => parseFloat(getComputedStyle(e).fontSize));
		expect(await font(page.getByLabel('Обычное'))).toBeGreaterThanOrEqual(16);
		expect(
			await font(page.getByRole('combobox', { name: 'Тип устройства' }))
		).toBeGreaterThanOrEqual(16);
	});
});
