import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

const api = vi.hoisted(() => ({ createStockItem: vi.fn(), stockSummary: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);
const dict = vi.hoisted(() => ({ listDictionary: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => dict);

import Harness from './StockAddDialogHarness.test-helper.svelte';

const validation = (field: string, code: string) =>
	new ApiError(422, 'validation', 'Неверно', {
		code: 'validation',
		message: 'Неверно',
		fields: [{ field, code, message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

beforeEach(() => {
	vi.clearAllMocks();
	dict.listDictionary.mockResolvedValue([]);
	api.stockSummary.mockResolvedValue({ restock: 0 });
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
});
