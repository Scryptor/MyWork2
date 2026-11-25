import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

// «В Excel» of the lists: there only with «data_export», and a refusal of the server is a toast.
const api = vi.hoisted(() => ({ downloadOrders: vi.fn(), downloadClients: vi.fn() }));
vi.mock('#lib/api/exports.js', () => api);
const rights = vi.hoisted(() => ({ export: true }));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: { can: (p: string) => p !== 'data_export' || rights.export }
}));

import ExportClientsMenu from './ExportClientsMenu.svelte';
import ExportOrdersButton from './ExportOrdersButton.svelte';

beforeEach(() => {
	api.downloadOrders.mockReset().mockResolvedValue(undefined);
	api.downloadClients.mockReset().mockResolvedValue(undefined);
	rights.export = true;
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('«В Excel» of the orders', () => {
	it('is there with the right and sends the list query', async () => {
		render(ExportOrdersButton, { query: { brand: 'Тест', sort: 'id' } });
		await userEvent.click(screen.getByRole('button', { name: 'В Excel' }));
		expect(api.downloadOrders).toHaveBeenCalledWith({ brand: 'Тест', sort: 'id' });
	});

	it('is not there without the right', () => {
		rights.export = false;
		render(ExportOrdersButton, { query: {} });
		expect(screen.queryByRole('button', { name: 'В Excel' })).toBeNull();
	});

	it("tells the server's refusal", async () => {
		api.downloadOrders.mockRejectedValue(
			new ApiError(422, 'validation', 'Слишком много строк', null)
		);
		render(ExportOrdersButton, { query: {} });
		await userEvent.click(screen.getByRole('button', { name: 'В Excel' }));
		await waitFor(() =>
			expect(toast.items.map((t) => t.message)).toEqual([
				'Не удалось выгрузить файл: Слишком много строк'
			])
		);
	});
});

describe('«Выгрузить» of the clients', () => {
	it('has «В Excel» with the right', async () => {
		render(ExportClientsMenu, { query: { query: 'и' } });
		// Opened and chosen from the keyboard; jsdom does not place the menu, so its parts are found with `hidden`.
		screen.getByRole('button', { name: 'Выгрузить' }).focus();
		await userEvent.keyboard('{Enter}');
		const item = await screen.findByRole('menuitem', { hidden: true });
		expect(item).toHaveTextContent('В Excel');
		item.focus();
		await userEvent.keyboard('{Enter}');
		expect(api.downloadClients).toHaveBeenCalledWith({ query: 'и' });
	});

	it('is not there without the right', () => {
		rights.export = false;
		render(ExportClientsMenu, { query: {} });
		expect(screen.queryByRole('button', { name: 'Выгрузить' })).toBeNull();
	});
});
