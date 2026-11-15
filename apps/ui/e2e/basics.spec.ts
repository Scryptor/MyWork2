import { expect, test } from '@playwright/test';

test('status badge fill and text come from its hue tokens', async ({ page }) => {
	await page.goto('/design');
	const badge = page.locator('#basics .badge.st-violet').first();
	const [actual, expected] = await badge.evaluate((el) => {
		const probe = document.createElement('div');
		const root = getComputedStyle(document.documentElement);
		probe.style.background = root.getPropertyValue('--st-violet-bg');
		probe.style.color = root.getPropertyValue('--st-violet-fg');
		document.body.append(probe);
		const p = getComputedStyle(probe);
		const e = { bg: p.backgroundColor, fg: p.color };
		probe.remove();
		const c = getComputedStyle(el);
		return [{ bg: c.backgroundColor, fg: c.color }, e];
	});
	expect(actual).toEqual(expected);
});

test('an icon button opens its tooltip on keyboard focus', async ({ page }) => {
	await page.goto('/design');
	const button = page.locator('#basics').getByRole('button', { name: 'Найти' });
	await button.scrollIntoViewIfNeeded();
	await page.locator('#basics').getByRole('button', { name: 'ghost lg' }).focus();
	for (let i = 0; i < 40 && !(await button.evaluate((el) => el === document.activeElement)); i++) {
		await page.keyboard.press('Tab');
	}
	await expect(button).toBeFocused();
	await expect(page.getByRole('tooltip')).toHaveText('Найти');
});

test('a flag icon tooltip opens on hover and on keyboard focus', async ({ page }) => {
	await page.goto('/design');
	const flag = page.locator('#basics .flag').first();
	await flag.scrollIntoViewIfNeeded();
	await flag.hover();
	await expect(page.getByRole('tooltip', { name: 'Позвонить' })).toBeVisible();
	await page.mouse.move(0, 0);
	await expect(page.getByRole('tooltip')).toHaveCount(0);
	await page.locator('#basics').getByRole('button', { name: 'Добавить' }).focus();
	for (let i = 0; i < 40 && !(await flag.evaluate((el) => el === document.activeElement)); i++) {
		await page.keyboard.press('Tab');
	}
	await expect(flag).toBeFocused();
	await expect(page.getByRole('tooltip', { name: 'Позвонить' })).toBeVisible();
});
