import type { StockItem } from '$api/StockItem';
import type { StockItemPatch } from '$api/StockItemPatch';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { createStockEditor, type StockEditorApi } from './editor.svelte.js';

const conflict = () =>
	new ApiError(409, 'conflict', 'Запись изменена', {
		code: 'conflict',
		message: 'Запись изменена',
		fields: [],
		current_version: 0,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

function item(over: Partial<StockItem> = {}): StockItem {
	return {
		id: 7,
		name: '',
		display_name: 'Дисплей iPhone',
		device_type: 'Телефон',
		part_type: 'Дисплей',
		brand: 'Apple',
		model: 'iPhone 12',
		color: '',
		note: '',
		quantity: 5,
		price: 250000,
		restock_threshold: 2,
		needs_restock: false,
		created_at: '2026-11-24T10:00:00',
		updated_at: '2026-11-24T10:00:00',
		version: 1,
		...over
	};
}

/** A server of one position: writes check the version like mw-server does. */
function fakeServer(start: StockItem) {
	let current = start;
	const api = {
		getStockItem: vi.fn(async () => current),
		updateStockItem: vi.fn(async (_id: number, version: number, patch: StockItemPatch) => {
			await Promise.resolve();
			if (version !== current.version) throw conflict();
			current = { ...current, ...patch, version: current.version + 1 } as StockItem;
			return current;
		})
	} satisfies StockEditorApi;
	return {
		api,
		/** Someone writes a part off at another computer. */
		elsewhere(patch: Partial<StockItem>) {
			current = { ...current, ...patch, version: current.version + 1 };
		},
		get current() {
			return current;
		}
	};
}

const settle = async () => {
	for (let i = 0; i < 20; i++) await Promise.resolve();
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
	vi.useRealTimers();
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('createStockEditor', () => {
	it('saves the price 600 ms after the last key, as one PATCH with the current version', async () => {
		const server = fakeServer(item());
		const emit = vi.fn();
		const ed = createStockEditor(server.current, { api: server.api, emit });
		ed.autosave.edit('price', 300000);
		vi.advanceTimersByTime(599);
		expect(server.api.updateStockItem).not.toHaveBeenCalled();
		ed.autosave.edit('price', 300100);
		vi.advanceTimersByTime(599);
		expect(server.api.updateStockItem).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		await settle();
		expect(server.api.updateStockItem).toHaveBeenCalledTimes(1);
		expect(server.api.updateStockItem).toHaveBeenCalledWith(7, 1, { price: 300100 });
		expect(ed.item.version).toBe(2);
		expect(ed.item.price).toBe(300100);
		expect(emit).toHaveBeenCalledWith(ed.item);
		ed.dispose();
	});

	it('a stale quantity edit after a write-off is a conflict, not an overwrite', async () => {
		const server = fakeServer(item({ quantity: 5 }));
		const ed = createStockEditor(server.current, { api: server.api });
		// Another master wrote one off: 4 left, version 2.
		server.elsewhere({ quantity: 4 });
		ed.autosave.commit('quantity', 9);
		await expect(ed.autosave.flush()).rejects.toMatchObject({ code: 'conflict' });
		await settle();

		expect([...ed.conflicts.keys()]).toEqual(['quantity']);
		expect(ed.conflicts.get('quantity')).toEqual({ mine: 9, theirs: 4 });
		expect(ed.saveState).toBe('conflict');
		// The write-off was not overwritten.
		expect(server.current.quantity).toBe(4);

		ed.take('quantity');
		await settle();
		expect(ed.conflicts.size).toBe(0);
		expect(ed.item.quantity).toBe(4);
		expect(ed.autosave.value('quantity', ed.item.quantity)).toBe(4);
		expect(server.current.quantity).toBe(4);
		ed.dispose();
	});

	it('«Оставить моё» sends own quantity over the newer version', async () => {
		const server = fakeServer(item({ quantity: 5 }));
		const ed = createStockEditor(server.current, { api: server.api });
		server.elsewhere({ quantity: 4 });
		ed.autosave.commit('quantity', 9);
		await expect(ed.autosave.flush()).rejects.toMatchObject({ code: 'conflict' });
		await settle();
		ed.keep('quantity');
		await settle();
		expect(server.current.quantity).toBe(9);
		expect(ed.conflicts.size).toBe(0);
		ed.dispose();
	});

	it('a field changed only elsewhere is taken without a question', async () => {
		const server = fakeServer(item());
		const ed = createStockEditor(server.current, { api: server.api });
		server.elsewhere({ note: 'Заказать у поставщика' });
		ed.autosave.commit('price', 260000);
		await ed.flush();
		await settle();
		expect(ed.conflicts.size).toBe(0);
		expect(ed.item.note).toBe('Заказать у поставщика');
		expect(server.current.price).toBe(260000);
		ed.dispose();
	});

	it('a position deleted meanwhile is not offered again', async () => {
		const server = fakeServer(item());
		const ed = createStockEditor(server.current, { api: server.api });
		server.api.updateStockItem.mockRejectedValue(
			new ApiError(404, 'not_found', 'Позиция не найдена', null)
		);
		ed.autosave.edit('note', 'x');
		await expect(ed.flush()).resolves.toBe(false);
		expect(ed.lost).toBe('позиция удалена');
		ed.dispose();
	});
});
