import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FolderCheck } from '$api/FolderCheck';

// The first run on a base that already has users (an administrator made by `mw-server create-admin`): the
// server answers the check with `replace_required` (SQLite), and the wizard offers the replacement with the
// same confirmation as «Настройки», naming the folder of the copy.
const api = vi.hoisted(() => ({
	checkLegacyFolder: vi.fn(),
	startLegacyImport: vi.fn(),
	currentLegacyImport: vi.fn()
}));
vi.mock('#lib/api/import.js', () => api);
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import ImportWizard from './ImportWizard.svelte';

const PATH = '/Users/u/MyWork2';
const found = (over: Partial<FolderCheck> = {}): FolderCheck => ({
	ok: true,
	problem: null,
	schema_version: 'колонок в заказах — 32',
	orders: 11,
	clients: 9,
	files: 6,
	files_bytes: 45,
	replace_required: false,
	backups_dir: null,
	...over
});
const notEmpty = (replace_required: boolean): FolderCheck =>
	found({
		ok: false,
		problem: 'В базе MyWork 3 уже есть данные (пользователи: 1). Перенести можно только с заменой',
		schema_version: null,
		orders: 0,
		clients: 0,
		files: 0,
		files_bytes: 0,
		replace_required
	});

beforeEach(() => {
	for (const f of Object.values(api)) f.mockReset();
	api.currentLegacyImport.mockResolvedValue(null);
	api.startLegacyImport.mockReturnValue(new Promise(() => {}));
});

async function check() {
	await userEvent.type(screen.getByRole('textbox', { name: /Путь к папке MyWork 2/ }), PATH);
	await userEvent.click(screen.getByRole('button', { name: 'Проверить' }));
}

describe('ImportWizard on a base that already has users', () => {
	it('offers the replacement, checks again with it and asks for the confirmation of «Настройки»', async () => {
		api.checkLegacyFolder
			.mockResolvedValueOnce(notEmpty(true))
			.mockResolvedValueOnce(found({ backups_dir: '/data/MyWork3/backups' }));
		render(ImportWizard, {});
		await check();
		expect(api.checkLegacyFolder).toHaveBeenLastCalledWith({ path: PATH, replace: false });
		expect(await screen.findByText(/уже есть данные \(пользователи: 1\)/)).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Перенести' })).not.toBeInTheDocument();

		await userEvent.click(screen.getByRole('button', { name: 'Перенести с заменой' }));
		expect(api.checkLegacyFolder).toHaveBeenLastCalledWith({ path: PATH, replace: true });
		await userEvent.click(await screen.findByRole('button', { name: 'Перенести' }));
		const dialog = await screen.findByRole('alertdialog');
		expect(within(dialog).getByText('Заменить данные?')).toBeInTheDocument();
		const text = within(dialog).getByText(/Пользователи и пароли станут такими, как в MyWork 2/);
		expect(text).toHaveTextContent('/data/MyWork3/backups');
		expect(api.startLegacyImport).not.toHaveBeenCalled();
		await userEvent.click(within(dialog).getByRole('button', { name: 'Заменить' }));
		expect(api.startLegacyImport).toHaveBeenCalledWith({ path: PATH, replace: true });
	});

	it('does not offer it where the server cannot replace (PostgreSQL)', async () => {
		api.checkLegacyFolder.mockResolvedValueOnce(notEmpty(false));
		render(ImportWizard, {});
		await check();
		expect(await screen.findByText(/уже есть данные/)).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: 'Перенести с заменой' })).not.toBeInTheDocument();
	});

	it('an empty base is transferred without a confirmation', async () => {
		api.checkLegacyFolder.mockResolvedValueOnce(found());
		render(ImportWizard, {});
		await check();
		await userEvent.click(await screen.findByRole('button', { name: 'Перенести' }));
		expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
		expect(api.startLegacyImport).toHaveBeenCalledWith({ path: PATH, replace: false });
	});

	it('without the folder of the copy the confirmation says where it goes', async () => {
		api.checkLegacyFolder.mockResolvedValueOnce(found());
		render(ImportWizard, { replace: true });
		await check();
		expect(api.checkLegacyFolder).toHaveBeenLastCalledWith({ path: PATH, replace: true });
		await userEvent.click(await screen.findByRole('button', { name: 'Перенести' }));
		const dialog = await screen.findByRole('alertdialog');
		expect(
			within(dialog).getByText(/папке backups внутри папки данных программы/)
		).toBeInTheDocument();
	});
});
