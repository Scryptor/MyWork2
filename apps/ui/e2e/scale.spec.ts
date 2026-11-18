import { expect, test } from '@playwright/test';

test.describe('4K screen, automatic size', () => {
	test.use({ screen: { width: 3840, height: 2160 }, viewport: { width: 1920, height: 1010 } });

	test('text 24px, full menu, icons grow with the text', async ({ page }) => {
		await page.goto('/orders');
		await expect(page.locator('html')).toHaveCSS('font-size', '24px');
		const link = page
			.getByRole('navigation', { name: 'Разделы' })
			.getByRole('link', { name: 'Заказы' });
		await expect(link).toBeVisible();
		await expect(link.getByText('Заказы')).toBeVisible();
		const box = await link.locator('svg').boundingBox();
		expect(Math.round(box!.width)).toBe(24);
		expect(Math.round(box!.height)).toBe(24);
	});
});

test.describe('1366x700 screen, manual 150 %', () => {
	test.use({ screen: { width: 1366, height: 700 }, viewport: { width: 1366, height: 700 } });

	test.beforeEach(async ({ page }) => {
		await page.addInitScript(() =>
			localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale: 1.5 }))
		);
	});

	test('the menu is a narrow rail and the page does not scroll sideways', async ({ page }) => {
		await page.goto('/orders');
		const nav = page.getByRole('navigation', { name: 'Разделы' });
		await expect(nav.getByRole('link', { name: 'Заказы' })).toBeVisible();
		await expect(nav.locator('.label')).toHaveCount(0);
		const fits = await page.evaluate(
			() => document.documentElement.scrollWidth <= window.innerWidth
		);
		expect(fits).toBe(true);
	});

	test('a wider window brings the full menu back without a reload', async ({ page }) => {
		await page.goto('/orders');
		const nav = page.getByRole('navigation', { name: 'Разделы' });
		await expect(nav.locator('.label')).toHaveCount(0);
		await page.setViewportSize({ width: 1920, height: 1010 });
		await expect(nav.getByText('Заказы')).toBeVisible();
		await expect(page.locator('html')).toHaveAttribute('data-width', 'xl');
	});
});
