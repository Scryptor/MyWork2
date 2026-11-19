import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import SaveNote from './SaveNote.svelte';
import type { SettingsEditor } from './settings-editor.svelte.js';

const fake = (p: Partial<SettingsEditor>) =>
	({ state: 'idle', error: null, fieldsFailed: false, retry: () => {}, ...p }) as SettingsEditor;

describe('SaveNote', () => {
	it('says to check the fields when only fields have a mistake', () => {
		render(SaveNote, { editor: fake({ state: 'error', fieldsFailed: true }) });
		expect(screen.getByText('Не сохранено: проверьте поля')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Повторить' })).not.toBeInTheDocument();
	});

	it('a failure of the whole save: its text and «Повторить»', () => {
		render(SaveNote, {
			editor: fake({
				state: 'error',
				error: new ApiError(500, 'internal', 'Сбой сервера', null)
			})
		});
		expect(screen.getByText('Не сохранено: Сбой сервера')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Повторить' })).toBeInTheDocument();
	});

	it('nothing about mistakes while all is well', () => {
		render(SaveNote, { editor: fake({ state: 'saved' }) });
		expect(screen.getByText('Сохранено')).toBeInTheDocument();
		expect(screen.queryByText(/Не сохранено/)).not.toBeInTheDocument();
	});
});
