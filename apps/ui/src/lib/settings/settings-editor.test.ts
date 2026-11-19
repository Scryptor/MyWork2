import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { SettingsPatch } from '$api/SettingsPatch';
import { ApiError } from '#lib/api/client.js';
import { createSettingsEditor, type SettingsEditor } from './settings-editor.svelte.js';

const SERVER = {
	firm_name: 'Сервис',
	currency_symbol: '₽',
	master_percent: 40,
	diagnostics_overdue_days: 3,
	ready_stale_days: 30
} as AppSettings;

const validation = (field: string, code: string) =>
	new ApiError(422, 'validation', 'Ошибка', {
		code: 'validation',
		message: 'Ошибка',
		fields: [{ field, code, message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

let save: ReturnType<typeof vi.fn<(patch: SettingsPatch) => Promise<AppSettings>>>;
let refresh: ReturnType<typeof vi.fn<() => Promise<void>>>;
let editor: SettingsEditor;

function make(can = true) {
	editor = createSettingsEditor({ can: () => can, save, refresh, delayMs: 600 });
}

beforeEach(() => {
	vi.useFakeTimers();
	save = vi.fn(async (p: SettingsPatch) => ({ ...SERVER, ...p }) as AppSettings);
	refresh = vi.fn(async () => {});
});
afterEach(() => {
	editor.dispose();
	vi.useRealTimers();
});

describe('createSettingsEditor', () => {
	it('sends nothing without the right to manage settings', async () => {
		make(false);
		editor.edit('firm_name', 'Новое');
		editor.commit('intake_required_fields', true);
		editor.editNumber('master_percent', '50');
		await vi.advanceTimersByTimeAsync(2000);
		await editor.flush();
		expect(save).not.toHaveBeenCalled();
		expect(editor.canEdit).toBe(false);
		expect(editor.value('firm_name', 'Сервис')).toBe('Сервис');
	});

	it('two quick changes go as one PATCH and the catalogs are read again after it', async () => {
		make();
		editor.edit('firm_name', 'Новое');
		await vi.advanceTimersByTimeAsync(200);
		editor.edit('currency_symbol', '$');
		await vi.advanceTimersByTimeAsync(700);
		expect(save).toHaveBeenCalledTimes(1);
		expect(save).toHaveBeenCalledWith({ firm_name: 'Новое', currency_symbol: '$' });
		expect(refresh).toHaveBeenCalledTimes(1);
	});

	it('shows the server error under its field and keeps the typed value', async () => {
		make();
		save.mockRejectedValueOnce(validation('diagnostics_overdue_days', 'out_of_range'));
		editor.editNumber('diagnostics_overdue_days', '0');
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldError('diagnostics_overdue_days')).toBe('От 1 до 365');
		expect(editor.draft('diagnostics_overdue_days', 3)).toBe('0');
		editor.editNumber('diagnostics_overdue_days', '5');
		expect(editor.fieldError('diagnostics_overdue_days')).toBeUndefined();
	});

	it('an empty currency is «Обязательное поле»', async () => {
		make();
		save.mockRejectedValueOnce(validation('currency_symbol', 'required'));
		editor.edit('currency_symbol', '');
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldError('currency_symbol')).toBe('Обязательное поле');
	});

	it('what is not a number is not sent', async () => {
		make();
		editor.editNumber('master_percent', '');
		expect(editor.fieldError('master_percent')).toBe('Обязательное поле');
		editor.editNumber('master_percent', '4x');
		expect(editor.fieldError('master_percent')).toBe('Введите целое число');
		await vi.advanceTimersByTimeAsync(2000);
		expect(save).not.toHaveBeenCalled();
		editor.leave('master_percent');
		expect(editor.fieldError('master_percent')).toBeUndefined();
		expect(editor.draft('master_percent', 40)).toBe('40');
	});

	it('a wrong field does not hold back the right one saved with it', async () => {
		make();
		save
			.mockRejectedValueOnce(validation('ready_stale_days', 'out_of_range'))
			.mockImplementation(async (p) => ({ ...SERVER, ...p }) as AppSettings);
		editor.editNumber('ready_stale_days', '0');
		editor.edit('firm_name', 'Новое');
		await vi.advanceTimersByTimeAsync(700);
		expect(save).toHaveBeenNthCalledWith(2, { firm_name: 'Новое' });
		expect(editor.fieldError('ready_stale_days')).toBe('От 1 до 365');
		expect(editor.fieldError('firm_name')).toBeUndefined();
		expect(editor.pendingFields()).toEqual(['ready_stale_days']);
	});
});
