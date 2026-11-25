import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { item, order } from '#lib/app/fixtures.test-helper.js';
import { PAGE_SIZE, createOrderList, withOrder } from './list.svelte.js';

interface Call {
	q: OrderListQuery;
	signal: AbortSignal | undefined;
	answer: (p: OrderPage) => void;
	fail: (e: unknown) => void;
}

/** A listOrders whose answers the test gives by hand, in any order. */
function fakeServer() {
	const calls: Call[] = [];
	const fetchPage = vi.fn(
		(q: OrderListQuery, signal?: AbortSignal) =>
			new Promise<OrderPage>((answer, fail) => {
				calls.push({ q, signal, answer, fail });
				signal?.addEventListener('abort', () =>
					fail(new DOMException('The operation was aborted.', 'AbortError'))
				);
			})
	);
	return { calls, fetchPage };
}

const ids = (from: number, n: number) => Array.from({ length: n }, (_, i) => item(from + i));
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createOrderList', () => {
	it('a new load aborts the previous one and loadMore makes a new array', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({ query: 'старый' });
		list.load({ query: 'новый' });
		expect(list.loading).toBe(true);
		expect(calls[0]!.signal?.aborted).toBe(true);
		expect(calls[1]!.q).toEqual({ query: 'новый', limit: PAGE_SIZE, offset: 0 });
		calls[1]!.answer({ items: ids(1, PAGE_SIZE), total: 150 });
		await flush();
		expect(list.loading).toBe(false);
		expect(list.rows).toHaveLength(PAGE_SIZE);
		expect(list.total).toBe(150);

		const first = list.rows;
		list.loadMore();
		list.loadMore(); // one request at a time
		expect(calls).toHaveLength(3);
		expect(calls[2]!.q).toEqual({ query: 'новый', limit: PAGE_SIZE, offset: PAGE_SIZE });
		calls[2]!.answer({ items: ids(PAGE_SIZE + 1, 50), total: 150 });
		await flush();
		expect(list.rows).not.toBe(first);
		expect(list.rows).toHaveLength(150);
		expect(first).toHaveLength(PAGE_SIZE);

		// Everything is here: nothing more to ask.
		list.loadMore();
		expect(calls).toHaveLength(3);
	});

	it('the page size is 100', () => {
		expect(PAGE_SIZE).toBe(100);
	});

	it('an answer to an aborted load changes nothing', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({ query: 'а' });
		list.load({ query: 'б' });
		calls[1]!.answer({ items: ids(10, 2), total: 2 });
		calls[0]!.answer({ items: ids(1, 5), total: 5 });
		await flush();
		expect(list.rows.map((r) => r.id)).toEqual([10, 11]);
		expect(list.total).toBe(2);
	});

	it('a new load drops a page more that is on its way', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({});
		calls[0]!.answer({ items: ids(1, PAGE_SIZE), total: 300 });
		await flush();
		list.loadMore();
		list.load({ query: 'другое' });
		expect(calls[1]!.signal?.aborted).toBe(true);
		calls[2]!.answer({ items: ids(500, 1), total: 1 });
		await flush();
		expect(list.rows.map((r) => r.id)).toEqual([500]);
	});

	it('orders that moved between pages are not shown twice', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({});
		calls[0]!.answer({ items: ids(1, PAGE_SIZE), total: 102 });
		await flush();
		list.loadMore();
		calls[1]!.answer({ items: [item(PAGE_SIZE), item(PAGE_SIZE + 1)], total: 102 });
		await flush();
		expect(list.rows.map((r) => r.id).slice(-2)).toEqual([PAGE_SIZE, PAGE_SIZE + 1]);
		expect(new Set(list.rows.map((r) => r.id)).size).toBe(list.rows.length);
	});

	it('a failed load drops the rows of the last view and keeps its error until the next one', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({});
		calls[0]!.answer({ items: ids(1, 3), total: 3 });
		await flush();
		list.load({ query: 'другое' });
		const e = new ApiError(0, 'network', 'Нет связи с сервером', null);
		calls[1]!.fail(e);
		await flush();
		expect(list.error).toBe(e);
		expect(list.loading).toBe(false);
		expect(list.rows).toEqual([]);
		expect(list.total).toBe(0);
		list.load({});
		expect(list.error).toBeNull();
		calls[2]!.fail(new TypeError('boom'));
		await flush();
		expect(list.error).toBeInstanceOf(ApiError);
	});

	it('a failed page more keeps the rows and may be asked again', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({});
		calls[0]!.answer({ items: ids(1, PAGE_SIZE), total: 150 });
		await flush();
		list.loadMore();
		calls[1]!.fail(new ApiError(0, 'network', 'Нет связи с сервером', null));
		await flush();
		expect(list.rows).toHaveLength(PAGE_SIZE);
		expect(list.error).not.toBeNull();
		list.loadMore();
		expect(calls[2]!.q.offset).toBe(PAGE_SIZE);
		expect(list.error).toBeNull();
	});

	it('replace updates the order fields of a row and leaves the client fields', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createOrderList(fetchPage);
		list.load({});
		const row = item(7, {
			client_name: 'ТЕСТОВ ИВАН',
			client_phone: '+79001112233',
			client_blacklisted: true,
			status_id: 1
		});
		calls[0]!.answer({ items: [item(6), row, item(8)], total: 3 });
		await flush();
		const before = list.rows;
		list.replace(
			order(7, {
				client_id: 999,
				status_id: 3,
				master: 'Антон',
				final_price: 650000,
				need_call: true,
				version: 2
			})
		);
		expect(list.rows).not.toBe(before);
		expect(list.rows[1]).toEqual({
			...row,
			status_id: 3,
			master: 'Антон',
			final_price: 650000,
			need_call: true,
			version: 2
		});
		expect(list.rows[0]).toBe(before[0]);

		// A row that no longer fits the view stays until the next load; an older answer is ignored.
		list.replace(order(7, { status_id: 1, version: 1 }));
		expect(list.rows[1]!.status_id).toBe(3);
		// An order that is not in the list is not added.
		const now = list.rows;
		list.replace(order(42, { version: 5 }));
		expect(list.rows).toBe(now);
	});
});

describe('withOrder', () => {
	// The client's card takes a changed order into its row the same way.
	it('takes the order fields a row shows, the day it was received too; the client fields stay', () => {
		const row = item(7, { client_name: 'ТЕСТОВ ИВАН', received_at: '2026-11-15T10:24:00' });
		const o = order(7, {
			client_id: 999,
			received_at: '2026-11-14T09:00:00',
			issued_at: '2026-11-20T12:00:00',
			discount: 20000,
			version: 4
		});
		expect(withOrder(row, o)).toEqual({
			...row,
			received_at: '2026-11-14T09:00:00',
			issued_at: '2026-11-20T12:00:00',
			discount: 20000,
			version: 4
		});
	});
});
