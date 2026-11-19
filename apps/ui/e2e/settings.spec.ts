import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import type { AppSettings } from '$api/AppSettings';
import type { AboutInfo } from '$api/AboutInfo';
import type { ServerInfo } from '$api/ServerInfo';
import { createOrder, E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, uniq } from './support/api';

// «Настройки»: «Внешний вид» is kept on this computer and applies at once; «Общие», «Заказы» and «О программе»
// are the server's and are saved as they are edited.

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
	await expect(page).toHaveURL(/\/settings\/general$/);
	await page
		.getByRole('navigation', { name: 'Разделы настроек' })
		.getByRole('link', { name: 'Внешний вид' })
		.click();
	await expect(page).toHaveURL(/\/settings\/appearance$/);
}

test('«Настройки» opened by its address goes on to «Общие»', async ({ page }) => {
	await page.goto('/settings');
	await expect(page).toHaveURL(/\/settings\/general$/);
	await expect(page.getByRole('heading', { name: 'Общие' })).toBeVisible();
	// Replaced, not pushed: «Назад» does not return to the address that only redirects.
	await page.goto('/orders');
	await page.goto('/settings');
	await expect(page).toHaveURL(/\/settings\/general$/);
	await page.goBack();
	await expect(page).toHaveURL(/\/orders$/);
});

test('«Настройки» in the menu opens «Общие»; the subsections are in order', async ({ page }) => {
	await page.goto('/orders');
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: 'Настройки' })
		.click();
	const sections = page.getByRole('navigation', { name: 'Разделы настроек' });
	await expect(sections.getByRole('link')).toHaveText([
		'Общие',
		'Внешний вид',
		'Заказы',
		'О программе'
	]);
	await expect(sections.getByRole('link', { name: 'Общие' })).toHaveAttribute(
		'aria-current',
		'page'
	);
});

test('«Настройки» opens «Внешний вид» from its subsection', async ({ page }) => {
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

test('size keys work over the open intake, in its phone field; Shift does not matter', async ({
	page
}) => {
	await page.goto('/orders');
	await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
	await page.keyboard.press('n');
	const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
	const phone = sheet.getByRole('textbox', { name: 'Телефон' });
	await expect(phone).toBeFocused();
	const m = await mod(page);
	await page.keyboard.press(`${m}+Equal`);
	await expect(html(page)).toHaveCSS('font-size', '17.6px');
	await expect(toastsOf(page).getByText('Размер 110 %')).toBeVisible();
	await page.keyboard.press(`${m}+Shift+Minus`);
	await expect(html(page)).toHaveCSS('font-size', '16px');
	// The new size toast replaces the old one.
	await expect(toastsOf(page).getByText('Размер 100 %')).toBeVisible();
	await expect(toastsOf(page).getByText('Размер 110 %')).toHaveCount(0);
	await expect(sheet).toBeVisible();
	await expect(phone).toBeFocused();
	await expect(phone).toHaveValue('');
});

test.describe('4K', () => {
	test.use({ screen: { width: 3840, height: 2160 }, viewport: { width: 1920, height: 1010 } });

	test('at the largest size «Авто» stays «Авто»', async ({ page }) => {
		await openAppearance(page);
		await expect(page.getByText('Подбирается по экрану: сейчас 150 %')).toBeVisible();
		await expect(html(page)).toHaveCSS('font-size', '24px');
		await page.keyboard.press(`${await mod(page)}+Equal`);
		await expect(toastsOf(page).getByText('Размер: авто (150 %)')).toBeVisible();
		await expect(html(page)).toHaveCSS('font-size', '24px');
		await expect(page.getByRole('radio', { name: 'Авто' })).toBeChecked();
	});
});

test('compact density makes table rows 28px', async ({ page, request }) => {
	const label = uniq('Плот');
	await createOrder(request, { clientName: label });
	await openAppearance(page);
	await page.getByRole('radio', { name: 'Компактная' }).check({ force: true });
	await page
		.getByRole('navigation', { name: 'Разделы', exact: true })
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

	test('a phone widened on «Настройки» lands on «Общие»', async ({ page }) => {
		await page.goto('/settings');
		await expect(
			page.getByRole('navigation', { name: 'Разделы настроек' }).getByRole('link', {
				name: 'Внешний вид'
			})
		).toBeVisible();
		await expect(page).toHaveURL(/\/settings$/);
		await page.setViewportSize({ width: 1280, height: 720 });
		await expect(page).toHaveURL(/\/settings\/general$/);
		await expect(page.getByRole('heading', { name: 'Общие' })).toBeVisible();
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

// The server's settings: shared by every test on the server, so each test puts back what it changed.
const settingsSaved = (page: Page) =>
	page.waitForResponse(
		(r) => r.url().endsWith('/api/v1/settings') && r.request().method() === 'PATCH'
	);

async function currentSettings(request: APIRequestContext): Promise<AppSettings> {
	const response = await request.get('/api/v1/settings');
	expect(response.ok()).toBe(true);
	return (await response.json()) as AppSettings;
}

test.describe('server settings', () => {
	test('«Название сервиса» is saved as it is typed and survives a reload', async ({
		page,
		request
	}) => {
		const before = (await currentSettings(request)).firm_name;
		const name = uniq('Сервис');
		try {
			await page.goto('/settings/general');
			const field = page.getByRole('textbox', { name: 'Название сервиса' });
			await expect(field).toBeEnabled();
			const saved = settingsSaved(page);
			await field.fill(name);
			expect((await saved).ok()).toBe(true);
			await expect(page.getByRole('status').filter({ hasText: 'Сохранено' })).toBeVisible();
			await page.reload();
			await expect(page.getByRole('textbox', { name: 'Название сервиса' })).toHaveValue(name);
		} finally {
			await request.patch('/api/v1/settings', { data: { firm_name: before } });
		}
	});

	test('«Долгая диагностика, дней» 0 is a mistake under the field and is not kept', async ({
		page,
		request
	}) => {
		const before = (await currentSettings(request)).diagnostics_overdue_days;
		await page.goto('/settings/orders');
		const field = page.getByRole('textbox', { name: 'Долгая диагностика, дней' });
		await expect(field).toHaveValue(String(before));
		await field.fill('0');
		await expect(page.getByText('От 1 до 365')).toBeVisible();
		await expect(field).toHaveAttribute('aria-invalid', 'true');
		await page.reload();
		await expect(page.getByRole('textbox', { name: 'Долгая диагностика, дней' })).toHaveValue(
			String(before)
		);
		expect((await currentSettings(request)).diagnostics_overdue_days).toBe(before);
	});

	test('an empty «Валюта» is «Обязательное поле» and the currency stays', async ({
		page,
		request
	}) => {
		const before = (await currentSettings(request)).currency_symbol;
		await page.goto('/settings/general');
		const field = page.getByRole('textbox', { name: 'Валюта' });
		await expect(field).toHaveValue(before);
		await field.fill('');
		await expect(page.getByText('Обязательное поле')).toBeVisible();
		await page.reload();
		await expect(page.getByRole('textbox', { name: 'Валюта' })).toHaveValue(before);
	});

	test('«Следующий номер заказа» gives the next order its number; a number in use is refused', async ({
		page,
		request
	}) => {
		const response = await request.get('/api/v1/orders/next-number');
		const current = ((await response.json()) as { next: number }).next;
		// Both engines run this on one server: the second one must ask for more than the first got.
		const wanted = Math.max(900000, current + 1000);
		await page.goto('/settings/general');
		const field = page.getByRole('textbox', { name: 'Следующий номер заказа' });
		await expect(field).toHaveValue(/^\d+$/);
		await field.fill(String(wanted));
		await field.press('Enter');
		await expect(page.getByText('Номер должен быть больше')).toHaveCount(0);
		await expect(async () => {
			const now = await request.get('/api/v1/orders/next-number');
			expect(((await now.json()) as { next: number }).next).toBeGreaterThanOrEqual(wanted);
		}).toPass();
		const order = await createOrder(request, { clientName: uniq('Номер') });
		expect(order.id).toBeGreaterThanOrEqual(wanted);

		// An order has that number already: the server refuses, the field is as it was.
		const shown = await field.inputValue();
		await field.fill('1');
		await field.blur();
		await expect(
			page.getByText('Номер должен быть больше номеров существующих заказов')
		).toBeVisible();
		await expect(field).toHaveValue(shown);
	});

	test('«О программе» shows the version the server reports', async ({ page, request }) => {
		const info = (await (await request.get('/api/v1/server-info')).json()) as ServerInfo;
		const about = (await (await request.get('/api/v1/about')).json()) as AboutInfo;
		await page.goto('/settings/about');
		await expect(page.getByRole('heading', { name: 'О программе' })).toBeVisible();
		await expect(page.getByText(info.version, { exact: true })).toBeVisible();
		await expect(page.getByText(about.database, { exact: true })).toBeVisible();
		await expect(page.getByRole('button', { name: 'Скопировать' })).toBeVisible();
	});

	test('«Заказы»: the default point offers «Не выбрана» and the live points', async ({ page }) => {
		await page.goto('/settings/orders');
		const point = page.getByRole('combobox', { name: 'Точка по умолчанию' });
		await expect(point).toBeEnabled();
		await point.click();
		await expect(page.getByRole('option', { name: 'Не выбрана' })).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(
			page.getByRole('switch', { name: 'Проверять обязательные поля при приёме' })
		).toBeEnabled();
	});
});

test.describe('signed in as the administrator', () => {
	test.use({ baseURL: 'http://127.0.0.1:4174' });

	test('the administrator may change the settings: the fields are enabled', async ({ page }) => {
		await page.goto('/login');
		await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
		await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
		await page.getByRole('button', { name: 'Войти' }).click();
		await expect(page).toHaveURL(/\/orders/);
		await page.goto('/settings/general');
		await expect(page.getByRole('textbox', { name: 'Название сервиса' })).toBeEnabled();
		await expect(page.getByText('Изменять настройки может администратор')).toHaveCount(0);
		await page.goto('/settings/orders');
		await expect(page.getByRole('textbox', { name: 'Долгая диагностика, дней' })).toBeEnabled();
	});
});
