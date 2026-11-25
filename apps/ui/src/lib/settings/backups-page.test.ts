import { mockIPC } from '@tauri-apps/api/mocks';
import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { fakeDesktop, leaveDesktop } from '#lib/platform/fake-desktop.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

// «Резервные копии» as a page: the copies, the progress of a copy, the restore at the next start, the offer
// of the settings of MyWork 2, the reasons that replace the screen, a refusal of the server under its field.
const api = vi.hoisted(() => ({
	getBackups: vi.fn(),
	updateBackupSettings: vi.fn(),
	startBackup: vi.fn(),
	deleteBackup: vi.fn(),
	requestRestore: vi.fn(),
	cancelRestore: vi.fn(),
	chooseLegacyBackup: vi.fn()
}));
vi.mock('#lib/api/backups.js', () => api);
const rights = vi.hoisted(() => ({ granted: ['settings_manage', 'users_manage'] as string[] }));
vi.mock('#lib/app/context.svelte.js', () => ({
	app: { can: (p: string) => rights.granted.includes(p) }
}));

import Page from './BackupsPageHarness.test-helper.svelte';

const copy = (name: string, kind: string, at: string) => ({
	name,
	kind,
	created_at: at,
	app_version: '0.1.0',
	database_bytes: 2048,
	files: 3,
	files_bytes: 1024
});
const settings = { auto: true, keep: 7, folder: null, legacy_reviewed: true };
const overview = (over: Record<string, unknown> = {}) => ({
	supported: true,
	reason: null,
	settings,
	folder: '/data/backups',
	folder_problem: null,
	copies: [],
	job: null,
	pending: null,
	last_restore: null,
	legacy: null,
	...over
});
const toastTexts = () => toast.items.map((i) => i.message);
const validation = (field: string, code: string, message: string) =>
	new ApiError(422, 'validation', 'Проверьте введённые данные', {
		code: 'validation',
		message: 'Проверьте введённые данные',
		fields: [{ field, code, message }],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

beforeEach(() => {
	vi.clearAllMocks();
	rights.granted = ['settings_manage', 'users_manage'];
	api.getBackups.mockResolvedValue(overview());
});
afterEach(() => {
	vi.useRealTimers();
	leaveDesktop();
	toast.clear();
});

describe('the copies', () => {
	it('shows the copies newest first with their kinds', async () => {
		api.getBackups.mockResolvedValue(
			overview({
				copies: [
					copy('c3', 'manual', '2026-11-25T10:00:00'),
					copy('c2', 'auto', '2026-11-24T03:00:00'),
					copy('c1', 'before_restore', '2026-11-23T09:00:00')
				]
			})
		);
		render(Page);
		const list = await screen.findByRole('list', { name: 'Копии' });
		const rows = within(list).getAllByRole('listitem');
		expect(rows).toHaveLength(3);
		expect(rows[0]).toHaveTextContent('25 ноября 2026, 10:00');
		expect(rows[0]).toHaveTextContent('Вручную');
		expect(rows[0]).toHaveTextContent('3 КБ · 3 файла');
		expect(rows[1]).toHaveTextContent('Авто');
		expect(rows[2]).toHaveTextContent('Перед восстановлением');
	});

	it('says there are none', async () => {
		render(Page);
		expect(await screen.findByText('Копий пока нет')).toBeInTheDocument();
	});

	it('polls while a copy is being made', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		const running = { kind: 'manual', started_at: '2026-11-25T10:00:00', finished_at: null };
		api.startBackup.mockResolvedValue({ ...running, copy: null, error: null, note: null });
		api.getBackups
			.mockResolvedValueOnce(overview())
			.mockResolvedValueOnce(overview({ job: { ...running, copy: null, error: null, note: null } }))
			.mockResolvedValue(
				overview({
					copies: [copy('c1', 'manual', '2026-11-25T10:00:00')],
					job: {
						...running,
						finished_at: '2026-11-25T10:00:02',
						copy: 'c1',
						error: null,
						note: null
					}
				})
			);
		render(Page);
		await user.click(await screen.findByRole('button', { name: 'Сделать копию сейчас' }));
		expect(api.startBackup).toHaveBeenCalledOnce();
		const button = screen.getByText('Сделать копию сейчас').closest('button')!;
		await waitFor(() => expect(button).toHaveAttribute('aria-busy', 'true'));
		await vi.advanceTimersByTimeAsync(1000);
		await waitFor(() => expect(button).not.toHaveAttribute('aria-busy'));
		await waitFor(() => expect(toastTexts()).toContain('Копия сохранена'));
		expect(screen.getAllByRole('listitem')[0]).toHaveTextContent('Вручную');
		const calls = api.getBackups.mock.calls.length;
		await vi.advanceTimersByTimeAsync(5000);
		expect(api.getBackups.mock.calls.length).toBe(calls);
	});

	it('stops polling when the page is left', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		api.getBackups.mockResolvedValue(
			overview({
				job: {
					kind: 'auto',
					started_at: '2026-11-25T10:00:00',
					finished_at: null,
					copy: null,
					error: null,
					note: null
				}
			})
		);
		const { unmount } = render(Page);
		await screen.findByText('Сделать копию сейчас');
		await vi.advanceTimersByTimeAsync(1000);
		unmount();
		const calls = api.getBackups.mock.calls.length;
		await vi.advanceTimersByTimeAsync(5000);
		expect(api.getBackups.mock.calls.length).toBe(calls);
	});

	it('shows the error and the note of a copy', async () => {
		api.getBackups.mockResolvedValue(
			overview({
				job: {
					kind: 'manual',
					started_at: '2026-11-25T10:00:00',
					finished_at: '2026-11-25T10:00:01',
					copy: null,
					error: 'Нет места на диске',
					note: 'Копия ушла в папку по умолчанию'
				}
			})
		);
		render(Page);
		expect(await screen.findByText('Нет места на диске')).toBeInTheDocument();
		expect(screen.getByText('Копия ушла в папку по умолчанию')).toBeInTheDocument();
	});

	it('asks before deleting a copy', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValue(
			overview({ copies: [copy('c1', 'manual', '2026-11-25T10:00:00')] })
		);
		api.deleteBackup.mockResolvedValue(undefined);
		render(Page);
		await user.click(await screen.findByRole('button', { name: /^Удалить копию от/ }));
		const dialog = await screen.findByRole('alertdialog');
		expect(dialog).toHaveTextContent('Копия от 25 ноября 2026, 10:00 удалится без возврата.');
		await user.click(within(dialog).getByRole('button', { name: 'Удалить' }));
		await waitFor(() => expect(api.deleteBackup).toHaveBeenCalledWith('c1'));
	});

	it('opens the dialog of the same copy again after «Отмена»', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValue(
			overview({ copies: [copy('c1', 'manual', '2026-11-25T10:00:00')] })
		);
		render(Page);
		for (const button of [/^Удалить копию от/, /^Восстановить копию от/]) {
			for (let i = 0; i < 2; i++) {
				await user.click(await screen.findByRole('button', { name: button }));
				const dialog = await screen.findByRole('alertdialog');
				await user.click(within(dialog).getByRole('button', { name: 'Отмена' }));
				await waitFor(() => expect(screen.queryByRole('alertdialog')).toBeNull());
			}
		}
	});

	it('leaves «Восстановить…» and «Удалить» to «Настройки» with «Пользователи»', async () => {
		rights.granted = ['settings_manage'];
		api.getBackups.mockResolvedValue(
			overview({ copies: [copy('c1', 'manual', '2026-11-25T10:00:00')] })
		);
		render(Page);
		await screen.findByText('Вручную');
		expect(screen.queryByRole('button', { name: /^Восстановить копию от/ })).toBeNull();
		expect(screen.queryByRole('button', { name: /^Удалить копию от/ })).toBeNull();
	});
});

describe('the restore', () => {
	const pending = {
		copy: copy('c1', 'manual', '2026-11-25T10:00:00'),
		requested_at: '2026-11-25T11:00:00',
		requested_by: 'Админ'
	};

	it('asks before restoring and offers a restart in the program', async () => {
		const user = userEvent.setup();
		const desktop = fakeDesktop();
		const calls: string[] = [];
		mockIPC((cmd) => {
			calls.push(cmd);
			return null;
		});
		void desktop;
		api.getBackups.mockResolvedValue(
			overview({ copies: [copy('c1', 'manual', '2026-11-25T10:00:00')] })
		);
		api.getBackups
			.mockResolvedValueOnce(overview({ copies: [copy('c1', 'manual', '2026-11-25T10:00:00')] }))
			.mockResolvedValue(overview({ pending }));
		api.requestRestore.mockResolvedValue(pending);
		render(Page);
		await user.click(await screen.findByRole('button', { name: /^Восстановить копию от/ }));
		const dialog = await screen.findByRole('alertdialog');
		expect(dialog).toHaveTextContent('Восстановить данные из копии?');
		expect(dialog).toHaveTextContent(
			'Все данные программы заменятся копией от 25 ноября 2026, 10:00'
		);
		await user.click(within(dialog).getByRole('button', { name: 'Восстановить' }));
		await waitFor(() => expect(api.requestRestore).toHaveBeenCalledWith('c1'));
		expect(await screen.findByText(/при следующем запуске программы/)).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: 'Перезапустить сейчас' }));
		await waitFor(() => expect(calls).toContain('restart_app'));
	});

	it('in a browser there is no restart, only the words about mw-server', async () => {
		api.getBackups.mockResolvedValue(overview({ pending }));
		render(Page);
		expect(await screen.findByText(/при следующем запуске программы/)).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Перезапустить сейчас' })).toBeNull();
		expect(
			screen.getByText('Перезапустите сервер mw-server, чтобы восстановить данные.')
		).toBeInTheDocument();
	});

	it('cancels a pending restore', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValueOnce(overview({ pending })).mockResolvedValue(overview());
		api.cancelRestore.mockResolvedValue(undefined);
		render(Page);
		await user.click(await screen.findByRole('button', { name: 'Отменить восстановление' }));
		await waitFor(() => expect(api.cancelRestore).toHaveBeenCalledOnce());
		await waitFor(() => expect(screen.queryByText(/при следующем запуске программы/)).toBeNull());
	});

	it('without «Пользователи» there is no cancel', async () => {
		rights.granted = ['settings_manage'];
		api.getBackups.mockResolvedValue(overview({ pending }));
		render(Page);
		await screen.findByText(/при следующем запуске программы/);
		expect(screen.queryByRole('button', { name: 'Отменить восстановление' })).toBeNull();
	});

	it('reports the last restore and hides the report', async () => {
		const user = userEvent.setup();
		const done = {
			ok: true,
			copy: copy('c1', 'manual', '2026-11-25T10:00:00'),
			requested_at: '2026-11-25T11:00:00',
			requested_by: 'Админ',
			safety: 's',
			error: null
		};
		api.getBackups
			.mockResolvedValueOnce(overview({ last_restore: done }))
			.mockResolvedValue(overview());
		api.cancelRestore.mockResolvedValue(undefined);
		render(Page);
		expect(
			await screen.findByText(
				'Данные восстановлены из копии от 25 ноября 2026, 10:00. Прежние данные сохранены в копии «Перед восстановлением».'
			)
		).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: 'Скрыть' }));
		await waitFor(() => expect(api.cancelRestore).toHaveBeenCalledOnce());
		await waitFor(() => expect(screen.queryByText(/Данные восстановлены/)).toBeNull());
	});

	it('reports a failed restore', async () => {
		api.getBackups.mockResolvedValue(
			overview({
				last_restore: {
					ok: false,
					copy: copy('c1', 'manual', '2026-11-25T10:00:00'),
					requested_at: '2026-11-25T11:00:00',
					requested_by: 'Админ',
					safety: null,
					error: 'копия повреждена'
				}
			})
		);
		render(Page);
		expect(
			await screen.findByText(
				'Восстановить не удалось: копия повреждена. Программа работает с прежними данными.'
			)
		).toBeInTheDocument();
	});
});

describe('the settings of MyWork 2', () => {
	const legacy = { auto: false, folders: ['D:\\Backup'] };

	it('offers them, and «Не нужно» answers no', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValueOnce(overview({ legacy })).mockResolvedValue(overview());
		api.chooseLegacyBackup.mockResolvedValue(settings);
		render(Page);
		expect(
			await screen.findByText(
				'В MyWork 2 ежедневная копия была выключена, папка копий — D:\\Backup.'
			)
		).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: 'Не нужно' }));
		await waitFor(() => expect(api.chooseLegacyBackup).toHaveBeenCalledWith(false));
		await waitFor(() => expect(screen.queryByText(/В MyWork 2 ежедневная/)).toBeNull());
	});

	it('says so when the folder of MyWork 2 is not here', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValueOnce(overview({ legacy })).mockResolvedValue(overview());
		api.chooseLegacyBackup.mockResolvedValue(settings);
		render(Page);
		await user.click(await screen.findByRole('button', { name: 'Взять настройки MyWork 2' }));
		await waitFor(() => expect(api.chooseLegacyBackup).toHaveBeenCalledWith(true));
		await waitFor(() =>
			expect(toastTexts()).toContain(
				'Папки D:\\Backup на этом компьютере нет — копии остаются в папке по умолчанию'
			)
		);
	});
});

describe('what replaces the screen', () => {
	it('shows the reason of an unsupported base', async () => {
		api.getBackups.mockResolvedValue(
			overview({ supported: false, reason: 'Копии для PostgreSQL пока не сделаны' })
		);
		render(Page);
		expect(await screen.findByText('Копии для PostgreSQL пока не сделаны')).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Сделать копию сейчас' })).toBeNull();
		expect(screen.queryByRole('switch')).toBeNull();
	});

	it('shows the text of a refusal of the server', async () => {
		api.getBackups.mockRejectedValue(
			new ApiError(
				403,
				'forbidden',
				'Резервные копии настраиваются на компьютере, где работает сервер',
				null
			)
		);
		render(Page);
		expect(
			await screen.findByText('Резервные копии настраиваются на компьютере, где работает сервер')
		).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Сделать копию сейчас' })).toBeNull();
	});

	it('asks for the right «Настройки» without asking the server', async () => {
		rights.granted = [];
		render(Page);
		expect(
			await screen.findByText('Резервные копии доступны с правом «Настройки»')
		).toBeInTheDocument();
		expect(api.getBackups).not.toHaveBeenCalled();
	});
});

describe('the settings of the copies', () => {
	it('saves the switch', async () => {
		const user = userEvent.setup();
		api.updateBackupSettings.mockResolvedValue({ ...settings, auto: false });
		render(Page);
		await user.click(await screen.findByRole('switch', { name: 'Делать копию каждый день' }));
		await waitFor(() => expect(api.updateBackupSettings).toHaveBeenCalledWith({ auto: false }));
	});

	it('keeps the error of the server under its field', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
		api.updateBackupSettings.mockRejectedValue(validation('keep', 'out_of_range', 'Слишком много'));
		render(Page);
		const field = await screen.findByRole('textbox', { name: 'Хранить автокопий' });
		await user.clear(field);
		await user.type(field, '50');
		expect(screen.queryByText('От 1 до 100')).toBeNull();
		await vi.advanceTimersByTimeAsync(700);
		await waitFor(() => expect(api.updateBackupSettings).toHaveBeenCalledWith({ keep: 50 }));
		expect(await screen.findByText('От 1 до 100')).toBeInTheDocument();
	});

	it('refuses a number out of range without asking the server', async () => {
		const user = userEvent.setup();
		render(Page);
		const field = await screen.findByRole('textbox', { name: 'Хранить автокопий' });
		await user.clear(field);
		await user.type(field, '500');
		expect(await screen.findByText('От 1 до 100')).toBeInTheDocument();
		expect(api.updateBackupSettings).not.toHaveBeenCalled();
	});

	it('shows the text of the server under the folder', async () => {
		const user = userEvent.setup();
		api.updateBackupSettings.mockRejectedValue(
			validation('folder', 'folder_missing', 'Папки нет на этом компьютере')
		);
		render(Page);
		const field = await screen.findByRole('textbox', { name: 'Папка копий' });
		await user.clear(field);
		await user.type(field, '/nowhere');
		expect(await screen.findByText('Папки нет на этом компьютере')).toBeInTheDocument();
	});

	it('returns the folder to the default', async () => {
		const user = userEvent.setup();
		api.getBackups.mockResolvedValue(
			overview({ settings: { ...settings, folder: '/mine' }, folder: '/mine' })
		);
		api.updateBackupSettings.mockResolvedValue(settings);
		render(Page);
		await user.click(await screen.findByRole('button', { name: 'По умолчанию' }));
		await waitFor(() => expect(api.updateBackupSettings).toHaveBeenCalledWith({ folder: null }));
	});

	it('warns that the chosen folder is not used', async () => {
		api.getBackups.mockResolvedValue(overview({ folder_problem: 'Папка только для чтения' }));
		render(Page);
		expect(await screen.findByText('Папка только для чтения')).toBeInTheDocument();
	});
});
