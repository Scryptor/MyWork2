import { expect, test, type Page } from '@playwright/test';
import { createOrder, uniq } from './support/api';

// «Настройки → Внешний вид»: kept on this computer, every field applies at once.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const html = (page: Page) => page.locator('html');
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

async function openAppearance(page: Page) {
	await page.goto('/orders');
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: 'Настройки' })
		.click();
	await expect(page).toHaveURL(/\/settings\/appearance$/);
}

test('«Настройки» in the menu opens «Внешний вид»', async ({ page }) => {
	await openAppearance(page);
	await expect(page.getByRole('heading', { name: 'Внешний вид' })).toBeVisible();
	await expect(page.getByText('Только на этом компьютере')).toBeVisible();
	await expect(
		page.getByRole('navigation', { name: 'Разделы настроек' }).getByRole('link', {
			name: 'Внешний вид'
		})
	).toHaveAttribute('aria-current', 'page');
});

test('theme and size apply at once and survive a reload', async ({ page }) => {
	await openAppearance(page);
	await page.getByRole('radio', { name: 'Тёмная' }).check({ force: true });
	await expect(html(page)).toHaveAttribute('data-theme', 'dark');
	await page.getByRole('radio', { name: '125 %' }).check({ force: true });
	await expect(html(page)).toHaveCSS('font-size', '20px');
	await page.reload();
	await expect(html(page)).toHaveAttribute('data-theme', 'dark');
	await expect(html(page)).toHaveCSS('font-size', '20px');
	await expect(page.getByRole('radio', { name: '125 %' })).toBeChecked();
});

test('«Авто» follows the screen and says so', async ({ page }) => {
	await openAppearance(page);
	await page.getByRole('radio', { name: '150 %' }).check({ force: true });
	await page.getByRole('radio', { name: 'Авто' }).check({ force: true });
	await expect(html(page)).toHaveCSS('font-size', '16px');
	await expect(page.getByText('Подбирается по экрану: сейчас 100 %')).toBeVisible();
});

test('size keys work in a field, stop at the end, and 0 returns to auto', async ({ page }) => {
	await page.goto('/orders');
	await page.getByRole('searchbox', { name: 'Фильтр списка' }).focus();
	const m = await mod(page);
	await page.keyboard.press(`${m}+Equal`);
	await expect(html(page)).toHaveCSS('font-size', '17.6px');
	await expect(toastsOf(page).getByText('Размер 110 %')).toBeVisible();
	await page.keyboard.press(`${m}+0`);
	await expect(html(page)).toHaveCSS('font-size', '16px');
	await expect(toastsOf(page).getByText('Размер: авто (100 %)')).toBeVisible();
	for (let i = 0; i < 5; i++) await page.keyboard.press(`${m}+Equal`);
	await expect(html(page)).toHaveCSS('font-size', '24px');
	await page.keyboard.press(`${m}+Equal`);
	await expect(html(page)).toHaveCSS('font-size', '24px');
	await page.keyboard.press(`${m}+Minus`);
	await expect(html(page)).toHaveCSS('font-size', '20px');
});

test('compact density makes table rows 28px', async ({ page, request }) => {
	const label = uniq('Плот');
	await createOrder(request, { clientName: label });
	await openAppearance(page);
	await page.getByRole('radio', { name: 'Компактная' }).check({ force: true });
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: 'Заказы' })
		.click();
	const row = page
		.getByRole('grid', { name: 'Заказы' })
		.getByRole('row')
		.filter({ hasText: label });
	await expect(row).toBeVisible();
	const box = await row.boundingBox();
	expect(Math.round(box!.height)).toBe(28);
});

test('Ctrl K «Внешний вид» opens the section', async ({ page }) => {
	await page.goto('/orders');
	await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const dialog = page.getByRole('dialog', { name: 'Найти или выполнить' });
	await dialog.getByRole('combobox').fill('Внешний вид');
	await dialog.getByRole('option', { name: /Внешний вид/ }).click();
	await expect(page).toHaveURL(/\/settings\/appearance$/);
});

test.describe('phone', () => {
	test.use({ viewport: { width: 390, height: 800 }, screen: { width: 390, height: 800 } });

	test('«Ещё» leads to «Настройки»: the list, then the section with «←»', async ({ page }) => {
		await page.goto('/orders');
		await page.getByRole('button', { name: 'Ещё', exact: true }).click();
		await page
			.getByRole('dialog', { name: 'Ещё' })
			.getByRole('link', { name: 'Настройки' })
			.click();
		await expect(page).toHaveURL(/\/settings$/);
		await page.getByRole('link', { name: 'Внешний вид' }).click();
		await page.getByRole('radio', { name: 'Тёмная' }).check({ force: true });
		await expect(html(page)).toHaveAttribute('data-theme', 'dark');
		await page.getByRole('link', { name: 'Настройки' }).click();
		await expect(page).toHaveURL(/\/settings$/);
	});

	test('«Размер» is a list: six sizes do not fit a phone in a row', async ({ page }) => {
		await page.goto('/settings/appearance');
		const size = page.getByRole('combobox', { name: 'Размер' });
		await expect(size).toContainText('Авто');
		await expect(page.getByText('Подбирается по экрану: сейчас 100 %')).toBeVisible();
		await size.click();
		await page.getByRole('option', { name: '110 %' }).click();
		await expect(html(page)).toHaveCSS('font-size', '17.6px');
		await expect(size).toContainText('110 %');
		await expect(page.getByText('Подбирается по экрану')).toHaveCount(0);
	});
});
