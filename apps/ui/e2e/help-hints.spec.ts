import { expect, test } from '@playwright/test';

// «Что это?»: the icon next to a label opens a small window; Esc returns focus to the icon, and while the
// window is open the one-key hotkeys do nothing.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

test('the hint of «Долгая диагностика, дней» opens, closes with Esc and returns focus', async ({
	page
}) => {
	await page.goto('/settings/orders');
	const icon = page.getByRole('button', { name: 'Что такое «Долгая диагностика, дней»' });
	await expect(icon).toBeVisible();
	await icon.click();
	const dialog = page.getByRole('dialog', { name: 'Долгая диагностика, дней' });
	await expect(dialog).toBeVisible();
	await expect(dialog).toContainText('Диагностика');
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();
	await expect(icon).toBeFocused();
	// The icon is outside the label: the field did not get the focus.
	await expect(page.getByRole('textbox', { name: 'Долгая диагностика, дней' })).not.toBeFocused();
});

test('«Понятно» closes the hint', async ({ page }) => {
	await page.goto('/settings/orders');
	await page.getByRole('button', { name: 'Что такое «Давно готов, дней»' }).click();
	const dialog = page.getByRole('dialog', { name: 'Давно готов, дней' });
	await expect(dialog).toBeVisible();
	await dialog.getByRole('button', { name: 'Понятно' }).click();
	await expect(dialog).toBeHidden();
});

test('the key N does not open the intake while a hint is open', async ({ page }) => {
	await page.goto('/settings/orders');
	await page.getByRole('button', { name: 'Что такое «Долгая диагностика, дней»' }).click();
	await expect(page.getByRole('dialog', { name: 'Долгая диагностика, дней' })).toBeVisible();
	await page.keyboard.press('n');
	await expect(page.getByRole('dialog', { name: 'Долгая диагностика, дней' })).toBeVisible();
	await expect(page.getByRole('dialog', { name: 'Новый заказ' })).toHaveCount(0);
	await page.keyboard.press('Escape');
	await expect(page.getByRole('dialog', { name: 'Долгая диагностика, дней' })).toBeHidden();
	// With the hint closed the same key works (the scan gap of the first press has passed).
	await page.waitForTimeout(500);
	await page.keyboard.press('n');
	await expect(page.getByRole('dialog', { name: 'Новый заказ' })).toBeVisible();
});

test('the sidebar «Требуют внимания» explains its four rules', async ({ page }) => {
	await page.goto('/orders');
	const icon = page.getByRole('button', { name: 'Что такое «Требуют внимания»' });
	await icon.click();
	const dialog = page.getByRole('dialog', { name: 'Требуют внимания' });
	for (const rule of ['Позвонить', 'Заказать запчасть', 'Долгая диагностика', 'Давно готовы']) {
		await expect(dialog).toContainText(rule);
	}
	await page.keyboard.press('Escape');
	await expect(icon).toBeFocused();
});

for (const [name, path] of [
	['Общие', '/settings/general'],
	['Внешний вид', '/settings/appearance'],
	['Заказы', '/settings/orders'],
	['Точки', '/settings/points'],
	['Справочники', '/settings/dictionaries']
] as const) {
	test(`«${name}»: the hints do not widen the page at 360 px and at 150 %`, async ({ page }) => {
		await page.addInitScript(() =>
			localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale: 1.5 }))
		);
		for (const width of [360, 1280]) {
			await page.setViewportSize({ width, height: 800 });
			await page.goto(path);
			await expect(page.getByRole('button', { name: /^Что такое/ }).first()).toBeVisible();
			const fits = await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth
			);
			expect(fits, `${path} at ${width}`).toBe(true);
		}
	});
}
