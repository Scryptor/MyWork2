import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

// «Статусы» as a page: a change of one status does not draw the selects of the others again (the focus in
// them stays), and without «settings_manage» the counts are not even asked for.
const api = vi.hoisted(() => ({
	listStatuses: vi.fn(),
	statusUsage: vi.fn(),
	createStatus: vi.fn(),
	updateStatus: vi.fn(),
	reorderStatuses: vi.fn()
}));
vi.mock('#lib/api/catalogs.js', async (importOriginal) => ({
	...(await importOriginal<object>()),
	...api
}));
const rights = vi.hoisted(() => ({ manage: true, refreshCatalogs: vi.fn(async () => {}) }));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: { can: () => rights.manage, refreshCatalogs: rights.refreshCatalogs }
}));

import Page from './StatusesPageHarness.test-helper.svelte';

const status = (id: number, name: string, archived = false) => ({
	id,
	name,
	kind: 'open',
	color: 'gray',
	sort_order: id,
	archived
});

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	rights.manage = true;
	api.listStatuses.mockResolvedValue([status(1, 'Первый'), status(2, 'Второй')]);
	api.statusUsage.mockResolvedValue([]);
	api.updateStatus.mockResolvedValue(status(1, 'Первый', true));
});

describe('«Статусы»', () => {
	it('a change of one status keeps the select of the other as it is', async () => {
		render(Page);
		const second = await screen.findByRole('combobox', { name: 'Вид статуса „Второй“' });
		const [archiveFirst] = screen.getAllByRole('button', { name: 'В архив' });
		await userEvent.click(archiveFirst!);
		await waitFor(() => expect(api.updateStatus).toHaveBeenCalledWith(1, { archived: true }));
		await waitFor(() => expect(api.listStatuses.mock.calls.length).toBeGreaterThan(1));
		expect(second.isConnected).toBe(true);
		expect(screen.getByRole('combobox', { name: 'Вид статуса „Второй“' })).toBe(second);
	});

	it('without «settings_manage» no counts are asked for', async () => {
		rights.manage = false;
		render(Page);
		await screen.findByText('Первый');
		expect(api.statusUsage).not.toHaveBeenCalled();
	});
});
