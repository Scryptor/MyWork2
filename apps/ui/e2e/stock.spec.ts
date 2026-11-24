import { expect, test, type Browser, type Page } from '@playwright/test';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, sessionToken, uniq } from './support/api';

// «Склад» on the server with an administrator (4174): a position is added with the button and with the
// shortcut, found without regard to case, filtered by «Докупить»; a user without «Просмотр склада» has no
// section and no request of the list. Names are unique: the server is shared.

const ORIGIN = 'http://127.0.0.1:4174';
test.use({
	baseURL: ORIGIN,
	screen: { width: 1280, height: 720 },
	viewport: { width: 1280, height: 720 }
});

/** Test password of the user made here (fictional). */
const USER_PASSWORD = 'e2e-stock-password';

async function signInAdmin(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

async function headers(page: Page) {
	return { Authorization: `Bearer ${(await sessionToken(page)) ?? ''}` };
}

for (const how of ['the button', 'Mod+Enter'] as const) {
	test(`a stock item is added with ${how}, found and filtered`, async ({ page }) => {
		const id = uniq('s');
		const type = `Дисплей-${id}`;
		const model = `M-${id}`;
		await signInAdmin(page);

		await page
			.getByRole('navigation', { name: 'Разделы' })
			.getByRole('link', { name: /^Склад/ })
			.click();
		await expect(page).toHaveURL(/\/stock/);
		await page.getByRole('button', { name: 'Добавить позицию' }).click();
		const dialog = page.getByRole('dialog', { name: 'Новая позиция' });
		await dialog.getByRole('combobox', { name: /^Тип запчасти/ }).fill(type);
		await dialog.getByRole('textbox', { name: 'Модель' }).fill(model);
		await dialog.getByRole('textbox', { name: 'Количество' }).fill('3');
		await dialog.getByRole('textbox', { name: 'Цена' }).fill('2500');
		await dialog.getByRole('textbox', { name: 'Докупить при' }).fill('5');
		if (how === 'the button')
			await dialog.getByRole('button', { name: 'Добавить', exact: true }).click();
		else await page.keyboard.press(`${await mod(page)}+Enter`);

		await expect(dialog).toBeHidden();
		// The panel of the new position opens: its title is the name made of the fields.
		const panel = page.getByRole('dialog', { name: `${type} ${model}` });
		await expect(panel).toBeVisible();
		await expect(page).toHaveURL(/item=\d+/);
		await page.keyboard.press('Escape');
		await expect(panel).toBeHidden();

		// Found by lower case; the row has the quantity (3 is below 5: «Докупить») and the price.
		const grid = page.getByRole('grid', { name: 'Склад' });
		await page.getByRole('searchbox', { name: 'Фильтр списка' }).fill(`m-${id}`);
		const row = grid.getByRole('row').filter({ hasText: model });
		await expect(row).toHaveCount(1);
		await expect(row.getByRole('gridcell', { name: /^3\s+Докупить$/ })).toBeVisible();
		await expect(row.getByRole('gridcell', { name: /^2\s500\s₽$/ })).toBeVisible();

		// A position that has enough in stock: through the API.
		const other = `N-${id}`;
		const made = await page.request.post('/api/v1/stock', {
			headers: await headers(page),
			data: {
				name: '',
				device_type: '',
				part_type: type,
				brand: '',
				model: other,
				color: '',
				note: '',
				quantity: 9,
				price: 100000,
				restock_threshold: 5
			}
		});
		expect(made.ok(), await made.text()).toBe(true);
		await page.getByRole('searchbox', { name: 'Фильтр списка' }).fill(type);
		await expect(grid.getByRole('row').filter({ hasText: model })).toHaveCount(1);
		await expect(grid.getByRole('row').filter({ hasText: other })).toHaveCount(1);

		const chip = page.getByRole('button', { name: 'Докупить', exact: true });
		await chip.click();
		await expect(page).toHaveURL(/restock=1/);
		await expect(chip).toHaveAttribute('aria-pressed', 'true');
		await expect(grid.getByRole('row').filter({ hasText: other })).toHaveCount(0);
		await expect(grid.getByRole('row').filter({ hasText: model })).toHaveCount(1);

		await page.reload();
		await expect(page.getByRole('button', { name: 'Докупить', exact: true })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		await expect(
			page.getByRole('grid', { name: 'Склад' }).getByRole('row').filter({ hasText: model })
		).toHaveCount(1);
	});
}

async function signInAs(browser: Browser, login: string) {
	const context = await browser.newContext({ baseURL: ORIGIN });
	const page = await context.newPage();
	await page.goto('/login');
	await page.getByLabel('Логин').fill(login);
	await page.getByLabel('Пароль').fill(USER_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
	return { page, context };
}

test('without stock_view there is no «Склад»', async ({ page, browser }) => {
	await signInAdmin(page);
	const auth = await headers(page);
	const role = await page.request.post('/api/v1/roles', {
		headers: auth,
		data: { name: uniq('Роль'), permissions: ['orders_create', 'orders_edit'] }
	});
	expect(role.ok(), await role.text()).toBe(true);
	const roleId = ((await role.json()) as { id: number }).id;
	const login = uniq('user');
	const made = await page.request.post('/api/v1/users', {
		headers: auth,
		data: {
			display_name: login,
			login,
			password: USER_PASSWORD,
			role_id: roleId,
			can_sign_in: true,
			is_master: false
		}
	});
	expect(made.ok(), await made.text()).toBe(true);

	const user = await signInAs(browser, login);
	try {
		const asked: string[] = [];
		user.page.on('request', (r) => {
			const path = new URL(r.url()).pathname;
			if (path === '/api/v1/stock' || path.startsWith('/api/v1/stock/')) asked.push(path);
		});
		const nav = user.page.getByRole('navigation', { name: 'Разделы' });
		await expect(nav.getByRole('link', { name: 'Заказы' })).toBeVisible();
		await expect(nav.getByRole('link', { name: /^Склад/ })).toHaveCount(0);

		await user.page.goto('/stock');
		await expect(user.page.getByText('Нет доступа к складу')).toBeVisible();
		await expect(user.page.getByText('Нужно право «Просмотр склада»')).toBeVisible();
		await expect(user.page.getByRole('button', { name: 'Добавить позицию' })).toHaveCount(0);
		expect(asked).toEqual([]);
	} finally {
		await user.context.close();
	}
});
