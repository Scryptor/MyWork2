import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { client, clientListItem, order } from '#lib/app/fixtures.test-helper.js';
import { PAGE_SIZE, createClientList } from './list.svelte.js';

interface Call {
	q: ClientListQuery;
	signal: AbortSignal | undefined;
	answer: (p: ClientPage) => void;
	fail: (e: unknown) => void;
}

/** A listClients whose answers the test gives by hand, in any order. */
function fakeServer() {
	const calls: Call[] = [];
	const fetchPage = vi.fn(
		(q: ClientListQuery, signal?: AbortSignal) =>
			new Promise<ClientPage>((answer, fail) => {
				calls.push({ q, signal, answer, fail });
				signal?.addEventListener('abort', () =>
					fail(new DOMException('The operation was aborted.', 'AbortError'))
				);
			})
	);
	return { calls, fetchPage };
}

const rows = (from: number, n: number) =>
	Array.from({ length: n }, (_, i) => clientListItem(from + i));
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createClientList', () => {
	it('pages of 100; a new load aborts the previous one; a page more is a new array', async () => {
		expect(PAGE_SIZE).toBe(100);
		const { calls, fetchPage } = fakeServer();
		const list = createClientList(fetchPage);
		list.load({ query: 'старый' });
		list.load({ query: 'новый', sort: 'orders_count', descending: true });
		expect(calls[0]!.signal?.aborted).toBe(true);
		expect(calls[1]!.q).toEqual({
			query: 'новый',
			sort: 'orders_count',
			descending: true,
			limit: 100,
			offset: 0
		});
		calls[1]!.answer({ items: rows(1, 100), total: 130 });
		await flush();
		expect(list.loading).toBe(false);
		expect(list.total).toBe(130);
		const first = list.rows;
		list.loadMore();
		list.loadMore();
		expect(calls).toHaveLength(3);
		expect(calls[2]!.q.offset).toBe(100);
		// One that moved between pages is not shown twice.
		calls[2]!.answer({ items: rows(100, 31), total: 130 });
		await flush();
		expect(list.rows).not.toBe(first);
		expect(list.rows).toHaveLength(130);
		list.loadMore();
		expect(calls).toHaveLength(3);
	});

	it('a failed load drops the rows; a failed page more keeps them', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createClientList(fetchPage);
		list.load({});
		calls[0]!.answer({ items: rows(1, 100), total: 150 });
		await flush();
		list.loadMore();
		calls[1]!.fail(new ApiError(0, 'network', 'Нет связи с сервером', null));
		await flush();
		expect(list.rows).toHaveLength(100);
		expect(list.error?.code).toBe('network');
		list.load({ query: 'x' });
		expect(list.error).toBeNull();
		calls[2]!.fail(new TypeError('boom'));
		await flush();
		expect(list.rows).toEqual([]);
		expect(list.total).toBe(0);
		expect(list.error).toBeInstanceOf(ApiError);
	});

	it('a client saved in the panel updates its row; the order counts stay', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createClientList(fetchPage);
		list.load({});
		const row = clientListItem(7, { orders_count: 3, last_order_at: '2026-11-15T10:24:00' });
		calls[0]!.answer({ items: [clientListItem(6), row], total: 2 });
		await flush();
		const before = list.rows;
		list.replace(
			client(7, {
				full_name: 'ПЕТРОВА АННА',
				phones: [{ phone: '79211234567', note: '' }],
				source: 'Авито',
				blacklisted: true,
				version: 2
			})
		);
		expect(list.rows).not.toBe(before);
		expect(list.rows[1]).toEqual({
			...row,
			full_name: 'ПЕТРОВА АННА',
			phones: [{ phone: '79211234567', note: '' }],
			source: 'Авито',
			blacklisted: true
		});
		expect(list.rows[0]).toBe(before[0]);
		const now = list.rows;
		list.replace(client(42));
		expect(list.rows).toBe(now);
	});

	it('an order accepted counts in its client row only: no page is asked again', async () => {
		const { calls, fetchPage } = fakeServer();
		const list = createClientList(fetchPage);
		list.load({});
		const row = clientListItem(7, { orders_count: 3, last_order_at: '2026-11-15T10:24:00' });
		calls[0]!.answer({ items: [clientListItem(6), row], total: 2 });
		await flush();
		const before = list.rows;
		list.orderCreated(order(900, { client_id: 7, received_at: '2026-11-19T09:00:00' }));
		expect(calls).toHaveLength(1);
		expect(list.rows[1]).toEqual({ ...row, orders_count: 4, last_order_at: '2026-11-19T09:00:00' });
		expect(list.rows[0]).toBe(before[0]);
		// A client that is not in the list is not added; the total stays.
		const now = list.rows;
		list.orderCreated(order(901, { client_id: 42 }));
		expect(list.rows).toBe(now);
		expect(list.total).toBe(2);
		expect(calls).toHaveLength(1);
	});
});
