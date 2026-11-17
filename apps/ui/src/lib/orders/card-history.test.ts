import type { AuditEntry } from '$api/AuditEntry';
import type { PageQuery } from '$api/PageQuery';
import { describe, expect, it, vi } from 'vitest';
import { createCardHistory, HISTORY_PAGE, TIMELINE_LIMIT } from './card-history.svelte.js';

function entry(id: number): AuditEntry {
	return {
		id,
		at: '2026-11-17T10:00:00',
		user_id: 1,
		user_name: 'Администратор',
		action: 'order.updated',
		order_id: 7,
		client_id: 3,
		summary: `Запись ${id}`,
		details: null
	};
}

/** A journal of `count` entries, newest (the biggest id) first, paged like the server does. */
function journal(count: number) {
	let all = Array.from({ length: count }, (_, i) => entry(count - i));
	return {
		add(n: number) {
			const top = all[0]?.id ?? 0;
			all = [...Array.from({ length: n }, (_, i) => entry(top + n - i)), ...all];
		},
		orderHistory: vi.fn(async (_id: number, p: PageQuery) => {
			const offset = p.offset ?? 0;
			return { items: all.slice(offset, offset + (p.limit ?? 50)), total: all.length };
		})
	};
}

describe('createCardHistory', () => {
	it('shows the first page and the newest entries, then pages on', async () => {
		const api = journal(120);
		const h = createCardHistory(api);
		await h.refresh(7);
		expect(api.orderHistory).toHaveBeenCalledWith(7, { limit: HISTORY_PAGE });
		expect(api.orderHistory).toHaveBeenCalledWith(7, { limit: TIMELINE_LIMIT });
		expect(h.entries.map((e) => e.id).slice(0, 2)).toEqual([120, 119]);
		expect(h.entries).toHaveLength(50);
		expect(h.newest).toHaveLength(120);
		expect(h.hasMore).toBe(true);

		await h.more();
		expect(api.orderHistory).toHaveBeenLastCalledWith(7, { limit: 50, offset: 50 });
		await h.more();
		expect(h.entries).toHaveLength(120);
		expect(h.entries.at(-1)?.id).toBe(1);
		expect(h.hasMore).toBe(false);
		await h.more();
		expect(api.orderHistory).toHaveBeenCalledTimes(4);
	});

	it('a new version puts its entries on top and keeps the pages shown', async () => {
		const api = journal(60);
		const h = createCardHistory(api);
		await h.refresh(7);
		await h.more();
		expect(h.entries).toHaveLength(60);
		api.add(2);
		await h.refresh(7);
		expect(h.entries).toHaveLength(62);
		expect(h.entries.slice(0, 3).map((e) => e.id)).toEqual([62, 61, 60]);
		expect(h.hasMore).toBe(false);
	});

	it('another order starts over; a failure with nothing shown says so', async () => {
		const api = journal(3);
		const h = createCardHistory(api);
		await h.refresh(7);
		expect(h.failed).toBe(false);
		api.orderHistory.mockRejectedValue(new Error('offline'));
		await h.refresh(7);
		expect(h.entries).toHaveLength(3);
		expect(h.failed).toBe(false);
		await h.refresh(8);
		expect(h.entries).toEqual([]);
		expect(h.failed).toBe(true);
	});

	it('an older answer does not overwrite a newer one', async () => {
		const api = journal(3);
		let release = () => {};
		api.orderHistory.mockImplementationOnce(
			() =>
				new Promise((r) => {
					release = () => r({ items: [entry(99)], total: 99 });
				})
		);
		const h = createCardHistory(api);
		const slow = h.refresh(7);
		await h.refresh(7);
		release();
		await slow;
		expect(h.entries.map((e) => e.id)).toEqual([3, 2, 1]);
		expect(h.hasMore).toBe(false);
	});
});
