import { mkdirSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { expect, test } from '@playwright/test';
import type { AppSettings } from '$api/AppSettings';
import type { Order } from '$api/Order';
import { createOrder, E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, statusId, uniq } from './support/api';

// «Обязательно при выдаче» (spec 6.8): the setting is global and would stop other specs from issuing, so
// this one runs on the server with the administrator (no other spec issues there), one engine at a time
// (a lock folder), and puts the setting back in `finally`.
const LOCK = join(tmpdir(), 'mw-e2e-issue-required.lock');
async function lock(): Promise<() => void> {
	for (;;) {
		try {
			mkdirSync(LOCK);
			return () => rmSync(LOCK, { recursive: true, force: true });
		} catch {
			await new Promise((r) => setTimeout(r, 200));
		}
	}
}

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

test('what the settings require is checked on the server and in «Выдать»', async ({
	page,
	playwright
}) => {
	test.setTimeout(90_000);
	const release = await lock();
	const origin = 'http://127.0.0.1:4174';
	await page.goto(`${origin}/login`);
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	const token = await page.evaluate(
		() => localStorage.getItem('mw.session') ?? sessionStorage.getItem('mw.session')
	);
	const api = await playwright.request.newContext({
		baseURL: origin,
		extraHTTPHeaders: token ? { Authorization: `Bearer ${token}` } : {}
	});
	const settings = async () => (await (await api.get('/api/v1/settings')).json()) as AppSettings;
	try {
		const marker = uniq('M');
		const ready = await statusId(api, 'Готов');
		const o = await createOrder(api, { model: marker, status_id: ready, master: 'Тестов' });
		const patched = await api.patch(`/api/v1/orders/${o.id}?version=${o.version}`, {
			data: { work_done: 'Работа' }
		});
		expect(patched.ok()).toBe(true);
		const filled = (await patched.json()) as Order;

		// The settings screen: a check box per field, saved as it is ticked.
		await page.goto(`${origin}/settings/orders`);
		const group = page.getByRole('group', { name: 'Обязательно при выдаче' });
		await expect(group.getByRole('checkbox')).toHaveCount(4);
		await group.getByRole('checkbox', { name: 'Итоговая цена' }).check();
		await group.getByRole('checkbox', { name: 'Гарантия' }).check();
		await expect
			.poll(async () => (await settings()).issue_required)
			.toEqual(['final_price', 'warranty']);

		// The server refuses to issue without the price, with a code per field.
		const refused = await api.post(`/api/v1/orders/${o.id}/issue?version=${filled.version}`, {
			headers: { 'Content-Type': 'application/json' }
		});
		expect(refused.status()).toBe(422);
		const body = (await refused.json()) as { fields: { field: string; code: string }[] };
		expect(body.fields).toEqual([
			expect.objectContaining({ field: 'final_price', code: 'required_for_issue' }),
			expect.objectContaining({ field: 'warranty', code: 'required_for_issue' })
		]);

		// The dialog: the price is entered, the warranty is cleared and shown as missing.
		await page.goto(`${origin}/orders?q=${encodeURIComponent(marker)}&chips=all`);
		await page
			.getByRole('grid', { name: 'Заказы' })
			.getByRole('row')
			.filter({ has: page.getByRole('gridcell', { name: String(o.id), exact: true }) })
			.getByRole('gridcell')
			.nth(3)
			.click();
		const panel = page.getByRole('dialog', { name: new RegExp(`^№ ${o.id}(?!\\d)`) });
		await expect(panel).toBeVisible();
		await panel.getByRole('button', { name: /^Выдать/ }).click();
		const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${o.id}` });
		await expect(dialog).toBeVisible();
		const warranty = dialog.getByRole('combobox', { name: /^Гарантия/ });
		await warranty.fill('');
		await expect(warranty).toHaveAttribute('aria-invalid', 'true');
		await expect(dialog.locator('#issue-warranty-error')).toHaveText('Заполните перед выдачей');
		await dialog.getByRole('button', { name: /^Выдать/ }).click();
		await expect(dialog).toBeVisible();
		expect((await (await api.get(`/api/v1/orders/${o.id}`)).json()).status_id).toBe(ready);

		await warranty.fill('14 дней');
		const price = dialog.getByRole('textbox', { name: /^Итоговая цена/ });
		await price.click();
		await price.fill('1200');
		await dialog.getByRole('button', { name: /^Выдать/ }).click();
		await expect(dialog).toBeHidden();
		const after = (await (await api.get(`/api/v1/orders/${o.id}`)).json()) as Order;
		expect(after.status_id).toBe(await statusId(api, 'Выдан'));
		expect(after.final_price).toBe(120000);
	} finally {
		await api.patch('/api/v1/settings', { data: { issue_required: [] } });
		await api.dispose();
		release();
	}
});
