import type { AuditEntry } from '$api/AuditEntry';
import type { AuditPage } from '$api/AuditPage';
import type { HistoryQuery } from '$api/HistoryQuery';
import { describe, expect, it, vi } from 'vitest';
import { createStockHistory } from './history.svelte.js';

const entry = (id: number): AuditEntry =>
	({
		id,
		at: `2026-11-24T10:${String(id).padStart(2, '0')}:00`,
		user_name: 'Админ',
		summary: `Изменено: поле ${id}`
	}) as AuditEntry;

/** A journal of `n` entries, newest first, served by cursor like the server does. */
function journal(n: number) {
	const all = Array.from({ length: n }, (_, i) => entry(n - i));
	return vi.fn(async (_id: number, q: HistoryQuery = {}): Promise<AuditPage> => {
		const rest = q.before_id === undefined ? all : all.filter((e) => e.id < q.before_id!);
		return { items: rest.slice(0, q.limit ?? 50), total: n } as AuditPage;
	});
}

describe('createStockHistory', () => {
	it('shows the first page; «Показать ещё» loads the next, the last page ends it', async () => {
		const api = journal(5);
		const h = createStockHistory(api, 2);
		await h.refresh(7);
		expect(h.entries.map((e) => e.id)).toEqual([5, 4]);
		expect(h.hasMore).toBe(true);
		await h.more();
		expect(h.entries.map((e) => e.id)).toEqual([5, 4, 3, 2]);
		expect(h.hasMore).toBe(true);
		await h.more();
		expect(h.entries.map((e) => e.id)).toEqual([5, 4, 3, 2, 1]);
		expect(h.hasMore).toBe(false);
		expect(api).toHaveBeenLastCalledWith(7, { limit: 2, before_at: entry(2).at, before_id: 2 });
	});

	it('a short first page has nothing more', async () => {
		const h = createStockHistory(journal(2), 5);
		await h.refresh(7);
		expect(h.hasMore).toBe(false);
	});

	it('a failed «Показать ещё» keeps the page and can be asked again', async () => {
		const api = journal(4);
		const h = createStockHistory(api, 2);
		await h.refresh(7);
		api.mockRejectedValueOnce(new Error('offline'));
		await h.more();
		expect(h.moreFailed).toBe(true);
		expect(h.entries).toHaveLength(2);
		await h.more();
		expect(h.moreFailed).toBe(false);
		expect(h.entries).toHaveLength(4);
	});

	it('a refresh after a new version adds the new entries on top and keeps the older pages', async () => {
		let n = 4;
		const api = vi.fn(async (_id: number, q: HistoryQuery = {}) => {
			const all = Array.from({ length: n }, (_, i) => entry(n - i));
			const rest = q.before_id === undefined ? all : all.filter((e) => e.id < q.before_id!);
			return { items: rest.slice(0, q.limit ?? 50), total: n } as AuditPage;
		});
		const h = createStockHistory(api, 2);
		await h.refresh(7);
		await h.more();
		n = 5;
		await h.refresh(7);
		expect(h.entries.map((e) => e.id)).toEqual([5, 4, 3, 2, 1]);
	});
});
