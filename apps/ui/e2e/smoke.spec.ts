import { expect, test } from '@playwright/test';

test('the design system page opens', async ({ page }) => {
	await page.goto('/design');
	await expect(page.getByRole('heading', { level: 1 })).toHaveText('Дизайн-система');
});
