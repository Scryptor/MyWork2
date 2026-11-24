import type { AuditEntry } from '$api/AuditEntry';
import type { HistoryQuery } from '$api/HistoryQuery';
import { describe, expect, it, vi } from 'vitest';
import {
	createCardHistory,
	HISTORY_PAGE,
	TIMELINE_MAX,
	TIMELINE_PAGE
} from './card-history.svelte.js';

/** `clock`: how the time of an entry follows its id; the same time for all by default. */
let clock: (id: number) => string = () => '2026-11-17T10:00:00';

function entry(id: number): AuditEntry {
	return {
		id,
		at: clock(id),
		user_id: 1,
		user_name: 'Администратор',
		// The oldest entry is the intake, as on the server.
		action: id === 1 ? 'order.created' : 'order.updated',
		order_id: 7,
		client_id: 3,
		stock_item_id: null,
		summary: `Запись ${id}`,
		details: null
	};
}

/** The server's order: time falling, then id falling. */
const shown = (a: AuditEntry, b: AuditEntry) =>
	a.at === b.at ? b.id - a.id : a.at < b.at ? 1 : -1;
const later = (e: AuditEntry, q: HistoryQuery) =>
	q.before_at === undefined ||
	e.at < q.before_at ||
	(e.at === q.before_at && e.id < (q.before_id ?? 0));

/** A journal of `count` entries, newest (the biggest id) first, paged like the server does. */
function journal(count: number) {
	let all = Array.from({ length: count }, (_, i) => entry(count - i)).sort(shown);
	return {
		add(n: number) {
			const top = Math.max(0, ...all.map((e) => e.id));
			all = [...Array.from({ length: n }, (_, i) => entry(top + n - i)), ...all].sort(shown);
		},
		orderHistory: vi.fn(async (_id: number, p: HistoryQuery) => {
			const older = all.filter((e) => later(e, p));
			const offset = p.offset ?? 0;
			return { items: older.slice(offset, offset + (p.limit ?? 50)), total: all.length };
		})
	};
}

describe('createCardHistory', () => {
	it('one request opens the history: the newest page of the journal serves both lists', async () => {
		const api = journal(120);
		const h = createCardHistory(api);
		await h.refresh(7);
		expect(api.orderHistory).toHaveBeenCalledTimes(1);
		expect(api.orderHistory).toHaveBeenCalledWith(7, { limit: TIMELINE_PAGE });
		expect(h.entries.map((e) => e.id).slice(0, 2)).toEqual([120, 119]);
		expect(h.entries).toHaveLength(HISTORY_PAGE);
		expect(h.timeline).toHaveLength(120);
		expect(h.timelineCut).toBeNull();
		expect(h.hasMore).toBe(true);

		await h.more();
		expect(api.orderHistory).toHaveBeenLastCalledWith(7, {
			limit: 50,
			before_at: '2026-11-17T10:00:00',
			before_id: 71
		});
		await h.more();
		expect(h.entries).toHaveLength(120);
		expect(h.entries.at(-1)?.id).toBe(1);
		expect(h.hasMore).toBe(false);
		await h.more();
		expect(api.orderHistory).toHaveBeenCalledTimes(3);
	});

	it('every new version costs one request and reads only what is new', async () => {
		const api = journal(60);
		const h = createCardHistory(api);
		await h.refresh(7);
		await h.more();
		api.orderHistory.mockClear();
		api.add(2);
		await h.refresh(7);
		expect(api.orderHistory).toHaveBeenCalledTimes(1);
		expect(h.entries).toHaveLength(62);
		expect(h.entries.slice(0, 3).map((e) => e.id)).toEqual([62, 61, 60]);
		expect(h.hasMore).toBe(false);
	});

	it('paging goes by the last id: 500 new entries meanwhile do not repeat a page', async () => {
		const api = journal(120);
		const h = createCardHistory(api);
		await h.refresh(7);
		expect(h.entries).toHaveLength(50);
		api.add(500);
		await h.more();
		const ids = h.entries.map((e) => e.id);
		expect(new Set(ids).size).toBe(ids.length);
		expect(h.entries).toHaveLength(100);
		expect(ids.slice(-1)[0]).toBe(21);
		expect(ids.slice(0, 1)[0]).toBe(120);
	});

	it('the timeline pages by id until the whole journal is there: the intake is the oldest entry', async () => {
		const api = journal(1200);
		const h = createCardHistory(api);
		await h.refresh(7);
		expect(api.orderHistory.mock.calls.map(([, p]) => p)).toEqual([
			{ limit: TIMELINE_PAGE },
			{ limit: TIMELINE_PAGE, before_at: '2026-11-17T10:00:00', before_id: 701 },
			{ limit: TIMELINE_PAGE, before_at: '2026-11-17T10:00:00', before_id: 201 }
		]);
		expect(h.timeline).toHaveLength(1200);
		expect(h.timeline.at(-1)?.action).toBe('order.created');
		expect(h.timelineCut).toBeNull();

		api.orderHistory.mockClear();
		api.add(1);
		await h.refresh(7);
		expect(api.orderHistory).toHaveBeenCalledTimes(1);
		expect(h.timeline).toHaveLength(1201);
		expect(h.timeline[0]?.id).toBe(1201);
	});

	it('a failed next page says so and «Повторить» continues; the first page stays', async () => {
		const api = journal(120);
		const h = createCardHistory(api);
		await h.refresh(7);
		api.orderHistory.mockRejectedValueOnce(new Error('offline'));
		await h.more();
		expect(h.moreFailed).toBe(true);
		expect(h.failed).toBe(false);
		expect(h.entries).toHaveLength(50);
		expect(h.hasMore).toBe(true);
		await h.more();
		expect(h.moreFailed).toBe(false);
		expect(h.entries).toHaveLength(100);
	});

	it('a failed next page of the timeline keeps what came and a refresh goes on from there', async () => {
		const api = journal(1200);
		const h = createCardHistory(api);
		api.orderHistory.mockImplementation(async (_id, p) => {
			if (p.before_id === 201) throw new Error('offline');
			const all = Array.from({ length: 1200 }, (_, i) => entry(1200 - i));
			const older = all.filter((e) => later(e, p));
			return { items: older.slice(0, p.limit ?? 50), total: 1200 };
		});
		await h.refresh(7);
		expect(h.timelineFailed).toBe(true);
		expect(h.failed).toBe(false);
		expect(h.timeline).toHaveLength(1000);
		api.orderHistory.mockImplementation(async (_id, p) => {
			const all = Array.from({ length: 1200 }, (_, i) => entry(1200 - i));
			const older = all.filter((e) => later(e, p));
			return { items: older.slice(0, p.limit ?? 50), total: 1200 };
		});
		await h.refresh(7);
		expect(h.timelineFailed).toBe(false);
		expect(h.timeline).toHaveLength(1200);
		expect(h.timeline.at(-1)?.action).toBe('order.created');
	});

	it('pages by time and id: a clock that went back repeats nothing and skips nothing', async () => {
		// Ids grow, the time dips: 41..50 are earlier than 31..40.
		clock = (id) => `2026-11-17T${id > 40 ? '08' : '10'}:${String(id % 60).padStart(2, '0')}:00`;
		try {
			const api = journal(120);
			const h = createCardHistory(api);
			await h.refresh(7);
			while (h.hasMore) await h.more();
			const ids = h.entries.map((e) => e.id);
			expect(new Set(ids).size).toBe(120);
			expect(h.timeline).toHaveLength(120);
		} finally {
			clock = () => '2026-11-17T10:00:00';
		}
	});

	it('pages on after the last page it read: a new entry on top with an earlier time skips nothing', async () => {
		const api = journal(120);
		const h = createCardHistory(api);
		await h.refresh(7);
		// The clock went back: the next entry is earlier than every one shown.
		clock = (id) => (id > 120 ? '2026-11-17T08:00:00' : '2026-11-17T10:00:00');
		try {
			api.add(1);
			await h.refresh(7);
			expect(h.entries[0]?.id).toBe(121);
			await h.more();
			expect(api.orderHistory).toHaveBeenLastCalledWith(7, {
				limit: HISTORY_PAGE,
				before_at: '2026-11-17T10:00:00',
				before_id: 71
			});
			while (h.hasMore) await h.more();
			const ids = h.entries.map((e) => e.id);
			expect(new Set(ids).size).toBe(121);
		} finally {
			clock = () => '2026-11-17T10:00:00';
		}
	});

	it('a journal longer than the limit shows the newest part and says so', async () => {
		expect(TIMELINE_MAX).toBe(20 * TIMELINE_PAGE);
		const api = journal(1200);
		const h = createCardHistory(api, { timelineMax: 1000 });
		await h.refresh(7);
		expect(h.timeline).toHaveLength(1000);
		expect(h.timelineCut).toBe(1000);
		expect(h.timeline.some((e) => e.action === 'order.created')).toBe(false);
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
