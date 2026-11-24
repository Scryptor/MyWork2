import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';

// The form of a new status while the first add is still finishing: the list shows the new row as soon as it is
// read again, the program's own copy of the catalogs follows a little later — and a person who already types
// the next name must keep it.
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
const app = vi.hoisted(() => ({ refreshCatalogs: vi.fn() }));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: { can: () => true, refreshCatalogs: app.refreshCatalogs }
}));

import Page from '#lib/settings/StatusesPageHarness.test-helper.svelte';

const status = (id: number, name: string) => ({
	id,
	name,
	kind: 'open',
	color: 'gray',
	sort_order: id,
	archived: false
});

const deferred = () => {
	let resolve!: () => void;
	const promise = new Promise<void>((r) => (resolve = r));
	return { promise, resolve };
};

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	app.refreshCatalogs.mockReset();
	api.statusUsage.mockResolvedValue([]);
});

describe('«Статусы»: a new status', () => {
	it('typing the next name while the first add is finishing keeps the typed text', async () => {
		const told = deferred();
		app.refreshCatalogs.mockReturnValue(told.promise);
		let rows = [status(1, 'Готов')];
		api.listStatuses.mockImplementation(async () => rows);
		api.createStatus.mockImplementation(async (v: { name: string }) => {
			const added = status(rows.length + 1, v.name);
			rows = [...rows, added];
			return added;
		});
		render(Page);
		const field = await screen.findByRole('textbox', { name: 'Название нового статуса' });

		await userEvent.type(field, 'А');
		await userEvent.keyboard('{Enter}');
		// The row is there (the list is read again); the program's copy is still being read.
		await waitFor(() => expect(screen.getAllByRole('listitem').length).toBeGreaterThan(1));
		await waitFor(() => expect(api.listStatuses.mock.calls.length).toBeGreaterThan(1));
		await userEvent.clear(field);
		await userEvent.type(field, 'Б');
		told.resolve();

		await waitFor(() =>
			expect(screen.getByRole('button', { name: 'Добавить статус' })).toBeEnabled()
		);
		expect(field).toHaveValue('Б');
		await userEvent.keyboard('{Enter}');
		await waitFor(() => expect(api.createStatus).toHaveBeenCalledTimes(2));
		expect(api.createStatus).toHaveBeenLastCalledWith({ name: 'Б', kind: 'open', color: 'gray' });
	});
});
