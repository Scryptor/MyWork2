import { render, screen, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';

// Without «settings_manage» (a user of plan 8) the screen only shows: no form, no boxes, no buttons,
// no handles and not a single write request.
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
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => false } }));

import Page from '../../routes/(app)/settings/dictionaries/+page.svelte';

const item = (id: number, value: string, archived = false) => ({
	id,
	kind: 'brand',
	value,
	sort_order: id,
	archived
});

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	api.listDictionary.mockResolvedValue([item(1, 'Alpha'), item(2, 'Beta'), item(3, 'Old', true)]);
	api.dictionaryUsage.mockResolvedValue([{ id: 1, count: 2 }]);
});

describe('«Справочники» without «settings_manage»', () => {
	it('shows the values and counts, but nothing to change them with', async () => {
		render(Page);
		expect(await screen.findByText('Alpha')).toBeInTheDocument();
		expect(screen.getByText('в 2 заказах')).toBeInTheDocument();
		expect(screen.getByText('Old')).toBeInTheDocument();

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
