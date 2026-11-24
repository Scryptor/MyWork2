import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LegacyStockState } from '$api/LegacyStockState';

const api = vi.hoisted(() => ({ legacyStockState: vi.fn() }));
vi.mock('#lib/api/import.js', () => api);

import { createLegacyStock } from './legacy.svelte.js';

const flush = () => new Promise((r) => setTimeout(r, 0));
const state = (available: boolean, blocked = false): LegacyStockState => ({
	import_id: 3,
	stock_rows: 4,
	stock_map_rows: 6,
	available,
	blocked_by_stock: blocked
});
const later = () => {
	let answer!: (s: LegacyStockState) => void;
	const promise = new Promise<LegacyStockState>((r) => (answer = r));
	return { promise, answer };
};

beforeEach(() => vi.clearAllMocks());

// The stock of an import made before the stock module waits on «Импорт»: while it does, «Склад» warns
// that the first position closes the transfer (decision 17).
describe('createLegacyStock', () => {
	it('is pending while the server offers the transfer, not once a position exists', async () => {
		const legacy = createLegacyStock();
		expect(legacy.pending).toBe(false);
		api.legacyStockState.mockResolvedValueOnce(state(true));
		legacy.refresh();
		await flush();
		expect(legacy.pending).toBe(true);
		api.legacyStockState.mockResolvedValueOnce(state(false, true));
		legacy.refresh();
		await flush();
		expect(legacy.pending).toBe(false);
	});

	it('without an answer says nothing new', async () => {
		const legacy = createLegacyStock();
		api.legacyStockState.mockRejectedValueOnce(new Error('Нет связи с сервером'));
		legacy.refresh();
		await flush();
		expect(legacy.pending).toBe(false);
		api.legacyStockState.mockResolvedValueOnce(state(true));
		legacy.refresh();
		await flush();
		api.legacyStockState.mockRejectedValueOnce(new Error('Нет связи с сервером'));
		legacy.refresh();
		await flush();
		expect(legacy.pending).toBe(true);
	});

	it('the newest answer wins', async () => {
		const legacy = createLegacyStock();
		const first = later();
		const second = later();
		api.legacyStockState.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
		legacy.refresh();
		legacy.refresh();
		second.answer(state(false, true));
		await flush();
		first.answer(state(true));
		await flush();
		expect(legacy.pending).toBe(false);
	});
});
