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

test('Lucide icons are drawn with a 1.75 stroke (spec 3.4)', async ({ page }) => {
	await page.goto('/design');
	const icon = page.locator('#basics svg.lucide').first();
	await expect(icon).toBeAttached();
	const widths = await icon.evaluate((svg) =>
		[svg, ...svg.querySelectorAll('*')].map((el) => parseFloat(getComputedStyle(el).strokeWidth))
	);
	expect([...new Set(widths)]).toEqual([1.75]);
});

test('the spinner stands still when the system asks for less motion (spec 10)', async ({
	page
}) => {
	await page.emulateMedia({ reducedMotion: 'reduce' });
	await page.goto('/design');
	const ring = page.locator('#basics .spinner .ring').first();
	await expect(ring).toBeAttached();
	const animation = () => ring.evaluate((el) => getComputedStyle(el).animationName);
	expect(await animation()).toBe('none');
	await page.emulateMedia({ reducedMotion: 'no-preference' });
	expect(await animation()).not.toBe('none');
});
