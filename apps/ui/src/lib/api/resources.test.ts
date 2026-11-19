import { afterEach, expect, it, vi } from 'vitest';
import { listClients, mergeClients, similarClients, updateClient } from './clients.js';
import { changeStatus, createOrder, orderValues, listOrders } from './orders.js';

function stubFetch() {
	const fn = vi.fn(async () => new Response('{}', { status: 200 }));
	vi.stubGlobal('fetch', fn);
	return fn;
}
const call = (fn: ReturnType<typeof stubFetch>) =>
	fn.mock.calls[0] as unknown as [string, RequestInit];

afterEach(() => vi.unstubAllGlobals());

it('passes the version as a query parameter', async () => {
	const fn = stubFetch();
	await changeStatus(5, 3, { status_id: 2 });
	const [url, init] = call(fn);
	expect(url).toBe('/api/v1/orders/5/status?version=3');
	expect(init.method).toBe('POST');
	expect(init.body).toBe('{"status_id":2}');
});

it('lists orders with repeated status ids', async () => {
	const fn = stubFetch();
	await listOrders({ status_ids: [1, 2], limit: 10 });
	expect(call(fn)[0]).toBe('/api/v1/orders?status_ids=1&status_ids=2&limit=10');
});

it('skips the brand when it is not given', async () => {
	const fn = stubFetch();
	await orderValues('model', 'ip');
	expect(call(fn)[0]).toBe('/api/v1/orders/values?field=model&query=ip');
});

it('edits a client with its version; similar and merge have their own paths', async () => {
	const fn = stubFetch();
	await updateClient(7, 4, { phones: [{ phone: '79211234567', note: 'жена' }] });
	expect(call(fn)[0]).toBe('/api/v1/clients/7?version=4');
	expect(call(fn)[1].method).toBe('PATCH');
	expect(call(fn)[1].body).toBe('{"phones":[{"phone":"79211234567","note":"жена"}]}');
	fn.mockClear();
	await similarClients(7);
	expect(call(fn)[0]).toBe('/api/v1/clients/7/similar');
	fn.mockClear();
	await mergeClients({
		target_id: 7,
		target_version: 4,
		sources: [{ id: 8, version: 1 }],
		full_name: 'Тестова Анна',
		address: '',
		note: '',
		source: '',
		blacklisted: false
	});
	expect(call(fn)[0]).toBe('/api/v1/clients/merge');
	expect(call(fn)[1].method).toBe('POST');
	fn.mockClear();
	await listClients({ query: 'ив', sort: 'orders_count', descending: true, limit: 100, offset: 0 });
	expect(call(fn)[0]).toBe(
		'/api/v1/clients?query=%D0%B8%D0%B2&sort=orders_count&descending=true&limit=100&offset=0'
	);
});

it('createOrder tells a replayed answer (X-MW-Replayed: 1) from a first one', async () => {
	const answer = (headers: Record<string, string>) =>
		vi.fn(async () => new Response('{"id":5}', { status: 201, headers }));
	vi.stubGlobal('fetch', answer({}));
	await expect(createOrder({} as never)).resolves.toEqual({ order: { id: 5 }, replayed: false });
	vi.stubGlobal('fetch', answer({ 'X-MW-Replayed': '1' }));
	await expect(createOrder({} as never)).resolves.toEqual({ order: { id: 5 }, replayed: true });
});
