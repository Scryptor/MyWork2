import { expect, test } from '@playwright/test';

test('the root redirects to the design system page', async ({ page }) => {
	await page.goto('/');
	await expect(page).toHaveURL(/\/design$/);
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('Дизайн-система');
});
