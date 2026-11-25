import type { StockItem } from '$api/StockItem';
import type { StockListQuery } from '$api/StockListQuery';
import type { StockPage } from '$api/StockPage';
import { describe, expect, it, vi } from 'vitest';
import { PAGE_SIZE, createStockList } from './list.svelte.js';

const item = (id: number): StockItem => ({
	id,
	name: '',
	display_name: `Позиция №${id}`,
	device_type: '',
	part_type: 'Дисплей',
	brand: '',
	model: `M-${id}`,
	color: '',
	note: '',
	quantity: 3,
	price: 250000,
	restock_threshold: 5,
	needs_restock: true,
	created_at: '2026-11-24T10:00:00',
	updated_at: '2026-11-24T10:00:00',
	version: 1,
	places: [],
	unplaced: 3
});
const flush = () => new Promise((r) => setTimeout(r, 0));

describe('createStockList', () => {
	it('asks pages of 100 with the view, a new load aborts the previous one', async () => {
		expect(PAGE_SIZE).toBe(100);
		const calls: { q: StockListQuery; signal?: AbortSignal; answer: (p: StockPage) => void }[] = [];
		const fetchPage = vi.fn(
			(q: StockListQuery, signal?: AbortSignal) =>
				new Promise<StockPage>((answer, fail) => {
					calls.push({ q, signal, answer });
					signal?.addEventListener('abort', () => fail(new DOMException('x', 'AbortError')));
				})
		);
		const list = createStockList(fetchPage);
		list.load({ query: 'старый' });
		list.load({ restock: true, sort: 'price', descending: true });
		expect(calls[0]!.signal?.aborted).toBe(true);
		expect(calls[1]!.q).toEqual({
			restock: true,
			sort: 'price',
			descending: true,
			limit: 100,
			offset: 0
		});
		calls[1]!.answer({ items: [item(1), item(2)], total: 2 });
		await flush();
		expect(list.rows.map((r) => r.id)).toEqual([1, 2]);
		expect(list.total).toBe(2);
		expect(list.loading).toBe(false);
	});
});
