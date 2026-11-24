import {
	expect,
	request as playwrightRequest,
	test,
	type APIRequestContext,
	type Browser,
	type Page
} from '@playwright/test';
import {
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	createOrder,
	patchOrder,
	sessionToken,
	uniq
} from './support/api';

// «Запчасти» of an order on the server with an administrator (4174): written off from the stock in the panel
// and given back, the last unit taken meanwhile, the full card, a user who may edit orders but not see the
// stock. Names are unique: the server is shared.

const ORIGIN = 'http://127.0.0.1:4174';
test.use({
	baseURL: ORIGIN,
	screen: { width: 1280, height: 720 },
	viewport: { width: 1280, height: 720 }
});

const USER_PASSWORD = 'e2e-parts-password';

async function signInAdmin(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

/** An API context that acts as the signed-in administrator of the page. */
async function api(page: Page): Promise<APIRequestContext> {
	return playwrightRequest.newContext({
		baseURL: ORIGIN,
		extraHTTPHeaders: { Authorization: `Bearer ${(await sessionToken(page)) ?? ''}` }
	});
}

async function makeItem(request: APIRequestContext, name: string, quantity: number) {
	const made = await request.post('/api/v1/stock', {
		data: {
			name,
			device_type: '',
			part_type: '',
			brand: '',
			model: '',
			color: '',
			note: '',
			quantity,
			price: 100_000,
			restock_threshold: 0
		}
	});
	expect(made.ok(), await made.text()).toBe(true);
	return (await made.json()) as { id: number };
}

async function itemQuantity(request: APIRequestContext, id: number): Promise<number> {
	const row = await request.get(`/api/v1/stock/${id}`);
	return ((await row.json()) as { quantity: number }).quantity;
}

async function writeOff(request: APIRequestContext, orderId: number, itemId: number, n: number) {
	const made = await request.post(`/api/v1/orders/${orderId}/parts`, {
		data: { stock_item_id: itemId, quantity: n, request_key: uniq('k') }
	});
	expect(made.ok(), await made.text()).toBe(true);
}

/** The order panel of an order found by its unique model. */
async function openPanel(page: Page, model: string, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(model)}&chips=all`);
	await page.getByRole('gridcell', { name: String(id), exact: true }).click();
	const panel = page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	return panel;
}

test('a part is written off and given back', async ({ page }) => {
	const id = uniq('p');
	await signInAdmin(page);
	const request = await api(page);
	const model = uniq('Модель');
	const item = await makeItem(request, `P-${id}`, 2);
	const order = await createOrder(request, { model });
	await patchOrder(request, order.id, { extra_costs: 50_000 });

	const panel = await openPanel(page, model, order.id);
	const block = panel.getByRole('region', { name: 'Запчасти', exact: true });
	await block.getByRole('button', { name: 'Добавить со склада' }).click();
	const dialog = page.getByRole('dialog', { name: 'Запчасть со склада' });
	await dialog.getByRole('searchbox', { name: 'Поиск по складу' }).fill(`p-${id}`);
	await dialog.getByRole('button', { name: new RegExp(`P-${id} 2 шт\\.`) }).click();
	await dialog.getByRole('textbox', { name: 'Количество' }).fill('2');
	await dialog.getByRole('button', { name: 'Списать', exact: true }).click();
	await expect(dialog).toBeHidden();

	await expect(block.getByRole('listitem')).toContainText(new RegExp(`P-${id} × 2 — 2\\s000\\s₽`));
	await expect(block).toContainText(/Итого\s+2\s000\s₽/);
	await expect(panel).toContainText(/Себестоимость\s+2\s500\s₽/);
	expect(await itemQuantity(request, item.id)).toBe(0);
	// The version of the order stays, yet the journal shows the write-off at once.
	await expect(panel).toContainText(`Списано со склада: P-${id} × 2`);

	await block.getByRole('button', { name: 'Вернуть на склад' }).click();
	const confirm = page.getByRole('alertdialog', { name: 'Вернуть на склад?' });
	await expect(confirm).toContainText(`Вернуть «P-${id}» × 2 на склад?`);
	await confirm.getByRole('button', { name: 'Вернуть', exact: true }).click();
	await expect(block.getByRole('listitem')).toHaveCount(0);
	await expect(panel).not.toContainText('Себестоимость');
	expect(await itemQuantity(request, item.id)).toBe(2);
	await expect(panel).toContainText(`Возвращено на склад: P-${id}`);
	await request.dispose();
});

test('the last unit taken meanwhile is refused with the stock shown', async ({ page }) => {
	const id = uniq('l');
	await signInAdmin(page);
	const request = await api(page);
	const model = uniq('Модель');
	const item = await makeItem(request, `L-${id}`, 1);
	const order = await createOrder(request, { model });
	const other = await createOrder(request);

	const panel = await openPanel(page, model, order.id);
	await panel.getByRole('button', { name: 'Добавить со склада' }).click();
	const dialog = page.getByRole('dialog', { name: 'Запчасть со склада' });
	await dialog.getByRole('searchbox', { name: 'Поиск по складу' }).fill(`l-${id}`);
	const row = dialog.getByRole('button', { name: new RegExp(`L-${id}`) });
	await expect(row).toContainText('1 шт.');
	await row.click();

	await writeOff(request, other.id, item.id, 1);
	await dialog.getByRole('button', { name: 'Списать', exact: true }).click();
	await expect(dialog).toContainText('На складе не хватает: осталось 0');
	await expect(dialog.getByRole('button', { name: new RegExp(`L-${id}`) })).toContainText('0 шт.');
	expect(await itemQuantity(request, item.id)).toBe(0);
	await expect(panel.getByRole('region', { name: 'Запчасти', exact: true })).not.toContainText(
		`L-${id}`
	);
	await request.dispose();
});

test('the full card shows «Запчасти»', async ({ page }) => {
	const id = uniq('c');
	await signInAdmin(page);
	const request = await api(page);
	const item = await makeItem(request, `C-${id}`, 3);
	const order = await createOrder(request);
	await writeOff(request, order.id, item.id, 2);

	await page.goto(`/orders/${order.id}`);
	const block = page.getByRole('main').getByRole('region', { name: 'Запчасти', exact: true });
	await expect(block.getByRole('listitem')).toContainText(new RegExp(`C-${id} × 2 — 2\\s000\\s₽`));
	await expect(block).toContainText(/Итого\s+2\s000\s₽/);
	await expect(block.getByRole('link', { name: `C-${id}` })).toHaveAttribute(
		'href',
		new RegExp(`/stock\\?item=${item.id}$`)
	);
	await request.dispose();
});

test('narrow, «Запчасти» are in the main tab without switching', async ({ page }) => {
	const id = uniq('n');
	await signInAdmin(page);
	const request = await api(page);
	const item = await makeItem(request, `N-${id}`, 3);
	const order = await createOrder(request);
	await writeOff(request, order.id, item.id, 1);

	await page.setViewportSize({ width: 800, height: 900 });
	await page.goto(`/orders/${order.id}`);
	const block = page.getByRole('main').getByRole('region', { name: 'Запчасти', exact: true });
	await expect(block.getByRole('listitem')).toContainText(new RegExp(`N-${id} × 1`));
	await expect(page.getByRole('tab', { name: 'Заказ' })).toHaveAttribute('aria-selected', 'true');
	await request.dispose();
});

test('an unsaved edit of the card is saved without a dispute after a write-off, and the journal shows it', async ({
	page
}) => {
	const id = uniq('f');
	await signInAdmin(page);
	const request = await api(page);
	await makeItem(request, `F-${id}`, 3);
	const order = await createOrder(request);

	await page.goto(`/orders/${order.id}`);
	const main = page.getByRole('main');
	const note = `Заметка ${id}`;
	await main.getByRole('textbox', { name: 'Неисправность' }).fill(note);
	const block = main.getByRole('region', { name: 'Запчасти', exact: true });
	await block.getByRole('button', { name: 'Добавить со склада' }).click();
	const dialog = page.getByRole('dialog', { name: 'Запчасть со склада' });
	await dialog.getByRole('searchbox', { name: 'Поиск по складу' }).fill(`f-${id}`);
	await dialog.getByRole('button', { name: new RegExp(`F-${id} 3 шт\\.`) }).click();
	await dialog.getByRole('button', { name: 'Списать', exact: true }).click();
	await expect(dialog).toBeHidden();

	await expect(main.getByText('✓ Сохранено')).toBeVisible();
	await expect(main.getByText('изменили на другом компьютере')).toHaveCount(0);
	await expect(main.getByRole('region', { name: 'История изменений' })).toContainText(
		`Списано со склада: F-${id} × 1`
	);
	await page.reload();
	await expect(main.getByRole('textbox', { name: 'Неисправность' })).toHaveValue(note);
	await request.dispose();
});

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

test('without stock_view the block only lists', async ({ page, browser }) => {
	const id = uniq('r');
	await signInAdmin(page);
	const request = await api(page);
	const item = await makeItem(request, `R-${id}`, 3);
	const order = await createOrder(request);
	await writeOff(request, order.id, item.id, 1);
	const role = await request.post('/api/v1/roles', {
		data: { name: uniq('Роль'), permissions: ['orders_create', 'orders_edit'] }
	});
	expect(role.ok(), await role.text()).toBe(true);
	const login = uniq('user');
	const made = await request.post('/api/v1/users', {
		data: {
			display_name: login,
			login,
			password: USER_PASSWORD,
			role_id: ((await role.json()) as { id: number }).id,
			can_sign_in: true,
			is_master: false
		}
	});
	expect(made.ok(), await made.text()).toBe(true);

	const user = await signInAs(browser, login);
	try {
		await user.page.goto(`/orders/${order.id}`);
		const block = user.page
			.getByRole('main')
			.getByRole('region', { name: 'Запчасти', exact: true });
		await expect(block.getByRole('listitem')).toContainText(new RegExp(`R-${id} × 1`));
		await expect(block.getByRole('button', { name: 'Добавить со склада' })).toHaveCount(0);
		await expect(block.getByRole('button', { name: 'Вернуть на склад' })).toHaveCount(0);
		await expect(block.getByRole('link')).toHaveCount(0);
	} finally {
		await user.context.close();
		await request.dispose();
	}
});
