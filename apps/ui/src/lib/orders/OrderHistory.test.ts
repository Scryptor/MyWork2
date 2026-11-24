import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, expect, it, vi } from 'vitest';
import type { AuditEntry } from '$api/AuditEntry';

const api = vi.hoisted(() => ({ orderHistory: vi.fn() }));
vi.mock('#lib/api/orders.js', () => api);

import Harness from './OrderHistoryHarness.test-helper.svelte';

const entry = (id: number, summary: string): AuditEntry => ({
	id,
	at: '2026-11-24T10:00:00',
	user_id: 1,
	user_name: 'Админ',
	action: 'order.part_added',
	order_id: 7,
	client_id: 3,
	stock_item_id: 5,
	summary,
	details: null
});

beforeEach(() => vi.clearAllMocks());

it('asks the journal again when a part action asks for it, though the version is the same', async () => {
	api.orderHistory.mockResolvedValueOnce({ items: [], total: 0 });
	render(Harness);
	await waitFor(() => expect(api.orderHistory).toHaveBeenCalledTimes(1));

	api.orderHistory.mockResolvedValueOnce({
		items: [entry(9, 'Списано со склада: Дисплей × 2')],
		total: 1
	});
	await userEvent.setup().click(screen.getByRole('button', { name: 'part action' }));
	expect(await screen.findByText('Списано со склада: Дисплей × 2')).toBeInTheDocument();
	expect(api.orderHistory).toHaveBeenCalledTimes(2);
});
