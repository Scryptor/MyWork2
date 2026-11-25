import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { rack } from '../places.test-helper.js';

const api = vi.hoisted(() => ({ createStockRack: vi.fn(), updateStockRack: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);

import Harness from './RackDialogHarness.test-helper.svelte';

const refusal = (status: number, code: 'conflict' | 'validation', message: string, fields = []) =>
	new ApiError(status, code, message, {
		code,
		message,
		fields,
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

const current = { ...rack(4, 'B', 3, 5), label: 'Дисплеи' };

beforeEach(() => vi.clearAllMocks());

describe('RackDialog', () => {
	it('warns when codes will change', async () => {
		const ui = userEvent.setup();
		render(Harness, { rack: current });
		const warning = 'Коды ячеек изменятся — перепечатайте наклейки шкафа';
		expect(screen.queryByText(warning)).toBeNull();
		const perShelf = screen.getByRole('textbox', { name: 'Ячеек на полке' });
		await ui.clear(perShelf);
		await ui.type(perShelf, '1');
		expect(await screen.findByText(warning)).toBeInTheDocument();
		await ui.clear(perShelf);
		await ui.type(perShelf, '6');
		await waitFor(() => expect(screen.queryByText(warning)).toBeNull());
	});

	it('shows the refusal of an occupied rack', async () => {
		const ui = userEvent.setup();
		api.updateStockRack.mockRejectedValue(
			refusal(409, 'conflict', 'В ячейках B-3-5, B-3-4 лежат запчасти')
		);
		render(Harness, { rack: current });
		await ui.clear(screen.getByRole('textbox', { name: 'Подпись' }));
		await ui.type(screen.getByRole('textbox', { name: 'Подпись' }), 'Х');
		await ui.click(screen.getByRole('button', { name: 'Сохранить' }));
		const alert = await screen.findByRole('alert');
		expect(alert).toHaveTextContent('В ячейках B-3-5, B-3-4 лежат запчасти');
		expect(screen.getByRole('dialog')).toBeInTheDocument();
		expect(screen.getByTestId('open')).toHaveTextContent('true');
	});

	it('keeps a refused field under its field', async () => {
		const ui = userEvent.setup();
		api.updateStockRack.mockRejectedValue(
			refusal(422, 'validation', 'Неверно', [
				{ field: 'code', code: 'duplicate', message: 'x' }
			] as never)
		);
		render(Harness, { rack: current });
		await ui.type(screen.getByRole('textbox', { name: 'Код' }), '2');
		await ui.click(screen.getByRole('button', { name: 'Сохранить' }));
		expect(await screen.findByText('Шкаф с таким кодом уже есть')).toBeInTheDocument();
		expect(screen.getByTestId('open')).toHaveTextContent('true');
	});

	it('sends only what changed', async () => {
		const ui = userEvent.setup();
		const saved = { ...current, label: 'Экраны' };
		api.updateStockRack.mockResolvedValue(saved);
		const onsaved = vi.fn();
		render(Harness, { rack: current, onsaved });
		const label = screen.getByRole('textbox', { name: 'Подпись' });
		await ui.clear(label);
		await ui.type(label, 'Экраны');
		await ui.click(screen.getByRole('button', { name: 'Сохранить' }));
		await waitFor(() => expect(api.updateStockRack).toHaveBeenCalledWith(4, { label: 'Экраны' }));
		expect(onsaved).toHaveBeenCalledWith(saved);
	});

	it('sends the whole rack when it is new', async () => {
		const ui = userEvent.setup();
		api.createStockRack.mockResolvedValue(current);
		render(Harness, {});
		await ui.type(screen.getByRole('textbox', { name: 'Код' }), 'C');
		await ui.click(screen.getByRole('button', { name: 'Добавить' }));
		await waitFor(() =>
			expect(api.createStockRack).toHaveBeenCalledWith({
				code: 'C',
				label: '',
				shelves: 1,
				cells_per_shelf: 1,
				service_point_id: null
			})
		);
	});
});
