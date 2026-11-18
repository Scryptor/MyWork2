import { expect, test, type Page } from '@playwright/test';
import {
	createClient,
	createOrder,
	fetchClient,
	patchClient,
	uniq,
	uniqPhone
} from './support/api';

// Merging clients on the server of 4173 (open mode: every right). Each test names its clients with a
// unique marker and narrows the list to it.

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Клиенты' });
const chip = (page: Page, name: string) =>
	page.getByRole('group', { name: 'Быстрые фильтры' }).getByRole('button', { name });
const mergeOf = (page: Page) => page.getByRole('dialog', { name: 'Объединение клиентов' });
const confirmOf = (page: Page) => page.getByRole('alertdialog');
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

/** Two clients with one phone and a name that differs by case only, each with an order. */
async function twins(request: Parameters<typeof createClient>[0], marker: string) {
	const phone = uniqPhone();
	const a = await createClient(request, {
		full_name: `ИВАНОВ ИВАН ${marker}`,
		phones: [{ phone, note: '' }],
		address: 'ул. Первая, 1'
	});
	const b = await createClient(request, {
		full_name: `Иванов Иван ${marker}`,
		phones: [{ phone, note: 'раб' }],
		address: 'ул. Вторая, 2'
	});
	await createOrder(request, { client: { existing: a.id }, model: uniq('M') });
	await createOrder(request, { client: { existing: b.id }, model: uniq('M') });
	return { a, b };
}

test('selected similar clients are merged: the chosen address, both orders, one client', async ({
	page,
	request
}) => {
	const marker = uniq('m');
	const { a, b } = await twins(request, marker);
	await page.goto(`/clients?q=${marker}`);
	await chip(page, 'Похожие').click();
	await expect(gridOf(page).getByRole('row').filter({ hasText: marker })).toHaveCount(2);
	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await page.getByRole('button', { name: 'Объединить', exact: true }).click();

	const dialog = mergeOf(page);
	await expect(dialog).toBeVisible();
	// A tie of orders: the smaller number stays; its address is the default, the second's is chosen.
	await expect(dialog.getByRole('radio', { name: `Оставить клиента № ${a.id}` })).toBeChecked();
	await dialog.getByRole('radio', { name: `Адрес: взять у клиента № ${b.id}` }).check();
	await expect(dialog.getByText(/Останется: .* · 2 заказа · /)).toBeVisible();
	await dialog.getByRole('button', { name: 'Объединить', exact: true }).click();
	await confirmOf(page).getByRole('button', { name: 'Объединить' }).click();

	await expect(toastsOf(page)).toContainText('Клиенты объединены');
	await expect(page).toHaveURL(new RegExp(`client=${a.id}`));
	const kept = await fetchClient(request, a.id);
	expect(kept.address).toBe('ул. Вторая, 2');
	expect(kept.phones).toHaveLength(1);
	const gone = await request.get(`/api/v1/clients/${b.id}`);
	expect(gone.status()).toBe(404);
	const orders = await request.get(`/api/v1/orders?client_id=${a.id}&limit=10`);
	expect((await orders.json()).total).toBe(2);
});

test('a client changed meanwhile: the dialog says so and reads the data again', async ({
	page,
	request
}) => {
	const marker = uniq('v');
	const { a, b } = await twins(request, marker);
	await page.goto(`/clients?q=${marker}&dup=1`);
	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await page.getByRole('button', { name: 'Объединить', exact: true }).click();
	const dialog = mergeOf(page);
	await expect(dialog).toBeVisible();
	await expect(dialog.getByText('ул. Вторая, 2')).toBeVisible();

	await patchClient(request, b.id, { address: 'ул. Третья, 3' });
	await dialog.getByRole('button', { name: 'Объединить', exact: true }).click();
	await confirmOf(page).getByRole('button', { name: 'Объединить' }).click();

	await expect(dialog.getByRole('alert')).toContainText(
		'Клиента изменили, пока вы выбирали. Проверьте и объедините снова'
	);
	await expect(dialog.getByText('ул. Третья, 3')).toBeVisible();
	expect((await fetchClient(request, b.id)).address).toBe('ул. Третья, 3');
	await dialog.getByRole('button', { name: 'Объединить', exact: true }).click();
	await confirmOf(page).getByRole('button', { name: 'Объединить' }).click();
	await expect(toastsOf(page)).toContainText('Клиенты объединены');
	expect((await fetchClient(request, a.id)).id).toBe(a.id);
});

test('from the panel: «Объединить с…» finds a client; «Объединить с этим» in similar', async ({
	page,
	request
}) => {
	const marker = uniq('p');
	const { a, b } = await twins(request, marker);
	const other = await createClient(request, { full_name: `Петров ${marker}` });
	await page.goto(`/clients?client=${a.id}`);
	const panel = page.getByRole('dialog', { name: `ИВАНОВ ИВАН ${marker}` });
	await expect(panel).toBeVisible();

	// «Объединить с этим» of the similar client.
	await panel.getByRole('button', { name: 'Объединить с этим' }).click();
	await expect(mergeOf(page).getByRole('radio', { name: /^Оставить клиента/ })).toHaveCount(2);
	await mergeOf(page).getByRole('button', { name: 'Отмена' }).click();
	await expect(mergeOf(page)).toHaveCount(0);

	// «Объединить с…»: a search, then the dialog with the two.
	await panel.getByRole('button', { name: 'Объединить с…' }).click();
	const picker = page.getByRole('dialog', { name: 'Объединить с…' });
	await picker.getByRole('combobox', { name: 'Клиент' }).fill(`Петров ${marker}`);
	await page.getByRole('option', { name: new RegExp(`Петров ${marker}`) }).click();
	await picker.getByRole('button', { name: 'Далее' }).click();
	await expect(
		mergeOf(page).getByRole('radio', { name: `Оставить клиента № ${other.id}` })
	).toBeVisible();
	await expect(
		mergeOf(page).getByRole('radio', { name: `Оставить клиента № ${a.id}` })
	).toBeVisible();
	expect(b.id).not.toBe(other.id);
});

test('the picker finds a client by phone digits,', async ({ page, request }) => {
	const marker = uniq('f');
	const { a } = await twins(request, marker);
	const phone = uniqPhone();
	const other = await createClient(request, {
		full_name: `Сидоров ${marker}`,
		phones: [{ phone, note: '' }]
	});
	await page.goto(`/clients?client=${a.id}`);
	await page.getByRole('button', { name: 'Объединить с…' }).click();
	const picker = page.getByRole('dialog', { name: 'Объединить с…' });
	await picker.getByRole('combobox', { name: 'Клиент' }).fill(phone.slice(-7));
	await page.getByRole('option', { name: new RegExp(`Сидоров ${marker}`) }).click();
	await picker.getByRole('button', { name: 'Далее' }).click();
	await expect(
		mergeOf(page).getByRole('radio', { name: `Оставить клиента № ${other.id}` })
	).toBeVisible();
});

test('four clients scroll sideways on a narrow screen and the field labels stay in view', async ({
	page,
	request
}) => {
	const marker = uniq('w');
	for (let i = 0; i < 4; i++) {
		await createClient(request, { full_name: `Четверной ${marker}`, address: `ул. ${i}` });
	}
	await page.goto(`/clients?q=${marker}`);
	await expect(gridOf(page).getByRole('row').filter({ hasText: marker })).toHaveCount(4);
	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await page.getByRole('button', { name: 'Объединить', exact: true }).click();
	const dialog = mergeOf(page);
	await expect(dialog.getByRole('radio', { name: /^Оставить клиента/ })).toHaveCount(4);
	await page.setViewportSize({ width: 390, height: 800 });
	const label = dialog.getByText('ФИО', { exact: true });
	await dialog.locator('.scroll').evaluate((el) => (el.scrollLeft = el.scrollWidth));
	const box = await label.boundingBox();
	expect(box).not.toBeNull();
	expect(box!.x).toBeGreaterThanOrEqual(0);
	expect(box!.x + box!.width).toBeLessThanOrEqual(390);
	const last = dialog.getByRole('radio', { name: /^Оставить клиента/ }).last();
	await expect(last).toBeInViewport();
});
