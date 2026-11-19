import { render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';

// «Настройки → Импорт из MyWork 2»: what the server is (SQLite or not) comes from «О программе». Without an
// answer (no connection) the page says so — not «Перенос с заменой есть только для базы SQLite».
const api = vi.hoisted(() => ({ currentLegacyImport: vi.fn(), listLegacyImports: vi.fn() }));
vi.mock('#lib/api/import.js', () => api);
const catalogs = vi.hoisted(() => ({ getAbout: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => catalogs);
vi.mock('$app/navigation', () => ({ goto: vi.fn() }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => true } }));

import Page from '../../routes/(app)/settings/import/+page.svelte';

const about = (database: string) => ({
	version: '0.1.0',
	database,
	data_dir: '/data',
	orders: 0,
	clients: 0
});

beforeEach(() => {
	for (const f of [...Object.values(api), catalogs.getAbout]) f.mockReset();
	api.listLegacyImports.mockResolvedValue([]);
	api.currentLegacyImport.mockResolvedValue(null);
});

describe('«Импорт из MyWork 2» in the settings', () => {
	it('no answer about the base: the network error, not «только SQLite»', async () => {
		catalogs.getAbout.mockRejectedValue(new ApiError(0, 'network', 'Нет связи с сервером', null));
		render(Page);
		expect(await screen.findByText('Нет связи с сервером')).toBeInTheDocument();
		expect(screen.queryByText(/только для базы SQLite/)).not.toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /Перенести/ })).not.toBeInTheDocument();
	});

	it('PostgreSQL: the replacement is for SQLite only', async () => {
		catalogs.getAbout.mockResolvedValue(about('PostgreSQL'));
		render(Page);
		expect(
			await screen.findByText('Перенос с заменой есть только для базы SQLite')
		).toBeInTheDocument();
	});

	it('SQLite: the transfer is offered', async () => {
		catalogs.getAbout.mockResolvedValue(about('SQLite'));
		render(Page);
		expect(
			await screen.findByRole('button', { name: 'Перенести из MyWork 2' })
		).toBeInTheDocument();
	});
});
