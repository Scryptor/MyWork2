import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

// «Справочники» as a page. Without «settings_manage» (an imported user, or a role of plan 8) the screen only shows: no form, no
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
const nav = vi.hoisted(() => ({ kind: 'brand' }));
vi.mock('$app/state', () => ({
	page: {
		get url() {
			return new URL(`http://localhost/settings/dictionaries?kind=${nav.kind}`);
		}
	}
}));
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));
const rights = vi.hoisted(() => ({
	manage: false,
	users: true,
	refreshCatalogs: vi.fn(async () => {})
}));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: {
		can: (p: string) => (p === 'users_manage' ? rights.users : rights.manage),
		refreshCatalogs: rights.refreshCatalogs
	}
}));

import { toast } from '#lib/ui/toast/toast.svelte.js';
import Page from './DictionariesPageHarness.test-helper.svelte';

const item = (id: number, value: string, archived = false) => ({
	id,
	kind: nav.kind,
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
		retry_after_secs: null,
		diagnostics: []
	});

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	rights.manage = false;
	rights.users = true;
	rights.refreshCatalogs.mockClear();
	nav.kind = 'brand';
	api.listDictionary.mockResolvedValue([item(1, 'Alpha'), item(2, 'Beta'), item(3, 'Old', true)]);
	api.dictionaryUsage.mockRejectedValue(forbidden());
});

describe('«Справочники» without «settings_manage»', () => {
	it('shows the values, but no counts and nothing to change them with', async () => {
		render(Page);
		expect(await screen.findByText('Alpha')).toBeInTheDocument();
		expect(screen.getByText('Old')).toBeInTheDocument();
		await waitFor(() => expect(api.listDictionary).toHaveBeenCalled());
		expect(api.dictionaryUsage).not.toHaveBeenCalled();
		expect(screen.queryAllByText(/^в \d+ заказ/)).toHaveLength(0);
		expect(screen.queryByText(t403)).not.toBeInTheDocument();

		expect(screen.queryByRole('textbox', { name: 'Добавить значение' })).not.toBeInTheDocument();
		expect(screen.queryAllByRole('checkbox')).toHaveLength(0);
		for (const name of [
			'В архив',
			'Вернуть',
			'Удалить',
			'Объединить выбранные',
			'Привести написание в заказах'
		]) {
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

describe('«Справочники» for an administrator', () => {
	beforeEach(() => {
		rights.manage = true;
		api.dictionaryUsage.mockResolvedValue([]);
	});

	it('has no «Мастера» section: masters are users, with a link to «Пользователи»', async () => {
		render(Page);
		await screen.findByText('Alpha');
		expect(screen.queryByRole('button', { name: 'Мастера' })).not.toBeInTheDocument();
		expect(screen.getByText(/Мастера теперь — пользователи с отметкой «Мастер»/)).toBeVisible();
		expect(screen.getByRole('link', { name: 'Пользователи' })).toHaveAttribute(
			'href',
			'/settings/users'
		);
	});

	it('an old address with ?kind=master opens the first dictionary', async () => {
		nav.kind = 'master';
		render(Page);
		expect(await screen.findByText('Alpha')).toBeInTheDocument();
		expect(api.listDictionary.mock.calls[0]?.[0]).toBe('device_type');
	});

	it('the link to «Пользователи» is only for those who manage users', async () => {
		rights.users = false;
		render(Page);
		await screen.findByText('Alpha');
		expect(screen.getByText(/Мастера теперь — пользователи с отметкой «Мастер»/)).toBeVisible();
		expect(screen.queryByRole('link', { name: 'Пользователи' })).not.toBeInTheDocument();
	});

	it('a brand is not a default: nothing more is read', async () => {
		api.listDictionary.mockResolvedValue([item(1, 'Alpha')]);
		api.updateDictionaryItem.mockResolvedValue(item(1, 'Omega'));
		render(Page);
		await userEvent.click(await screen.findByRole('button', { name: 'Alpha' }));
		await userEvent.type(screen.getByRole('textbox', { name: 'Значение „Alpha“' }), 'X{Enter}');
		await waitFor(() => expect(api.updateDictionaryItem).toHaveBeenCalled());
		expect(rights.refreshCatalogs).not.toHaveBeenCalled();
	});

	it('«Привести написание в заказах» only for a value in use: renames it to itself in the orders', async () => {
		api.dictionaryUsage.mockResolvedValue([{ id: 1, count: 3 }]);
		api.renameDictionaryItem.mockResolvedValue({ item: item(1, 'Alpha'), records_changed: 2 });
		render(Page);
		const align = await screen.findAllByRole('button', { name: 'Привести написание в заказах' });
		// Alpha is used by 3 orders; Beta and the archived Old by none.
		expect(align).toHaveLength(1);
		await userEvent.click(align[0]!);
		await waitFor(() =>
			expect(api.renameDictionaryItem).toHaveBeenCalledWith(1, {
				value: 'Alpha',
				in_orders: true
			})
		);
		await waitFor(() =>
			expect(toast.items.map((x) => x.message)).toContain('Приведено: изменено 2 заказа')
		);
	});

	it('merges what the dialog shows: a value archived meanwhile stays out', async () => {
		api.listDictionary.mockResolvedValue([item(1, 'Alpha'), item(2, 'Beta'), item(3, 'Gamma')]);
		api.updateDictionaryItem.mockResolvedValue(item(2, 'Beta 2'));
		api.mergeDictionary.mockResolvedValue({ item: item(1, 'Alpha'), records_changed: 0 });
		render(Page);
		for (const name of ['Alpha', 'Beta', 'Gamma']) {
			await userEvent.click(await screen.findByRole('checkbox', { name: `Выбрать „${name}“` }));
		}
		// Somebody else archives «Gamma»; the list is read again after a rename here.
		api.listDictionary.mockResolvedValue([
			item(1, 'Alpha'),
			item(2, 'Beta 2'),
			item(3, 'Gamma', true)
		]);
		await userEvent.click(screen.getByRole('button', { name: 'Beta' }));
		await userEvent.type(screen.getByRole('textbox', { name: 'Значение „Beta“' }), ' 2{Enter}');
		expect(await screen.findByRole('button', { name: 'Beta 2' })).toBeInTheDocument();

		await userEvent.click(screen.getByRole('button', { name: 'Объединить выбранные' }));
		const dialog = await screen.findByRole('dialog', { name: 'Объединить значения' });
		expect(within(dialog).queryByText('Gamma')).not.toBeInTheDocument();
		await userEvent.click(within(dialog).getByRole('radio', { name: /Alpha/ }));
		await userEvent.click(within(dialog).getByRole('button', { name: 'Объединить' }));
		await waitFor(() =>
			expect(api.mergeDictionary).toHaveBeenCalledWith('brand', {
				target_id: 1,
				source_ids: [2],
				in_orders: true
			})
		);
	});
});
