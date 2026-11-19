import { render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

// Without «settings_manage» (an imported user, or a role of plan 8) the screen only shows: no form, no
// boxes, no buttons, no handles and not a single write request. The counts need the right too: the server
// refuses them, and the screen then does not know them — it does not say «в 0 заказах».
const api = vi.hoisted(() => ({
	listDictionary: vi.fn(),
	dictionaryUsage: vi.fn(),
	addDictionaryItem: vi.fn(),
	updateDictionaryItem: vi.fn(),
	deleteDictionaryItem: vi.fn(),
	reorderDictionary: vi.fn(),
	renameDictionaryItem: vi.fn(),
	mergeDictionary: vi.fn()
}));
vi.mock('#lib/api/catalogs.js', () => api);
vi.mock('$app/state', () => ({
	page: { url: new URL('http://localhost/settings/dictionaries?kind=brand') }
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));
const rights = vi.hoisted(() => ({ manage: false }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => rights.manage } }));

import Page from './DictionariesPageHarness.test-helper.svelte';

const item = (id: number, value: string, archived = false) => ({
	id,
	kind: 'brand',
	value,
	sort_order: id,
	archived
});

const t403 = 'Недостаточно прав';
const forbidden = () =>
	new ApiError(403, 'forbidden', t403, {
		code: 'forbidden',
		message: t403,
		fields: [],
		current_version: null,
		permission: 'settings_manage',
		retry_after_secs: null
	});

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	rights.manage = false;
	api.listDictionary.mockResolvedValue([item(1, 'Alpha'), item(2, 'Beta'), item(3, 'Old', true)]);
	api.dictionaryUsage.mockRejectedValue(forbidden());
});

describe('«Справочники» without «settings_manage»', () => {
	it('shows the values, but no counts and nothing to change them with', async () => {
		render(Page);
		expect(await screen.findByText('Alpha')).toBeInTheDocument();
		expect(screen.getByText('Old')).toBeInTheDocument();
		await waitFor(() => expect(api.dictionaryUsage).toHaveBeenCalled());
		expect(screen.queryAllByText(/^в \d+ заказ/)).toHaveLength(0);
		expect(screen.queryByText(t403)).not.toBeInTheDocument();

		expect(screen.queryByRole('textbox', { name: 'Добавить значение' })).not.toBeInTheDocument();
		expect(screen.queryAllByRole('checkbox')).toHaveLength(0);
		for (const name of ['В архив', 'Вернуть', 'Удалить', 'Объединить выбранные']) {
			expect(screen.queryByRole('button', { name })).not.toBeInTheDocument();
		}
		// The handles of the SortableList, and the value that would be edited in place.
		expect(screen.queryByRole('button', { name: /^Переместить/ })).not.toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Alpha' })).not.toBeInTheDocument();
		expect(screen.getByText('Изменять настройки может администратор')).toBeInTheDocument();
		// The search still works: it only reads.
		expect(screen.getByRole('textbox', { name: 'Найти' })).toBeEnabled();

		await waitFor(() => expect(api.listDictionary).toHaveBeenCalled());
		for (const name of [
			'addDictionaryItem',
			'updateDictionaryItem',
			'deleteDictionaryItem',
			'reorderDictionary',
			'renameDictionaryItem',
			'mergeDictionary'
		] as const) {
			expect(api[name]).not.toHaveBeenCalled();
		}
	});
});

describe('«Справочники» when the counts cannot be read', () => {
	it('an administrator sees no counts and no «Удалить»: a value may still be in orders', async () => {
		rights.manage = true;
		api.dictionaryUsage.mockRejectedValue(new TypeError('Failed to fetch'));
		render(Page);
		expect(await screen.findByRole('button', { name: 'Alpha' })).toBeInTheDocument();
		await waitFor(() => expect(api.dictionaryUsage).toHaveBeenCalled());
		expect(screen.queryAllByText(/^в \d+ заказ/)).toHaveLength(0);
		expect(screen.getAllByRole('button', { name: 'В архив' })).toHaveLength(2);
		expect(screen.queryByRole('button', { name: 'Удалить' })).not.toBeInTheDocument();
	});
});
