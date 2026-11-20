import { expect, test as base } from '@playwright/test';
import type { AppSettings } from '$api/AppSettings';
import type { MasterUser } from '$api/MasterUser';
import type { ServerInfo } from '$api/ServerInfo';
import type { ServicePoint } from '$api/ServicePoint';
import { uniq } from './support/api';
import { freshOrigin } from './support/fresh';

// The first run (spec 6.9) on a new base of its own (`--fresh`, one per engine): the first test sets it up,
// the second finds it set up. They go in this order.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(freshOrigin('setup', browserName))
});
test.describe.configure({ mode: 'serial' });

test('a new base opens the first run; «Начать с пустой базы» sets it up and opens the orders', async ({
	page,
	request
}) => {
	const before = (await (await request.get('/api/v1/server-info')).json()) as ServerInfo;
	expect(before.setup_required).toBe(true);

	await page.goto('/orders');
	await expect(page).toHaveURL(/\/setup$/);
	await expect(page.getByRole('heading', { name: 'Первый запуск' })).toBeVisible();
	await expect(page.getByRole('button', { name: /Перенести из MyWork 2/ })).toBeVisible();
	await page.getByRole('button', { name: /Начать с пустой базы/ }).click();

	// Nothing typed: the two fields say so, and nothing is sent.
	const firm = page.getByRole('textbox', { name: /^Название сервиса/ });
	const point = page.getByRole('textbox', { name: /^Первая точка/ });
	await page.getByRole('button', { name: 'Начать работу' }).click();
	await expect(firm).toHaveAttribute('aria-invalid', 'true');
	await expect(point).toHaveAttribute('aria-invalid', 'true');

	const name = uniq('Сервис');
	const pointName = `Мира, 5 ${name}`;
	await firm.fill(name);
	await point.fill(pointName);
	// Masters are chips: Enter adds one, a repeat is not added, × takes one away.
	const masters = page.getByRole('textbox', { name: 'Мастера' });
	for (const m of ['Петров', 'Сидоров', ' петров ']) {
		await masters.fill(m);
		await masters.press('Enter');
	}
	const chips = page.getByRole('list', { name: 'Мастера' });
	await expect(chips.getByRole('listitem')).toHaveText(['Петров', 'Сидоров']);
	await chips.getByRole('button', { name: 'Убрать «Сидоров»' }).click();
	await masters.fill('Иванов');
	await masters.press('Enter');
	await expect(chips.getByRole('listitem')).toHaveText(['Петров', 'Иванов']);
	await page.getByRole('button', { name: 'Начать работу' }).click();

	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByText(/Примите первое устройство \(N\)/)).toBeVisible();

	// What the form set: the name, the point (the default one too) and the masters.
	const settings = (await (await request.get('/api/v1/settings')).json()) as AppSettings;
	expect(settings.firm_name).toBe(name);
	const points = (await (await request.get('/api/v1/service-points')).json()) as ServicePoint[];
	const created = points.find((p) => p.name === pointName);
	expect(created).toBeDefined();
	expect(settings.default_service_point_id).toBe(created!.id);
	// Masters are users marked «Мастер» (without sign-in).
	const users = (await (await request.get('/api/v1/masters')).json()) as MasterUser[];
	const values = users.map((m) => m.display_name);
	expect(values).toEqual(expect.arrayContaining(['Петров', 'Иванов']));
	expect(values).not.toContain('Сидоров');
});

test('on the same base the first run is not shown again', async ({ page, request }) => {
	const info = (await (await request.get('/api/v1/server-info')).json()) as ServerInfo;
	expect(info.setup_required).toBe(false);
	await page.goto('/');
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByText(/Примите первое устройство \(N\)/)).toBeVisible();
	await page.goto('/setup');
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByRole('heading', { name: 'Первый запуск' })).toHaveCount(0);
	// The server refuses a second setup in words.
	const again = await request.post('/api/v1/setup', {
		data: { firm_name: 'Другой', point_name: 'Другая', masters: [] }
	});
	expect(again.status()).toBe(409);
	expect(((await again.json()) as { message: string }).message).toBe('Программа уже настроена');
});
