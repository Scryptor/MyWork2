import { expect, test, type Browser, type Page } from '@playwright/test';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, sessionToken, tinyPng, uniq } from './support/api';

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

/** A user with exactly these rights (and no others): its login. */
async function makeUser(page: Page, permissions: string[]): Promise<string> {
	const auth = await headers(page);
	const role = await page.request.post('/api/v1/roles', {
		headers: auth,
		data: { name: uniq('Роль'), permissions }
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
	return login;
}

/** A position made through the API; `marker` makes its name unique on the shared server. */
async function makeItem(page: Page, marker: string, over: Record<string, unknown> = {}) {
	const made = await page.request.post('/api/v1/stock', {
		headers: await headers(page),
		data: {
			name: '',
			device_type: '',
			part_type: `Дисплей-${marker}`,
			brand: '',
			model: `M-${marker}`,
			color: '',
			note: '',
			quantity: 5,
			price: 100000,
			restock_threshold: 0,
			...over
		}
	});
	expect(made.ok(), await made.text()).toBe(true);
	return (await made.json()) as { id: number; display_name: string };
}

const panelOf = (page: Page, name: string | RegExp) => page.getByRole('dialog', { name });
const stockLink = (page: Page) =>
	page.getByRole('navigation', { name: 'Разделы' }).getByRole('link', { name: /^Склад/ });

test('without stock_view there is no «Склад»', async ({ page, browser }) => {
	await signInAdmin(page);
	const login = await makeUser(page, ['orders_create', 'orders_edit']);

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

test('the panel edits an item and keeps it after reload', async ({ page }) => {
	const id = uniq('p');
	await signInAdmin(page);
	const item = await makeItem(page, id, { name: `Тест ${id}`, quantity: 5, restock_threshold: 0 });
	await page.goto(`/stock?item=${item.id}`);
	const panel = panelOf(page, `Тест ${id}`);
	await expect(panel).toBeVisible();
	await expect(panel.getByText(`Арт. ${item.id}`)).toBeVisible();

	// The field turns «1 000» into «1000» when it gets focus: select the text after that, then type.
	const price = panel.getByRole('textbox', { name: 'Цена' });
	await price.click();
	await expect(price).toHaveValue('1000');
	await price.press('ControlOrMeta+a');
	await price.pressSequentially('3000');
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	await page.reload();
	await expect(panelOf(page, `Тест ${id}`).getByRole('textbox', { name: 'Цена' })).toHaveValue(
		/^3\s000/
	);

	// The name cleared: the title is the name made of the other fields.
	const again = panelOf(page, `Тест ${id}`);
	await again.getByRole('textbox', { name: 'Наименование' }).fill('');
	await expect(panelOf(page, `Дисплей-${id} M-${id}`)).toBeVisible();
	await expect(panelOf(page, `Дисплей-${id} M-${id}`).getByText('✓ Сохранено')).toBeVisible();

	// The history says what changed.
	await expect(panelOf(page, `Дисплей-${id} M-${id}`).getByText('Изменено: цена')).toBeVisible();
	const row = await page.request.get(`/api/v1/stock/${item.id}`, { headers: await headers(page) });
	expect(((await row.json()) as { price: number }).price).toBe(300000);
});

test('«Докупить» in the sidebar follows an edit of the panel', async ({ page }) => {
	const id = uniq('c');
	await signInAdmin(page);
	const item = await makeItem(page, id, { quantity: 5, restock_threshold: 0 });
	await page.goto(`/stock?item=${item.id}`);
	const panel = panelOf(page, `Дисплей-${id} M-${id}`);
	await expect(panel).toBeVisible();
	const count = async () =>
		Number(((await stockLink(page).textContent()) ?? '').replace(/\D/g, '') || 0);
	await expect.poll(count).toBeGreaterThanOrEqual(0);
	const before = await count();
	await panel.getByRole('textbox', { name: 'Докупить при' }).fill('9');
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	// Other tests change the shared counter too: it is at least one more.
	await expect.poll(count).toBeGreaterThan(before);
});

test('photos: three are kept, the fourth is refused', async ({ page }) => {
	const id = uniq('f');
	await signInAdmin(page);
	const item = await makeItem(page, id);
	await page.goto(`/stock?item=${item.id}`);
	const panel = panelOf(page, `Дисплей-${id} M-${id}`);
	await expect(panel).toBeVisible();
	const photos = panel.getByRole('group', { name: 'Фото', exact: true });
	await panel
		.getByRole('group', { name: 'Область для фото' })
		.locator('input[type=file]')
		.setInputFiles(
			[1, 2, 3, 4].map((n) => ({ name: `p${n}.png`, mimeType: 'image/png', buffer: tinyPng(n) }))
		);
	await expect(photos.getByRole('button', { name: /^Открыть фото/ })).toHaveCount(3);
	await expect(
		page.getByRole('status', { name: 'Уведомления' }).getByText('У позиции не больше 3 фото')
	).toBeVisible();
	await expect(panel.getByRole('button', { name: 'Добавить фото' })).toHaveCount(0);
	const list = await page.request.get(`/api/v1/stock/${item.id}/photos`, {
		headers: await headers(page)
	});
	expect(await list.json()).toHaveLength(3);
});

test('an item is deleted', async ({ page }) => {
	const id = uniq('d');
	await signInAdmin(page);
	const item = await makeItem(page, id);
	await page.goto(`/stock?q=${encodeURIComponent(`M-${id}`)}&item=${item.id}`);
	const panel = panelOf(page, `Дисплей-${id} M-${id}`);
	await expect(panel).toBeVisible();
	const grid = page.getByRole('grid', { name: 'Склад' });
	await expect(grid.getByRole('row').filter({ hasText: `M-${id}` })).toHaveCount(1);

	await panel.getByRole('button', { name: 'Ещё действия' }).click();
	await page.getByRole('menuitem', { name: 'Удалить позицию' }).click();
	const confirm = page.getByRole('alertdialog');
	await expect(confirm.getByText('Списанные в заказы запчасти останутся в заказах.')).toBeVisible();
	await confirm.getByRole('button', { name: 'Удалить позицию' }).click();

	await expect(panel).toBeHidden();
	await expect(grid.getByRole('row').filter({ hasText: `M-${id}` })).toHaveCount(0);
	const gone = await page.request.get(`/api/v1/stock/${item.id}`, { headers: await headers(page) });
	expect(gone.status()).toBe(404);
});

test('readonly without stock_edit', async ({ page, browser }) => {
	const id = uniq('r');
	await signInAdmin(page);
	const item = await makeItem(page, id, { name: `Чтение ${id}` });
	const login = await makeUser(page, ['stock_view']);
	const user = await signInAs(browser, login);
	try {
		await user.page.goto(`/stock?item=${item.id}`);
		const panel = panelOf(user.page, `Чтение ${id}`);
		await expect(panel).toBeVisible();
		await expect(panel.getByRole('textbox', { name: 'Цена' })).toBeDisabled();
		await expect(panel.getByRole('textbox', { name: 'Наименование' })).toBeDisabled();
		await expect(panel.getByRole('textbox', { name: 'Количество' })).toBeDisabled();
		await expect(panel.getByRole('button', { name: 'Добавить фото' })).toHaveCount(0);
		await expect(panel.getByRole('button', { name: 'Ещё действия' })).toHaveCount(0);
		await expect(user.page.getByRole('button', { name: 'Удалить позицию' })).toHaveCount(0);
	} finally {
		await user.context.close();
	}
});
