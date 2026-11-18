import { expect, test, type Page } from '@playwright/test';
import { createClient, createOrder, fetchClient, uniq, uniqPhone } from './support/api';

// «Клиенты» on the server of 4173 (open mode: every right). The database is shared by every test and
// both browsers: each test names its clients with a unique marker and narrows the list to it.

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Клиенты' });
const rowsOf = (page: Page, text: string) =>
	gridOf(page).getByRole('row').filter({ hasText: text });
const filterOf = (page: Page) => page.getByRole('searchbox', { name: 'Фильтр списка' });
const chip = (page: Page, name: string) =>
	page.getByRole('group', { name: 'Быстрые фильтры' }).getByRole('button', { name });
const panelOf = (page: Page, name: string) => page.getByRole('dialog', { name });
const orderPanelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const sheetOf = (page: Page) => page.getByRole('dialog', { name: 'Новый заказ' });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

/** A client name nobody else has, in mixed case: the screen shows it as it is. */
const clientName = () => `Клиентова ${uniq('k')}`;

/** The panel of a client opened through the address, with its fields there. */
async function openPanel(page: Page, id: number, name: string) {
	await page.goto(`/clients?client=${id}`);
	await expect(panelOf(page, name)).toBeVisible();
	await expect(panelOf(page, name).getByRole('textbox', { name: 'ФИО' })).toHaveValue(name);
	return panelOf(page, name);
}

test('the list finds a client by a phone number', async ({ page, request }) => {
	const name = clientName();
	const phone = uniqPhone();
	await createClient(request, { full_name: name, phones: [{ phone, note: '' }] });
	await page.goto('/clients');
	await expect(page.getByRole('heading', { name: 'Клиенты', level: 1 })).toBeAttached();
	await filterOf(page).fill(phone.slice(-7));
	await expect(page).toHaveURL(new RegExp(`q=${phone.slice(-7)}`));
	await expect(rowsOf(page, name)).toHaveCount(1);
	// The header and that client.
	await expect(gridOf(page).getByRole('row')).toHaveCount(2);
	await expect(rowsOf(page, name)).toContainText(
		`+7 911 ${phone.slice(5, 8)}-${phone.slice(8, 10)}-${phone.slice(10)}`
	);
});

test('sorts by the number of orders', async ({ page, request }) => {
	const marker = uniq('s');
	const one = await createOrder(request, { clientName: `Один ${marker}` });
	const two = await createOrder(request, { clientName: `Два ${marker}` });
	await createOrder(request, { client: { existing: two.client_id } });
	await page.goto(`/clients?q=${marker}`);
	const rows = gridOf(page).getByRole('row').filter({ hasText: marker });
	await expect(rows).toHaveCount(2);
	// By default: the last order first, so the one with the second order on top.
	await expect(rows.first()).toContainText(`Два ${marker}`);

	await gridOf(page).getByRole('button', { name: 'Заказы' }).click();
	await expect(page).toHaveURL(/sort=orders&desc=1/);
	await expect(rows.first()).toContainText(`Два ${marker}`);
	await gridOf(page).getByRole('button', { name: 'Заказы' }).click();
	await expect(page).toHaveURL(/sort=orders&desc=0/);
	await expect(rows.first()).toContainText(`Один ${marker}`);
	expect(one.client_id).not.toBe(two.client_id);
});

test('a row opens the panel; closing it takes the client out of the address', async ({
	page,
	request
}) => {
	const name = clientName();
	const c = await createClient(request, { full_name: name });
	await page.goto(`/clients?q=${encodeURIComponent(name)}`);
	await rowsOf(page, name).getByText(name).click();
	await expect(panelOf(page, name)).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`client=${c.id}`));
	await expect(page).toHaveURL(/q=/);
	await page.keyboard.press('Escape');
	await expect(panelOf(page, name)).toBeHidden();
	await expect(page).not.toHaveURL(/client=/);
	await expect(page).toHaveURL(/q=/);
});

test('the panel: the name is saved by itself', async ({ page, request }) => {
	const name = clientName();
	const c = await createClient(request, { full_name: name });
	const panel = await openPanel(page, c.id, name);
	const field = panel.getByRole('textbox', { name: 'ФИО' });
	await field.fill(`${name} Петровна`);
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
	await expect
		.poll(async () => (await fetchClient(request, c.id)).full_name)
		.toBe(`${name} Петровна`);
	// The list behind shows it too.
	await expect(panelOf(page, `${name} Петровна`)).toBeVisible();
});

test('«＋ номер» with the note «жена»: the client has two phones', async ({ page, request }) => {
	const name = clientName();
	const c = await createClient(request, { full_name: name });
	const panel = await openPanel(page, c.id, name);
	await panel.getByRole('button', { name: '＋ номер' }).click();
	const second = panel.getByRole('textbox', { name: 'Номер 2' });
	await expect(second).toBeFocused();
	const phone = uniqPhone();
	await second.fill(phone);
	await panel.getByRole('textbox', { name: 'Пометка 2' }).fill('жена');
	await expect
		.poll(async () => (await fetchClient(request, c.id)).phones)
		.toEqual([c.phones[0], { phone: phone.replace(/\D/g, ''), note: 'жена' }]);
	await expect(panel.getByText('✓ Сохранено')).toBeVisible();
});

test('«Сделать основным» puts the second number first', async ({ page, request }) => {
	const name = clientName();
	const first = uniqPhone();
	const second = uniqPhone();
	const c = await createClient(request, {
		full_name: name,
		phones: [
			{ phone: first, note: '' },
			{ phone: second, note: 'раб' }
		]
	});
	const panel = await openPanel(page, c.id, name);
	await panel.getByRole('button', { name: 'Сделать основным' }).click();
	await expect
		.poll(async () => (await fetchClient(request, c.id)).phones.map((p) => p.phone))
		.toEqual([second, first].map((p) => p.replace(/\D/g, '')));
	await expect(panel.getByRole('textbox', { name: 'Пометка', exact: true })).toHaveValue('раб');

	// A number taken away goes at once.
	await panel.getByRole('button', { name: 'Убрать номер 2' }).click();
	await expect
		.poll(async () => (await fetchClient(request, c.id)).phones.map((p) => p.phone))
		.toEqual([second.replace(/\D/g, '')]);
});

test('an order of the client opens its panel with «← к клиенту», and that leads back', async ({
	page,
	request
}) => {
	const name = clientName();
	const o = await createOrder(request, { clientName: name, model: uniq('M') });
	const panel = await openPanel(page, o.client_id, name);
	const orders = panel.getByRole('list', { name: 'Заказы клиента' });
	await orders.getByRole('button', { name: new RegExp(`^№ ${o.id}(?!\\d)`) }).click();
	await expect(orderPanelOf(page, o.id)).toBeVisible();
	await orderPanelOf(page, o.id).getByRole('link', { name: '← к клиенту' }).click();
	await expect(orderPanelOf(page, o.id)).toBeHidden();
	await expect(panelOf(page, name)).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`/clients\\?client=${o.client_id}$`));
});

test('«Новый заказ» opens the intake with this client', async ({ page, request }) => {
	const name = clientName();
	const c = await createClient(request, { full_name: name });
	const panel = await openPanel(page, c.id, name);
	await panel.getByRole('button', { name: 'Новый заказ' }).click();
	await expect(sheetOf(page)).toBeVisible();
	await expect(sheetOf(page).getByRole('region', { name: 'Клиент заказа' })).toContainText(name);
});

test('«Новый заказ» replaces a draft already accepted: this client, an empty device', async ({
	page,
	request
}) => {
	// A draft whose «Принять» went and was changed afterwards: it stays, marked «уже принят».
	await page.goto('/orders');
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
	await page.keyboard.press('n');
	const sheet = sheetOf(page);
	await expect(sheet.getByRole('textbox', { name: 'Телефон' })).toBeFocused();
	await sheet.getByRole('textbox', { name: 'Телефон' }).fill(uniqPhone());
	for (const [label, value] of [
		[/^Тип устройства/, 'Телефон'],
		[/^Бренд/, 'Тест'],
		[/^Модель/, 'Модель 1']
	] as const) {
		const box = sheet.getByRole('combobox', { name: label });
		await box.click();
		await box.fill(value);
	}
	const malfunction = sheet.getByRole('textbox', { name: /^Неисправность/ });
	await malfunction.click();
	await expect(sheet.getByRole('combobox', { expanded: true })).toHaveCount(0);
	const marker = uniq('Не включается');
	await malfunction.fill(marker);
	let letGo: () => void = () => {};
	const held = new Promise<void>((resolve) => (letGo = resolve));
	await page.route('**/api/v1/orders', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		await held;
		await route.fallback();
	});
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheet.getByText('Принимаю…')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
	await page.keyboard.press('n');
	await sheet.getByRole('textbox', { name: 'Комментарий (только для сервиса)' }).fill('Позже');
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
	letGo();
	await expect(toastsOf(page).getByText(/^Принят № \d+$/)).toBeVisible();
	await page.keyboard.press('n');
	await expect(sheet.getByText(/^Заказ № \d+ уже принят\./)).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(sheet).toBeHidden();
	await page.unroute('**/api/v1/orders');

	const name = clientName();
	const c = await createClient(request, { full_name: name });
	const panel = await openPanel(page, c.id, name);
	await panel.getByRole('button', { name: 'Новый заказ' }).click();
	await expect(sheet).toBeVisible();
	await expect(sheet.getByRole('region', { name: 'Клиент заказа' })).toContainText(name);
	await expect(malfunction).toHaveValue('');
	await expect(sheet.getByRole('combobox', { name: /^Модель/ })).toHaveValue('');
	await expect(sheet.getByText(/уже принят/)).toHaveCount(0);
});

test('«Похожие» shows clients with a twin; the panel offers the twin', async ({
	page,
	request
}) => {
	const marker = uniq('p');
	const phone = uniqPhone();
	const a = await createClient(request, {
		full_name: `Похожая ${marker}`,
		phones: [{ phone, note: '' }]
	});
	const b = await createClient(request, {
		full_name: `Двойник ${marker}`,
		phones: [{ phone, note: 'раб' }]
	});
	await page.goto(`/clients?q=${marker}`);
	await expect(chip(page, 'Все')).toHaveAttribute('aria-pressed', 'true');
	await chip(page, 'Похожие').click();
	await expect(page).toHaveURL(/dup=1/);
	await expect(gridOf(page).getByRole('row').filter({ hasText: marker })).toHaveCount(2);

	const panel = await openPanel(page, a.id, `Похожая ${marker}`);
	await expect(panel.getByText('Похожие клиенты: 1')).toBeVisible();
	await panel.getByRole('button', { name: new RegExp(`^Двойник ${marker}`) }).click();
	await expect(panelOf(page, `Двойник ${marker}`)).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`client=${b.id}`));
});

test('«Клиенты» is in the menu; a client of an order and of Ctrl K opens there', async ({
	page,
	request
}) => {
	const name = clientName();
	const o = await createOrder(request, { clientName: name, model: uniq('M') });
	await page.goto('/orders');
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: 'Клиенты' })
		.click();
	await expect(page).toHaveURL(/\/clients$/);
	await expect(gridOf(page)).toBeVisible();

	// Ctrl K: a client opens on «Клиенты».
	await page.goto('/orders?chips=all');
	await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
	await palette.getByRole('combobox').fill(name);
	await palette.getByRole('option', { name: new RegExp(`^${name}`) }).click();
	await expect(page).toHaveURL(new RegExp(`/clients\\?client=${o.client_id}$`));
	await expect(panelOf(page, name)).toBeVisible();
	await page.keyboard.press('Escape');

	// «открыть» at the client of the order panel.
	await page.goto(`/orders?q=${encodeURIComponent(name)}&chips=all`);
	await page
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(o.id), exact: true }) })
		.getByRole('gridcell')
		.nth(3)
		.click();
	await orderPanelOf(page, o.id).getByRole('link', { name: 'открыть' }).click();
	await expect(page).toHaveURL(new RegExp(`/clients\\?client=${o.client_id}$`));
	await expect(orderPanelOf(page, o.id)).toBeHidden();
	await expect(panelOf(page, name)).toBeVisible();
});

test.describe('on a phone', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the list is cards; a card opens the client on the whole screen', async ({
		page,
		request
	}) => {
		const name = clientName();
		const o = await createOrder(request, { clientName: name });
		await page.goto(`/clients?q=${encodeURIComponent(name)}`);
		await expect(gridOf(page)).toHaveCount(0);
		const card = page.getByRole('button', { name: new RegExp(`^${name}`) });
		await expect(card).toBeVisible();
		await expect(card).toContainText('1 заказ');
		await expect(
			page
				.getByRole('navigation', { name: 'Основная навигация' })
				.getByRole('link', { name: 'Клиенты' })
		).toHaveAttribute('aria-current', 'page');
		await card.click();
		await expect(panelOf(page, name)).toBeVisible();
		await expect(page).toHaveURL(new RegExp(`client=${o.client_id}`));
	});
});
