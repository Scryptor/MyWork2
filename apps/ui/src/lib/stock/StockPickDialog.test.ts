import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { StockItem } from '$api/StockItem';
import { ApiError } from '#lib/api/client.js';
import type { OrderPartsState } from '#lib/orders/parts.svelte.js';

const api = vi.hoisted(() => ({ listStock: vi.fn(), getStockItem: vi.fn() }));
vi.mock('#lib/api/stock.js', () => api);

import Harness from './StockPickDialogHarness.test-helper.svelte';

const item = (over: Partial<StockItem> = {}): StockItem => ({
	id: 5,
	name: 'Дисплей',
	display_name: 'Дисплей',
	device_type: '',
	part_type: '',
	brand: '',
	model: '',
	color: '',
	note: '',
	quantity: 3,
	price: 100_000,
	restock_threshold: 0,
	needs_restock: false,
	created_at: '2026-11-24T10:00:00',
	updated_at: '2026-11-24T10:00:00',
	version: 1,
	...over
});

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

const partsWith = (add: OrderPartsState['add']): OrderPartsState => ({
	parts: [],
	total: 0,
	loading: false,
	error: null,
	adding: false,
	revision: 0,
	isReturning: () => false,
	load: async () => {},
	add,
	giveBack: async () => {},
	detach: () => {}
});

beforeEach(() => {
	vi.clearAllMocks();
});

describe('StockPickDialog', () => {
	it('does not say «Ничего не нашлось» while the search is still on its way', async () => {
		let answer!: (v: unknown) => void;
		api.listStock.mockReturnValue(new Promise((r) => (answer = r)));
		render(Harness, { parts: partsWith(async () => {}) });
		expect(screen.queryByText(/Ничего не нашлось/)).toBeNull();
		await waitFor(() => expect(api.listStock).toHaveBeenCalled());
		expect(screen.queryByText(/Ничего не нашлось/)).toBeNull();
		answer({ items: [], total: 0 });
		expect(await screen.findByText(/Ничего не нашлось/)).toBeInTheDocument();
	});

	it('a position deleted meanwhile is told and the list is read again without it', async () => {
		const ui = userEvent.setup();
		api.listStock.mockResolvedValueOnce({ items: [item()], total: 1 });
		api.listStock.mockResolvedValue({ items: [], total: 0 });
		const add = vi.fn().mockRejectedValue(validation('stock_item_id', 'deleted'));
		render(Harness, { parts: partsWith(add) });
		await ui.click(await screen.findByRole('button', { name: /Дисплей/ }));
		await ui.click(screen.getByRole('button', { name: 'Списать' }));
		expect(await screen.findByText('Позицию удалили')).toBeInTheDocument();
		await waitFor(() => expect(api.listStock).toHaveBeenCalledTimes(2));
		expect(screen.queryByRole('button', { name: /Дисплей/ })).toBeNull();
		expect(screen.getByTestId('open')).toHaveTextContent('true');
	});
});
