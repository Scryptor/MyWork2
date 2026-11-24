import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StockSummary } from '$api/StockSummary';

const api = vi.hoisted(() => ({ stockSummary: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);

import { resetSession } from '#lib/app/session-reset.js';
import { stockCount } from './summary.svelte.js';

const flush = () => new Promise((r) => setTimeout(r, 0));
const later = () => {
	let answer!: (s: StockSummary) => void;
	const promise = new Promise<StockSummary>((r) => (answer = r));
	return { promise, answer };
};

beforeEach(() => {
	vi.clearAllMocks();
	stockCount.reset();
});

describe('stockCount', () => {
	it('shows the answer of the server', async () => {
		api.stockSummary.mockResolvedValue({ restock: 4 });
		expect(stockCount.restock).toBeNull();
		stockCount.refresh();
		await flush();
		expect(stockCount.restock).toBe(4);
	});

	it('an older answer that comes late does not overwrite a newer one', async () => {
		const first = later();
		const second = later();
		api.stockSummary.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
		stockCount.refresh();
		stockCount.refresh();
		second.answer({ restock: 2 });
		await flush();
		first.answer({ restock: 9 });
		await flush();
		expect(stockCount.restock).toBe(2);
	});

	it('a failed request keeps the last count', async () => {
		api.stockSummary.mockResolvedValueOnce({ restock: 3 }).mockRejectedValueOnce(new Error('x'));
		stockCount.refresh();
		await flush();
		stockCount.refresh();
		await flush();
		expect(stockCount.restock).toBe(3);
	});

	it('signing out forgets the count and ignores an answer still on its way', async () => {
		const pending = later();
		api.stockSummary.mockResolvedValueOnce({ restock: 3 }).mockReturnValueOnce(pending.promise);
		stockCount.refresh();
		await flush();
		stockCount.refresh();
		resetSession();
		expect(stockCount.restock).toBeNull();
		pending.answer({ restock: 7 });
		await flush();
		expect(stockCount.restock).toBeNull();
	});
});
