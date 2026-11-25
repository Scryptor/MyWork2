import { readFileSync, statSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { createClient, createOrder, uniq } from './support/api';

// «В Excel» of the lists of orders and clients on the server of 4173 (open mode: every right). Each test names its
// data with a marker of its own and narrows the list to it, so the file is the narrowed list.

const filterOf = (page: import('@playwright/test').Page) =>
	page.getByRole('searchbox', { name: 'Фильтр списка' });

test('«В Excel» of the orders gives a file named «Заказы …»', async ({ page, request }) => {
	const marker = uniq('Выгр');
	await createOrder(request, { clientName: `Клиент ${marker}` });
	await page.goto('/orders');
	await filterOf(page).fill(marker);
	await expect(page.getByRole('grid').getByRole('row').filter({ hasText: marker })).toHaveCount(1);
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('button', { name: 'В Excel' }).click()
	]);
	// WebKit on a Mac gives the name with letters taken apart: the same letters.
	const name = download.suggestedFilename().normalize('NFC');
	expect(name.startsWith('Заказы ')).toBe(true);
	expect(name.endsWith('.xlsx')).toBe(true);
	expect(statSync(await download.path()).size).toBeGreaterThan(0);
});

test('«Выгрузить» → «В Excel» of the clients gives a file named «Клиенты …»', async ({
	page,
	request
}) => {
	const marker = uniq('Выгр');
	await createClient(request, { full_name: `Клиентов ${marker}` });
	await page.goto('/clients');
	await filterOf(page).fill(marker);
	await expect(
		page.getByRole('grid', { name: 'Клиенты' }).getByRole('row').filter({ hasText: marker })
	).toHaveCount(1);
	await page.getByRole('button', { name: 'Выгрузить' }).click();
	const [download] = await Promise.all([
		page.waitForEvent('download'),
		page.getByRole('menuitem', { name: 'В Excel' }).click()
	]);
	const name = download.suggestedFilename().normalize('NFC');
	expect(name.startsWith('Клиенты ')).toBe(true);
	expect(name.endsWith('.xlsx')).toBe(true);
	expect(statSync(await download.path()).size).toBeGreaterThan(0);
});

for (const [format, extension, start] of [
	['Google Контакты (CSV)', '.csv', 'Name,Given Name,'],
	['vCard (iPhone, Android)', '.vcf', 'BEGIN:VCARD']
] as const) {
	test(`«Контакты для телефона…» as ${extension} holds the name and the number`, async ({
		page,
		request
	}) => {
		const marker = uniq('Контакт');
		const client = await createClient(request, { full_name: `Контактов ${marker}` });
		const phone = client.phones[0]!.phone;
		await page.goto('/clients');
		await filterOf(page).fill(marker);
		await expect(
			page.getByRole('grid', { name: 'Клиенты' }).getByRole('row').filter({ hasText: marker })
		).toHaveCount(1);
		await page.getByRole('button', { name: 'Выгрузить' }).click();
		await page.getByRole('menuitem', { name: 'Контакты для телефона…' }).click();
		const dialog = page.getByRole('dialog', { name: 'Контакты для телефона' });
		await dialog.getByRole('radio', { name: format }).check();
		const [download] = await Promise.all([
			page.waitForEvent('download'),
			dialog.getByRole('button', { name: 'Выгрузить' }).click()
		]);
		const name = download.suggestedFilename().normalize('NFC');
		expect(name.startsWith('Контакты ')).toBe(true);
		expect(name.endsWith(extension)).toBe(true);
		const text = readFileSync(await download.path(), 'utf8');
		expect(text.startsWith(start)).toBe(true);
		expect(text).toContain(`Контактов ${marker}`);
		expect(text).toContain(phone);
		await expect(dialog).toBeHidden();
	});
}

test("a refusal of the server is a toast with the server's words", async ({ page }) => {
	const words = 'Слишком много строк для одного файла — не больше 50 000, сузьте выборку';
	await page.route('**/api/v1/exports/orders*', (route) =>
		route.fulfill({
			status: 422,
			contentType: 'application/json',
			body: JSON.stringify({
				code: 'validation',
				message: 'Проверьте введённые данные',
				fields: [{ field: 'rows', code: 'too_many_rows', message: words }],
				current_version: null,
				permission: null,
				retry_after_secs: null,
				diagnostics: []
			})
		})
	);
	await page.goto('/orders');
	await page.getByRole('button', { name: 'В Excel' }).click();
	await expect(
		page.getByRole('status', { name: 'Уведомления' }).getByText(words, { exact: false })
	).toBeVisible();
});
