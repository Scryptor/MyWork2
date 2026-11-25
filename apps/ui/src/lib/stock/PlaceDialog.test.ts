import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { stockItem } from './places.test-helper.js';

const api = vi.hoisted(() => ({ listStock: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);

import Harness from './PlaceDialogHarness.test-helper.svelte';

beforeEach(() => vi.clearAllMocks());

describe('PlaceDialog of a cell', () => {
	it('finds a position with something unplaced and lays it into the given cell', async () => {
		const ui = userEvent.setup();
		api.listStock.mockResolvedValue({
			items: [stockItem({ id: 9, unplaced: 4 }), stockItem({ id: 10, unplaced: 0 })],
			total: 2,
			place: null
		});
		const onsubmit = vi.fn().mockResolvedValue(undefined);
		render(Harness, { onsubmit, reload: vi.fn() });
		expect(screen.getByRole('dialog', { name: 'Положить в ячейку B-2-4' })).toBeInTheDocument();
		await ui.type(screen.getByRole('combobox', { name: 'Позиция' }), 'диспл');
		await waitFor(() =>
			expect(api.listStock).toHaveBeenCalledWith({
				query: 'диспл',
				as_text: true,
				limit: 50,
				sort: 'name'
			})
		);
		await ui.click(
			await waitFor(() => {
				const el = document.querySelector('[data-combobox-item]');
				if (!el) throw new Error('no option');
				return el;
			})
		);
		// The field holds the choice; the list may or may not be open again by now (it follows the text).
		await waitFor(() =>
			expect(screen.getByRole('combobox', { name: 'Позиция' })).toHaveValue(
				'Дисплей iPhone · Арт. 9'
			)
		);
		await ui.click(screen.getByRole('button', { name: 'Положить' }));
		await waitFor(() =>
			expect(onsubmit).toHaveBeenCalledWith(9, { rack_id: 2, shelf: 2, cell: 4 }, 4)
		);
	});

	it('does not lay a position whose name was edited after it was chosen', async () => {
		const ui = userEvent.setup();
		api.listStock.mockResolvedValue({
			items: [stockItem({ id: 9, unplaced: 4 })],
			total: 1,
			place: null
		});
		const onsubmit = vi.fn().mockResolvedValue(undefined);
		render(Harness, { onsubmit, reload: vi.fn() });
		const field = screen.getByRole('combobox', { name: 'Позиция' });
		await ui.type(field, 'диспл');
		await ui.click(
			await waitFor(() => {
				const el = document.querySelector('[data-combobox-item]');
				if (!el) throw new Error('no option');
				return el;
			})
		);
		await ui.type(field, 'x');
		await ui.click(screen.getByRole('button', { name: 'Положить' }));
		expect(await screen.findByText('Выберите позицию из списка')).toBeInTheDocument();
		expect(onsubmit).not.toHaveBeenCalled();
	});
});
