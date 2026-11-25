import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { place, rack, stockItem } from '../places.test-helper.js';

const api = vi.hoisted(() => ({ listStock: vi.fn(), putStock: vi.fn(), getStockItem: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);
const print = vi.hoisted(() => ({ openPreview: vi.fn() }));
vi.mock('#lib/print/print.svelte.js', () => print);

import Harness from './CellPanelHarness.test-helper.svelte';

const racks = [rack(2, 'B', 2, 4)];
const here = stockItem({ id: 7, places: [place('B-2-4', 3)], unplaced: 2 });
const spare = stockItem({ id: 9, display_name: 'Батарея iPhone', unplaced: 4 });

beforeEach(() => {
	vi.clearAllMocks();
	api.listStock.mockImplementation(async (q: { place?: string }) =>
		q.place
			? { items: [here], total: 1, place: 'B-2-4' }
			: { items: [spare], total: 1, place: null }
	);
});

describe('CellPanel', () => {
	it('lists what lies in the cell', async () => {
		const onopenitem = vi.fn();
		render(Harness, { racks, onopenitem });
		expect(screen.getByRole('dialog', { name: 'Ячейка B-2-4' })).toBeInTheDocument();
		const row = await screen.findByRole('button', { name: /Дисплей iPhone/ });
		expect(row).toHaveTextContent('3 шт.');
		expect(api.listStock).toHaveBeenCalledWith(expect.objectContaining({ place: 'B-2-4' }));
		await userEvent.click(row);
		expect(onopenitem).toHaveBeenCalledWith(7);
	});

	it('says so when the cell is empty', async () => {
		api.listStock.mockResolvedValue({ items: [], total: 0, place: 'B-2-4' });
		render(Harness, { racks });
		expect(await screen.findByText('В ячейке пусто')).toBeInTheDocument();
	});

	it('puts a chosen position here', async () => {
		const ui = userEvent.setup();
		const onchanged = vi.fn();
		api.putStock.mockResolvedValue(here);
		render(Harness, { racks, onchanged });
		await screen.findByRole('button', { name: /Дисплей iPhone/ });
		await ui.click(screen.getByRole('button', { name: 'Положить сюда…' }));
		await ui.type(screen.getByRole('combobox', { name: 'Позиция' }), 'бат');
		await ui.click(
			await waitFor(() => {
				const el = document.querySelector('[data-combobox-item]');
				if (!el) throw new Error('no option');
				return el;
			})
		);
		const before = api.listStock.mock.calls.filter(([q]) => q.place).length;
		await ui.click(screen.getByRole('button', { name: 'Положить' }));
		await waitFor(() =>
			expect(api.putStock).toHaveBeenCalledWith(9, {
				place: { rack_id: 2, shelf: 2, cell: 4 },
				quantity: 4
			})
		);
		await waitFor(() => expect(onchanged).toHaveBeenCalled());
		expect(api.listStock.mock.calls.filter(([q]) => q.place).length).toBe(before + 1);
	});

	it('hides «Положить сюда…» without the right to edit', async () => {
		render(Harness, { racks, canEdit: false });
		await screen.findByRole('button', { name: /Дисплей iPhone/ });
		expect(screen.queryByRole('button', { name: 'Положить сюда…' })).toBeNull();
	});

	it('prints the sticker of the cell', async () => {
		render(Harness, { racks });
		await userEvent.click(screen.getByRole('button', { name: 'Печать наклейки' }));
		expect(print.openPreview).toHaveBeenCalledWith('cell_sticker', {
			rackId: 2,
			shelf: 2,
			cell: 4,
			code: 'B-2-4'
		});
	});
});
