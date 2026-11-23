import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { SettingsPatch } from '$api/SettingsPatch';
import { ApiError } from '#lib/api/client.js';
import { discardQueues, hasUnsaved, settleQueues } from '#lib/app/session-reset.js';
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
		retry_after_secs: null,
		diagnostics: []
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

	it('a link that is not a link, and a link that is too long, are said in words of the field', async () => {
		make();
		save.mockRejectedValueOnce(validation('firm_telegram', 'not_a_link'));
		editor.edit('firm_telegram', 't.me/example');
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldError('firm_telegram')).toBe('Нужна ссылка целиком, с https://');
		save.mockRejectedValueOnce(validation('firm_google_maps', 'too_long'));
		editor.edit('firm_google_maps', 'https://' + 'a'.repeat(500));
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldError('firm_google_maps')).toBe('Не длиннее 500 знаков');
		save.mockRejectedValueOnce(validation('firm_yandex_maps', 'too_long'));
		editor.edit('firm_yandex_maps', 'https://' + 'a'.repeat(500));
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldError('firm_yandex_maps')).toBe('Не длиннее 500 знаков');
	});

	it('a typed time zone is sent as it is: the server decides if it exists', async () => {
		make();
		save.mockRejectedValueOnce(validation('timezone', 'invalid'));
		editor.edit('timezone', 'Mars/Olympus');
		await vi.advanceTimersByTimeAsync(700);
		expect(save).toHaveBeenCalledWith({ timezone: 'Mars/Olympus' });
		expect(editor.fieldError('timezone')).toBe('Неизвестный часовой пояс');
	});
});

describe('createSettingsEditor: a typed time zone and the state of the screen', () => {
	it('a time zone typed by hand waits for Enter or leaving the field, not for a pause', async () => {
		make();
		editor.type('timezone', 'Europe/Mos');
		await vi.advanceTimersByTimeAsync(5000);
		expect(save).not.toHaveBeenCalled();
		expect(editor.value('timezone', 'Europe/Moscow')).toBe('Europe/Mos');
		editor.type('timezone', 'Europe/Minsk');
		editor.commitTyped('timezone');
		await vi.advanceTimersByTimeAsync(1);
		await editor.flush();
		expect(save).toHaveBeenCalledTimes(1);
		expect(save).toHaveBeenCalledWith({ timezone: 'Europe/Minsk' });
	});

	it('leaving a typed field that was not changed sends nothing', async () => {
		make();
		editor.commitTyped('timezone');
		await vi.advanceTimersByTimeAsync(2000);
		expect(save).not.toHaveBeenCalled();
	});

	it('errors only of fields: the state is «error» with no failure of the whole', async () => {
		make();
		save.mockRejectedValueOnce(validation('timezone', 'invalid'));
		editor.commit('timezone', 'Mars/Olympus');
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.error).toBeNull();
		expect(editor.state).toBe('error');
		expect(editor.fieldsFailed).toBe(true);
	});

	it('no failure of fields once the wrong value is corrected', async () => {
		make();
		save.mockRejectedValueOnce(validation('timezone', 'invalid'));
		editor.commit('timezone', 'Mars/Olympus');
		await vi.advanceTimersByTimeAsync(700);
		editor.commit('timezone', 'Europe/Moscow');
		await vi.advanceTimersByTimeAsync(700);
		expect(editor.fieldsFailed).toBe(false);
	});
});

describe('signing out', () => {
	it('a settings edit in its pause goes before the token does', async () => {
		make();
		editor.edit('firm_name', 'Новое');
		expect(hasUnsaved()).toBe(true);
		await expect(settleQueues(1000)).resolves.toBe(true);
		expect(save).toHaveBeenCalledWith({ firm_name: 'Новое' });
		expect(hasUnsaved()).toBe(false);
	});

	it('a typed time zone not yet left goes too', async () => {
		make();
		editor.type('timezone', 'Europe/Moscow');
		expect(hasUnsaved()).toBe(true);
		await expect(settleQueues(1000)).resolves.toBe(true);
		expect(save).toHaveBeenCalledWith({ timezone: 'Europe/Moscow' });
	});

	it('a save that fails is asked about; «Выйти без сохранения» gives the edit up', async () => {
		make();
		save.mockRejectedValue(new ApiError(0, 'network', 'Нет связи', null));
		editor.edit('firm_name', 'Новое');
		await expect(settleQueues(1000)).resolves.toBe(false);
		discardQueues();
		expect(hasUnsaved()).toBe(false);
		expect(editor.value('firm_name', 'Сервис')).toBe('Сервис');
	});

	it('a disposed editor is no longer asked', async () => {
		make();
		editor.edit('firm_name', 'Новое');
		editor.dispose();
		expect(hasUnsaved()).toBe(false);
		make();
	});
});
