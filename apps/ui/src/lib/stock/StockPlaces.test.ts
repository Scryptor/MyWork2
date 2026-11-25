import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { StockItem } from '$api/StockItem';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { createStockEditor } from './editor.svelte.js';
import { place, rack, stockItem } from './places.test-helper.js';

const api = vi.hoisted(() => ({
	putStock: vi.fn(),
	moveStock: vi.fn(),
	unplaceStock: vi.fn(),
	getStockItem: vi.fn(),
	updateStockItem: vi.fn(),
	listStock: vi.fn()
}));
vi.mock('#lib/api/stock.js', () => api);

import Harness from './StockPlacesHarness.test-helper.svelte';

const racks = [rack(1, 'A', 1, 2), rack(2, 'B', 2, 4)];
const exceed = () =>
	new ApiError(422, 'validation', 'Столько нет', {
		code: 'validation',
		message: 'Столько нет',
		fields: [{ field: 'quantity', code: 'places_exceed', message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

const editorOf = (item: StockItem) =>
	createStockEditor(item, {
		api: { getStockItem: api.getStockItem, updateStockItem: api.updateStockItem }
	});

/** The server's answer: the same position a version on, with its places. */
const next = (item: StockItem, over: Partial<StockItem>): StockItem => ({
	...item,
	version: item.version + 1,
	...over
});

/** The one option the typed code leaves in the open list (the popup is not in the accessibility tree of jsdom). */
const option = () =>
	waitFor(() => {
		const el = document.querySelector('[data-combobox-item]');
		if (!el) throw new Error('no option');
		return el;
	});

beforeEach(() => {
	vi.clearAllMocks();
});
afterEach(() => {
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('StockPlaces', () => {
	it('lists places and the unplaced rest', () => {
		const ed = editorOf(stockItem({ quantity: 5, places: [place('B-2-4', 3)], unplaced: 2 }));
		render(Harness, { editor: ed, racks });
		expect(screen.getByText('B-2-4 · 3 шт.')).toBeInTheDocument();
		expect(screen.getByText('Не разложено: 2')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: /^Положить…/ })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: /^Переложить…/ })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: /^Убрать из ячейки/ })).toBeInTheDocument();
	});

	it('says where racks are made when there are none', () => {
		const ed = editorOf(stockItem());
		render(Harness, { editor: ed, racks: [] });
		expect(screen.getByText('Шкафы и ячейки задаются на вкладке «Карта»')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /^Положить…/ })).not.toBeInTheDocument();
	});

	it('puts into a cell through the editor, after what was typed is saved', async () => {
		const ui = userEvent.setup();
		const item = stockItem({ unplaced: 5 });
		const ed = editorOf(item);
		api.updateStockItem.mockResolvedValue(next(item, { note: 'x' }));
		api.putStock.mockResolvedValue(
			next(next(item, { note: 'x' }), { places: [place('B-2-4', 2)], unplaced: 3 })
		);
		render(Harness, { editor: ed, racks });
		ed.autosave.commit('note', 'x');
		await ui.click(screen.getByRole('button', { name: /^Положить…/ }));
		const dialog = screen.getByRole('dialog', { name: 'Положить в ячейку' });
		await ui.type(within(dialog).getByRole('combobox', { name: 'Ячейка' }), 'B-2-4');
		await ui.click(await option());
		const qty = within(dialog).getByRole('textbox', { name: 'Количество' });
		await ui.clear(qty);
		await ui.type(qty, '2');
		await ui.click(within(dialog).getByRole('button', { name: 'Положить' }));
		await waitFor(() => expect(api.putStock).toHaveBeenCalled());
		expect(api.putStock).toHaveBeenCalledWith(7, {
			place: { rack_id: 2, shelf: 2, cell: 4 },
			quantity: 2
		});
		expect(api.updateStockItem.mock.invocationCallOrder[0]).toBeLessThan(
			api.putStock.mock.invocationCallOrder[0]!
		);
		await waitFor(() => expect(screen.getByTestId('unplaced')).toHaveTextContent('3'));
		expect(screen.getByText('B-2-4 · 2 шт.')).toBeInTheDocument();
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('moves and unplaces', async () => {
		const ui = userEvent.setup();
		const item = stockItem({ quantity: 3, places: [place('B-2-4', 3)], unplaced: 0 });
		const ed = editorOf(item);
		api.moveStock.mockResolvedValue(
			next(item, { places: [place('A-1', 3, 1, 1, 1)], unplaced: 0 })
		);
		render(Harness, { editor: ed, racks });
		await ui.click(screen.getByRole('button', { name: /^Переложить…/ }));
		const dialog = screen.getByRole('dialog', { name: 'Переложить в другую ячейку' });
		const cell = within(dialog).getByRole('combobox', { name: 'Ячейка' });
		await ui.type(cell, 'B-2-4');
		// The cell it lies in is not offered.
		expect(document.querySelector('[data-combobox-item]')).toBeNull();
		await ui.clear(cell);
		await ui.type(cell, 'A-1');
		await ui.click(await option());
		await ui.click(within(dialog).getByRole('button', { name: 'Переложить' }));
		await waitFor(() => expect(api.moveStock).toHaveBeenCalled());
		expect(api.moveStock).toHaveBeenCalledWith(7, {
			from: { rack_id: 2, shelf: 2, cell: 4 },
			to: { rack_id: 1, shelf: 1, cell: 1 },
			quantity: 3
		});
		expect(await screen.findByText('A-1 · 3 шт.')).toBeInTheDocument();
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

		api.unplaceStock.mockResolvedValue(next(ed.item, { places: [], unplaced: 3 }));
		await ui.click(screen.getByRole('button', { name: /^Убрать из ячейки/ }));
		await waitFor(() =>
			expect(api.unplaceStock).toHaveBeenCalledWith(7, { rack_id: 1, shelf: 1, cell: 1 })
		);
		expect(await screen.findByText('Не разложено: 3')).toBeInTheDocument();
	});

	it('explains places_exceed with the reloaded quantity', async () => {
		const ui = userEvent.setup();
		const item = stockItem({ unplaced: 5 });
		const ed = editorOf(item);
		api.putStock.mockRejectedValue(exceed());
		// Someone laid four units elsewhere meanwhile.
		api.getStockItem.mockResolvedValue(
			next(item, { places: [place('A-1', 4, 1, 1, 1)], unplaced: 1 })
		);
		render(Harness, { editor: ed, racks });
		await ui.click(screen.getByRole('button', { name: /^Положить…/ }));
		const dialog = screen.getByRole('dialog', { name: 'Положить в ячейку' });
		await ui.type(within(dialog).getByRole('combobox', { name: 'Ячейка' }), 'B-2-4');
		await ui.click(await option());
		await ui.click(within(dialog).getByRole('button', { name: 'Положить' }));
		expect(await within(dialog).findByText('Столько нет — не разложено 1 шт.')).toBeInTheDocument();
		expect(api.getStockItem).toHaveBeenCalledWith(7);
		expect(screen.getByTestId('unplaced')).toHaveTextContent('1');
	});

	it('hides actions without stock_edit', () => {
		const ed = editorOf(stockItem({ places: [place('B-2-4', 3)], unplaced: 2 }));
		render(Harness, { editor: ed, racks, canEdit: false });
		expect(screen.getByText('B-2-4 · 3 шт.')).toBeInTheDocument();
		expect(screen.getByText('Не разложено: 2')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /Положить|Переложить|Убрать/ })).toBeNull();
	});
});
