import { expect, test } from '@playwright/test';

test('a row is dragged by its handle and the swatches pick with arrows', async ({ page }) => {
	await page.goto('/design');
	const demo = page.getByTestId('sortable-demo');
	const rows = demo.locator('li');
	await rows.first().scrollIntoViewIfNeeded();
	await expect(rows.first()).toContainText('Принят');
	const from = await demo.getByRole('button', { name: 'Переместить „Принят“' }).boundingBox();
	const to = await rows.nth(2).boundingBox();
	if (!from || !to) throw new Error('no boxes');
	await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
	await page.mouse.down();
	await page.mouse.move(from.x + from.width / 2, to.y + to.height * 0.8, { steps: 8 });
	await page.mouse.up();
	await expect(rows.nth(2)).toContainText('Принят');
	await expect(rows.first()).toContainText('Диагностика');

	await demo.getByRole('button', { name: 'Переместить „Принят“' }).focus();
	await page.keyboard.press('ArrowDown');
	await expect(rows.nth(3)).toContainText('Принят');
	await expect(page.getByText('„Принят“ — место 4 из 4')).toBeAttached();

	const swatches = page.getByRole('radiogroup', { name: 'Цвет статуса' });
	await swatches.getByRole('radio', { name: 'Фиолетовый' }).focus();
	await page.keyboard.press('ArrowRight');
	await expect(swatches.getByRole('radio', { name: 'Розовый' })).toBeChecked();
});
