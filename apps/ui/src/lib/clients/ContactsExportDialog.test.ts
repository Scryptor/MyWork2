import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { serverClock } from '#lib/api/server-clock.svelte.js';

const api = vi.hoisted(() => ({ downloadContacts: vi.fn() }));
vi.mock('#lib/api/exports.js', () => api);

import ContactsExportDialog from './ContactsExportDialog.svelte';

const KEY = 'mw.contactsExportedOn';
const open = () => render(ContactsExportDialog, { open: true, query: { query: 'и' } });

beforeEach(() => {
	api.downloadContacts.mockReset().mockResolvedValue(undefined);
	localStorage.clear();
});
afterEach(() => vi.restoreAllMocks());

describe('«Контакты для телефона»', () => {
	it('starts with «Все из списка» when no day is kept', () => {
		open();
		expect(screen.getByLabelText('Все из списка')).toBeChecked();
		expect(screen.queryByLabelText('Дата')).toBeNull();
	});

	it('starts with «Добавленные с» and the day of the last export', () => {
		localStorage.setItem(KEY, '2026-11-20');
		open();
		expect(screen.getByLabelText('Добавленные с')).toBeChecked();
		expect(screen.getByLabelText('Дата')).toHaveValue('20.11.2026');
	});

	it('works when the storage throws', async () => {
		vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
			throw new Error('blocked');
		});
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new Error('blocked');
		});
		open();
		expect(screen.getByLabelText('Все из списка')).toBeChecked();
		await userEvent.click(screen.getByLabelText('vCard (iPhone, Android)'));
		await userEvent.click(screen.getByRole('button', { name: 'Выгрузить' }));
		await waitFor(() =>
			expect(api.downloadContacts).toHaveBeenCalledWith({ query: 'и' }, 'vcard', null)
		);
	});

	it('sends the format and the day, then remembers today', async () => {
		serverClock.sync('2031-05-06T23:30:00');
		localStorage.setItem(KEY, '2026-11-20');
		open();
		await userEvent.click(screen.getByRole('button', { name: 'Выгрузить' }));
		await waitFor(() =>
			expect(api.downloadContacts).toHaveBeenCalledWith({ query: 'и' }, 'csv', '2026-11-20')
		);
		// The enterprise's day by the server's clock (here a day far from the browser's own).
		expect(localStorage.getItem(KEY)).toBe('2031-05-06');
	});

	it("keeps the dialog and tells the server's reason under the form", async () => {
		const reason = 'Нет клиентов с телефонами для выгрузки';
		const body = {
			code: 'validation' as const,
			message: 'Проверьте введённые данные',
			fields: [{ field: 'rows', code: 'nothing_to_export', message: reason }],
			current_version: null,
			permission: null,
			retry_after_secs: null,
			diagnostics: []
		};
		api.downloadContacts.mockRejectedValue(new ApiError(422, 'validation', body.message, body));
		open();
		await userEvent.click(screen.getByRole('button', { name: 'Выгрузить' }));
		expect(await screen.findByText(reason)).toBeInTheDocument();
		expect(localStorage.getItem(KEY)).toBeNull();
		expect(screen.getByRole('button', { name: 'Выгрузить' })).toBeEnabled();
	});

	it('Ctrl+Enter submits', async () => {
		open();
		await userEvent.keyboard('{Control>}{Enter}{/Control}');
		await waitFor(() => expect(api.downloadContacts).toHaveBeenCalledOnce());
	});
});
