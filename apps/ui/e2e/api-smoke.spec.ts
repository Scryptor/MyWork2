import { expect, test } from '@playwright/test';
import { createOrder, uniq } from './support/api';

test('the server serves the UI and the API on one origin', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	expect(o.id).toBeGreaterThan(0);
	await page.goto('/design');
	await expect(page.getByRole('heading', { level: 1 })).toBeVisible();
});

test('unknown API paths are JSON 404, other paths fall back to the SPA', async ({ request }) => {
	const api = await request.get('/api/v1/nope');
	expect(api.status()).toBe(404);
	expect((await api.json()).code).toBe('not_found');
	const page = await request.get('/some/spa/route');
	expect(page.status()).toBe(200);
	expect(await page.text()).toContain('<html');
});
