import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import type { OrderPart } from '$api/OrderPart';
import { ApiError } from '#lib/api/client.js';

const toast = vi.hoisted(() => ({ show: vi.fn() }));
vi.mock('#lib/ui/toast/toast.svelte.js', () => ({ toast }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => true } }));
vi.mock('#lib/api/stock.js', () => ({ listStock: vi.fn(), getStockItem: vi.fn() }));

import OrderParts from './OrderParts.svelte';
import type { OrderPartsState } from './parts.svelte.js';

const part: OrderPart = {
	id: 3,
	stock_item_id: 5,
	item_exists: true,
	name: 'Дисплей',
	price: 100_000,
	quantity: 2,
	sum: 200_000,
	added_by_name: 'Админ',
	added_at: '2026-11-24T10:00:00',
	legacy: false
};

beforeEach(() => vi.clearAllMocks());

it('a return that fails (not 404) is told in a toast and the row stays', async () => {
	const ui = userEvent.setup();
	const giveBack = vi.fn().mockRejectedValue(new ApiError(0, 'network', 'Нет связи', null));
	const parts: OrderPartsState = {
		parts: [part],
		total: 200_000,
		loading: false,
		error: null,
		adding: false,
		revision: 0,
		isReturning: () => false,
		load: async () => {},
		add: async () => {},
		giveBack,
		detach: () => {}
	};
	render(OrderParts, { parts });
	await ui.click(screen.getByRole('button', { name: /Вернуть на склад/ }));
	await ui.click(await screen.findByRole('button', { name: /^Вернуть$/ }));
	expect(giveBack).toHaveBeenCalledWith(3);
	await vi.waitFor(() =>
		expect(toast.show).toHaveBeenCalledWith(
			expect.objectContaining({ message: 'Не удалось вернуть: Нет связи', tone: 'danger' })
		)
	);
	expect(screen.getByText(/Дисплей/)).toBeInTheDocument();
});
