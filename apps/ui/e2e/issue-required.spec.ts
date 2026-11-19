import { mkdirSync, rmSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
	expect,
	test,
	type APIRequestContext,
	type Page,
	type PlaywrightWorkerArgs
} from '@playwright/test';
import type { AppSettings } from '$api/AppSettings';
import type { Order } from '$api/Order';
import { createOrder, E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, statusId, uniq } from './support/api';

// «Обязательно при выдаче» (spec 6.8): the setting is global and would stop other specs from issuing, so
// this one runs on the server with the administrator (no other spec issues there), one engine at a time
// (a lock folder), and puts the setting back in `finally`.
const DEFAULT_SET = ['final_price', 'work_done', 'master'];
const LOCK = join(tmpdir(), 'mw-e2e-issue-required.lock');
const STALE_MS = 2 * 60_000;
const WAIT_MS = 80_000;
async function lock(): Promise<() => void> {
	const until = Date.now() + WAIT_MS;
	for (;;) {
		try {
			mkdirSync(LOCK);
			return () => rmSync(LOCK, { recursive: true, force: true });
		} catch {
			// A lock of a run that died is older than any test: take it away.
			try {
				if (Date.now() - statSync(LOCK).mtimeMs > STALE_MS)
					rmSync(LOCK, { recursive: true, force: true });
			} catch {
				// Gone already.
			}
			if (Date.now() > until) {
				throw new Error(`issue-required: no lock ${LOCK} after ${WAIT_MS / 1000} s`);
			}
			await new Promise((r) => setTimeout(r, 200));
		}
	}
}

/** Signs in as the administrator, holds the lock and puts the default set back at the end. */
async function withSetting(
	page: Page,
	playwright: PlaywrightWorkerArgs['playwright'],
	run: (api: APIRequestContext, origin: string) => Promise<void>
) {
	const release = await lock();
	const origin = 'http://127.0.0.1:4174';
	let api: APIRequestContext | undefined;
	try {
		await page.goto(`${origin}/login`);
		await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
		await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
		await page.getByRole('button', { name: 'Войти' }).click();
		await expect(page).toHaveURL(/\/orders$/);
		const token = await page.evaluate(
			() => localStorage.getItem('mw.session') ?? sessionStorage.getItem('mw.session')
		);
		api = await playwright.request.newContext({
			baseURL: origin,
			extraHTTPHeaders: token ? { Authorization: `Bearer ${token}` } : {}
		});
		await run(api, origin);
	} finally {
		await api?.patch('/api/v1/settings', { data: { issue_required: DEFAULT_SET } });
		await api?.dispose();
		release();
	}
}

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

const rowOf = (page: Page, id: number) =>
	page
		.getByRole('grid', { name: 'Заказы' })
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });

test('the set of the settings is what the server checks and «Выдать» asks for', async ({
	page,
	playwright
}) => {
	test.setTimeout(120_000);
	await withSetting(page, playwright, async (api, origin) => {
		const settings = async () => (await (await api.get('/api/v1/settings')).json()) as AppSettings;
		const marker = uniq('M');
		const ready = await statusId(api, 'Готов');
		// No master and no price: the master will not be required below.
		const o = await createOrder(api, { model: marker, status_id: ready });
		const patched = await api.patch(`/api/v1/orders/${o.id}?version=${o.version}`, {
			data: { work_done: 'Работа' }
		});
		expect(patched.ok()).toBe(true);
		const filled = (await patched.json()) as Order;

		// The settings screen: three of four are ticked by default; the set is saved as it is edited.
		await page.goto(`${origin}/settings/orders`);
		const group = page.getByRole('group', { name: 'Обязательно при выдаче' });
		await expect(group.getByRole('checkbox')).toHaveCount(4);
		await expect(group.getByRole('checkbox', { name: 'Итоговая цена' })).toBeChecked();
		await expect(group.getByRole('checkbox', { name: 'Выполненные работы' })).toBeChecked();
		await expect(group.getByRole('checkbox', { name: 'Мастер' })).toBeChecked();
		await expect(group.getByRole('checkbox', { name: 'Гарантия' })).not.toBeChecked();
		await group.getByRole('checkbox', { name: 'Мастер' }).uncheck();
		await group.getByRole('checkbox', { name: 'Гарантия' }).check();
		await expect
			.poll(async () => (await settings()).issue_required)
			.toEqual(['final_price', 'work_done', 'warranty']);

		// The server refuses exactly what the set lists, with a code per field.
		const refused = await api.post(`/api/v1/orders/${o.id}/issue?version=${filled.version}`, {
			headers: { 'Content-Type': 'application/json' }
		});
		expect(refused.status()).toBe(422);
		const body = (await refused.json()) as { fields: { field: string; code: string }[] };
		expect(body.fields).toEqual([
			expect.objectContaining({ field: 'final_price', code: 'required_for_issue' }),
			expect.objectContaining({ field: 'warranty', code: 'required_for_issue' })
		]);

		// The dialog follows the same set: no master asked for, the warranty is.
		await page.goto(`${origin}/orders?q=${encodeURIComponent(marker)}&chips=all`);
		await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
		const panel = page.getByRole('dialog', { name: new RegExp(`^№ ${o.id}(?!\\d)`) });
		await expect(panel).toBeVisible();
		await panel.getByRole('button', { name: /^Выдать/ }).click();
		const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${o.id}` });
		await expect(dialog).toBeVisible();
		await expect(dialog.getByRole('combobox', { name: /^Мастер/ })).not.toHaveAttribute(
			'aria-invalid',
			'true'
		);
		const warranty = dialog.getByRole('combobox', { name: /^Гарантия/ });
		await warranty.fill('');
		await expect(warranty).toHaveAttribute('aria-invalid', 'true');
		await expect(dialog.locator('#issue-warranty-error')).toHaveText('Заполните перед выдачей');
		await dialog.getByRole('button', { name: /^Выдать/ }).click();
		await expect(dialog).toBeVisible();
		expect(((await (await api.get(`/api/v1/orders/${o.id}`)).json()) as Order).status_id).toBe(
			ready
		);

		await warranty.fill('14 дней');
		const price = dialog.getByRole('textbox', { name: /^Итоговая цена/ });
		await price.click();
		await price.fill('1200');
		await dialog.getByRole('button', { name: /^Выдать/ }).click();
		await expect(dialog).toBeHidden();
		const after = (await (await api.get(`/api/v1/orders/${o.id}`)).json()) as Order;
		expect(after.status_id).toBe(await statusId(api, 'Выдан'));
		expect(after.final_price).toBe(120000);
	});
});

test('bulk issue skips an order that lacks what the set requires', async ({ page, playwright }) => {
	test.setTimeout(120_000);
	await withSetting(page, playwright, async (api, origin) => {
		const set = await api.patch('/api/v1/settings', {
			data: { issue_required: ['final_price', 'warranty'] }
		});
		expect(set.ok()).toBe(true);
		const marker = uniq('M');
		const ready = await statusId(api, 'Готов');
		const issued = await statusId(api, 'Выдан');
		const make = async (price: number | null) => {
			const o = await createOrder(api, {
				model: marker,
				status_id: ready,
				master: 'Тестов',
				warranty: '30 дней'
			});
			const r = await api.patch(`/api/v1/orders/${o.id}?version=${o.version}`, {
				data: { work_done: 'Чистка', final_price: price }
			});
			expect(r.ok()).toBe(true);
			return (await r.json()) as Order;
		};
		const good = await make(100000);
		const bad = await make(null);

		await page.goto(`${origin}/orders?q=${encodeURIComponent(marker)}&chips=all`);
		await expect(rowOf(page, good.id)).toBeVisible();
		await expect(rowOf(page, bad.id)).toBeVisible();
		await page.getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
		const bar = page.getByRole('region', { name: 'Выбранные строки' });
		await expect(bar).toContainText('Выбрано 2');
		await bar.getByRole('button', { name: 'Статус' }).click();
		await page.getByRole('menuitem', { name: 'Выдан' }).click();
		await page
			.getByRole('dialog', { name: 'Дата выдачи' })
			.getByRole('button', { name: 'Сейчас' })
			.click();

		const toasts = page.getByRole('status', { name: 'Уведомления' });
		await expect(toasts).toContainText('Статус «Выдан» у 1 заказа');
		await expect(toasts).toContainText(
			`Не удалось у 1 заказа: № ${bad.id} — заполните: итоговая цена`
		);
		// As before: the selection is dropped after the batch.
		await expect(bar).toBeHidden();
		expect(((await (await api.get(`/api/v1/orders/${good.id}`)).json()) as Order).status_id).toBe(
			issued
		);
		expect(((await (await api.get(`/api/v1/orders/${bad.id}`)).json()) as Order).status_id).toBe(
			ready
		);
	});
});
