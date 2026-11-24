import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LegacyStockState } from '$api/LegacyStockState';
import type { StockImportSummary } from '$api/StockImportSummary';
import { ApiError } from '#lib/api/client.js';

// «Настройки → Импорт из MyWork 2»: what the server is (SQLite or not) comes from «О программе». Without an
// answer (no connection) the page says so — not «Перенос с заменой есть только для базы SQLite».
const api = vi.hoisted(() => ({
	currentLegacyImport: vi.fn(),
	listLegacyImports: vi.fn(),
	legacyStockState: vi.fn(),
	migrateLegacyStock: vi.fn()
}));
vi.mock('#lib/api/import.js', () => api);
const catalogs = vi.hoisted(() => ({ getAbout: vi.fn() }));
vi.mock('#lib/api/catalogs.js', () => catalogs);
const nav = vi.hoisted(() => ({ goto: vi.fn() }));
vi.mock('$app/navigation', () => nav);
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { can: () => true } }));

import Page from './ImportPageHarness.test-helper.svelte';

const about = (database: string) => ({
	version: '0.1.0',
	database,
	data_dir: '/data',
	orders: 0,
	clients: 0
});

const stockState = (available: boolean): LegacyStockState => ({
	import_id: 3,
	stock_rows: 4,
	stock_map_rows: 6,
	available
});

beforeEach(() => {
	for (const f of [...Object.values(api), catalogs.getAbout, nav.goto]) f.mockReset();
	api.listLegacyImports.mockResolvedValue([]);
	api.currentLegacyImport.mockResolvedValue(null);
	api.legacyStockState.mockResolvedValue({
		import_id: null,
		stock_rows: 0,
		stock_map_rows: 0,
		available: false
	});
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
		// What the replacement does is a hint next to its words.
		expect(
			screen.getByRole('button', { name: 'Что такое «Перенос с заменой»' })
		).toBeInTheDocument();
	});
});

// The stock of an import made before the stock module (decision 17): offered while the base has no stock.
describe('the stock of a past import', () => {
	const MIGRATE = 'Перенести склад из прошлого импорта';

	it('is not offered when it cannot be transferred', async () => {
		catalogs.getAbout.mockResolvedValue(about('SQLite'));
		api.legacyStockState.mockResolvedValue(stockState(false));
		render(Page);
		await screen.findByRole('button', { name: 'Перенести из MyWork 2' });
		await vi.waitFor(() => expect(api.legacyStockState).toHaveBeenCalled());
		expect(screen.queryByRole('button', { name: MIGRATE })).not.toBeInTheDocument();
		expect(screen.queryByText('Склад MyWork 2 ещё не перенесён')).not.toBeInTheDocument();
	});

	it('is transferred once and its report is opened', async () => {
		catalogs.getAbout.mockResolvedValue(about('SQLite'));
		api.legacyStockState.mockResolvedValue(stockState(true));
		let finish: (s: StockImportSummary) => void = () => {};
		api.migrateLegacyStock.mockReturnValue(new Promise((r) => (finish = r)));
		render(Page);
		const button = await screen.findByRole('button', { name: MIGRATE });
		expect(screen.getByText('Склад MyWork 2 ещё не перенесён')).toBeInTheDocument();
		expect(screen.getByText(/4 позиции, 6 списаний в заказы/)).toBeInTheDocument();
		await userEvent.click(button);
		// A second click while the request runs sends nothing.
		await userEvent.click(button);
		expect(api.migrateLegacyStock).toHaveBeenCalledTimes(1);
		finish({ import_id: 3, stock_items: 4, order_parts: 5, orders_changed: 2 });
		await vi.waitFor(() => expect(nav.goto).toHaveBeenCalledWith('/settings/import/3'));
	});

	it('a refusal is shown', async () => {
		catalogs.getAbout.mockResolvedValue(about('PostgreSQL'));
		api.legacyStockState.mockResolvedValue(stockState(true));
		api.migrateLegacyStock.mockRejectedValue(
			new ApiError(409, 'conflict', 'Импорт уже идёт', null)
		);
		render(Page);
		await userEvent.click(await screen.findByRole('button', { name: MIGRATE }));
		expect(await screen.findByText('Импорт уже идёт')).toBeInTheDocument();
		expect(nav.goto).not.toHaveBeenCalled();
	});

	it('after a refusal the offer is read again and goes when the stock is no longer to transfer', async () => {
		catalogs.getAbout.mockResolvedValue(about('SQLite'));
		api.legacyStockState
			.mockResolvedValueOnce(stockState(true))
			.mockResolvedValueOnce(stockState(false));
		api.migrateLegacyStock.mockRejectedValue(
			new ApiError(422, 'validation', 'Проверьте введённые данные', {
				code: 'validation',
				message: 'Проверьте введённые данные',
				fields: [
					{
						field: 'stock',
						code: 'not_empty',
						message: 'На складе или в заказах уже есть запчасти: перенос склада не нужен'
					}
				],
				current_version: null,
				permission: null,
				retry_after_secs: null,
				diagnostics: []
			})
		);
		render(Page);
		await userEvent.click(await screen.findByRole('button', { name: MIGRATE }));
		expect(
			await screen.findByText('На складе или в заказах уже есть запчасти: перенос склада не нужен')
		).toBeInTheDocument();
		await vi.waitFor(() =>
			expect(screen.queryByRole('button', { name: MIGRATE })).not.toBeInTheDocument()
		);
		expect(screen.queryByText('Склад MyWork 2 ещё не перенесён')).not.toBeInTheDocument();
		expect(api.legacyStockState).toHaveBeenCalledTimes(2);
	});
});
