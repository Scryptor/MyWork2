import { expect, test } from '@playwright/test';

test('stored dark theme applies before paint', async ({ page }) => {
	await page.goto('/design');
	await page.evaluate(() => localStorage.setItem('mw.prefs', '{"theme":"dark"}'));
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('system theme follows the OS live', async ({ page }) => {
	await page.goto('/design');
	await page.evaluate(() => localStorage.setItem('mw.prefs', '{"theme":"system"}'));
	await page.emulateMedia({ colorScheme: 'dark' });
	await page.reload();
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
	await page.emulateMedia({ colorScheme: 'light' });
	await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('body uses Inter Variable', async ({ page }) => {
	await page.goto('/design');
	const family = await page.evaluate(() => getComputedStyle(document.body).fontFamily);
	// Chromium serializes the family with quotes, WebKit without.
	expect(family).toMatch(/^["']?Inter Variable["']?,/);
});
