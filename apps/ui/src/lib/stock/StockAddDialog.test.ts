import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

const api = vi.hoisted(() => ({
	createStockItem: vi.fn(),
	stockSummary: vi.fn(),
	listStockRacks: vi.fn()
}));
vi.mock('#lib/api/stock.js', () => api);
const dict = vi.hoisted(() => ({ listDictionary: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => dict);

import { rack } from './places.test-helper.js';
import Harness from './StockAddDialogHarness.test-helper.svelte';

const validation = (field: string, code: string, message = 'x') =>
	new ApiError(422, 'validation', 'Неверно', {
		code: 'validation',
		message: 'Неверно',
		fields: [{ field, code, message }],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
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
	dict.listDictionary.mockResolvedValue([]);
	api.stockSummary.mockResolvedValue({ restock: 0 });
	api.listStockRacks.mockResolvedValue([]);
});

describe('StockAddDialog', () => {
	it('a refusal of the server stays at its field and what was typed is kept', async () => {
		const ui = userEvent.setup();
		api.createStockItem.mockRejectedValue(validation('price', 'out_of_range'));
		render(Harness);
		await ui.type(screen.getByRole('textbox', { name: 'Модель' }), 'M-1');
		await ui.click(screen.getByRole('button', { name: /^Добавить$/ }));
		expect(await screen.findByText('Цена — от 0 до 10 000 000 ₽')).toBeInTheDocument();
		expect(screen.getByRole('textbox', { name: 'Модель' })).toHaveValue('M-1');
		expect(screen.getByRole('dialog', { name: 'Новая позиция' })).toBeInTheDocument();
	});

	// A base imported before the stock module: its first position closes the transfer of the stock of MyWork 2
	// (decision 17), so the dialog says it before that position is made.
	it('warns while the stock of MyWork 2 waits to be transferred', async () => {
		const { unmount } = render(Harness, { legacyPending: true });
		expect(
			screen.getByText(
				'Склад MyWork 2 ещё не перенесён: после этой позиции перенести его будет нельзя.'
			)
		).toBeInTheDocument();
		unmount();
		render(Harness);
		expect(screen.getByRole('dialog', { name: 'Новая позиция' })).toBeInTheDocument();
		expect(screen.queryByText(/Склад MyWork 2 ещё не перенесён/)).not.toBeInTheDocument();
	});

	it('a double submit makes one position', async () => {
		const ui = userEvent.setup();
		let answer!: (v: unknown) => void;
		api.createStockItem.mockReturnValue(new Promise((r) => (answer = r)));
		render(Harness);
		await ui.type(screen.getByRole('textbox', { name: 'Модель' }), 'M-1');
		const add = screen.getByRole('button', { name: /^Добавить$/ });
		await ui.click(add);
		await ui.click(add);
		answer({ id: 5 });
		await waitFor(() => expect(screen.getByTestId('created')).toHaveTextContent('5'));
		expect(api.createStockItem).toHaveBeenCalledTimes(1);
	});

	describe('the cell', () => {
		it('sends the chosen cell', async () => {
			const ui = userEvent.setup();
			api.listStockRacks.mockResolvedValue([rack(1, 'A', 1, 2), rack(2, 'B', 2, 4)]);
			api.createStockItem.mockResolvedValue({ id: 5 });
			render(Harness);
			await ui.type(screen.getByRole('textbox', { name: 'Модель' }), 'M-1');
			await ui.type(await screen.findByRole('combobox', { name: 'Ячейка' }), 'B-2-4');
			await ui.click(await option());
			await ui.click(screen.getByRole('button', { name: /^Добавить$/ }));
			await waitFor(() => expect(api.createStockItem).toHaveBeenCalled());
			expect(api.createStockItem.mock.calls[0]![0].place).toEqual({
				rack_id: 2,
				shelf: 2,
				cell: 4
			});
		});

		it('has no field and no cell without racks', async () => {
			const ui = userEvent.setup();
			api.createStockItem.mockResolvedValue({ id: 5 });
			render(Harness);
			await ui.type(screen.getByRole('textbox', { name: 'Модель' }), 'M-1');
			await waitFor(() => expect(api.listStockRacks).toHaveBeenCalled());
			expect(screen.queryByRole('combobox', { name: 'Ячейка' })).not.toBeInTheDocument();
			await ui.click(screen.getByRole('button', { name: /^Добавить$/ }));
			await waitFor(() => expect(api.createStockItem).toHaveBeenCalled());
			expect(api.createStockItem.mock.calls[0]![0].place).toBeNull();
		});

		it('tells no_such_place under the field', async () => {
			const ui = userEvent.setup();
			api.listStockRacks.mockResolvedValue([rack(1, 'A', 1, 2)]);
			api.createStockItem.mockRejectedValue(
				validation('place', 'no_such_place', 'Такого места на складе нет')
			);
			render(Harness);
			await ui.type(screen.getByRole('textbox', { name: 'Модель' }), 'M-1');
			await ui.type(await screen.findByRole('combobox', { name: 'Ячейка' }), 'A-1');
			await ui.click(await option());
			await ui.click(screen.getByRole('button', { name: /^Добавить$/ }));
			expect(await screen.findByText('Такого места на складе нет')).toBeInTheDocument();
		});
	});
});
