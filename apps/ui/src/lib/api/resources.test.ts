import { afterEach, expect, it, vi } from 'vitest';
import { changeStatus, orderValues, listOrders } from './orders.js';

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
