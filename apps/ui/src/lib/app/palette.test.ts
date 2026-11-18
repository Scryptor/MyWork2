import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import type { Order } from '$api/Order';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import type { CommandItem } from '#lib/ui/commands.js';
import { STATUSES, client, clientListItem, item, order } from './fixtures.test-helper.js';
import {
	PALETTE_DELAY_MS,
	createPaletteSearch,
	exactNumber,
	loadRecent,
	paletteGroups,
	type PaletteResults
} from './palette.js';

const COMMANDS: CommandItem[] = [
	{ id: 'cmd:intake', label: 'Принять устройство' },
	{ id: 'cmd:theme:dark', label: 'Тема: тёмная' }
];
const statusOf = (id: number) => STATUSES.find((s) => s.id === id);

describe('paletteGroups', () => {
	it('puts the exact order number first without a duplicate', () => {
		const g = paletteGroups({
			query: '104',
			orders: [item(10481), item(104)],
			exact: order(104),
			clients: [],
			recent: [],
			commands: []
		});
		expect(g[0]!.items.map((i) => i.id)).toEqual(['order:104', 'order:10481']);
	});

	it('empty query shows recent and commands', () => {
		const g = paletteGroups({
			query: '  ',
			orders: [item(5)],
			exact: null,
			clients: [clientListItem(7)],
			recent: [item(3), item(1)],
			commands: COMMANDS
		});
		expect(g.map((x) => x.title)).toEqual(['Недавние', 'Команды']);
		expect(g[0]!.items.map((i) => i.id)).toEqual(['order:3', 'order:1']);
		expect(g[1]!.items).toEqual(COMMANDS);
	});

	it('an order shows its device, client and status; an exact order outside the search comes first', () => {
		const g = paletteGroups({
			query: '42',
			orders: [item(4200, { brand: 'Samsung', model: 'A52', client_name: 'ПЕТРОВ ВИКТОР' })],
			exact: order(42, { brand: '', model: '', device_type: 'Ноутбук', status_id: 3 }),
			clients: [],
			recent: [],
			commands: [],
			statusOf
		});
		const [exact, other] = g[0]!.items;
		expect(exact).toMatchObject({
			id: 'order:42',
			label: '№ 42',
			hint: 'Ноутбук',
			status: { name: 'Готов', color: 'green' }
		});
		expect(other).toMatchObject({ label: '№ 4200', hint: 'Samsung A52 · Петров В.' });
	});

	it('a query gives orders, clients and the commands that match it', () => {
		const g = paletteGroups({
			query: 'тём',
			orders: [],
			exact: null,
			clients: [
				clientListItem(7, {
					full_name: 'ТЁМКИН ОЛЕГ',
					phones: [{ phone: '89211234567', note: '' }]
				})
			],
			recent: [item(1)],
			commands: COMMANDS
		});
		expect(g.map((x) => x.title)).toEqual(['Заказы', 'Клиенты', 'Команды']);
		expect(g[1]!.items).toEqual([
			{ id: 'client:7', label: 'Тёмкин Олег', hint: '+7 921 123-45-67' }
		]);
		expect(g[2]!.items.map((i) => i.id)).toEqual(['cmd:theme:dark']);
	});
});

describe('exactNumber', () => {
	it('is a number of 1 to 7 digits', () => {
		expect(exactNumber('104')).toBe(104);
		expect(exactNumber(' 1234567 ')).toBe(1234567);
		expect(exactNumber('12345678')).toBeNull();
		expect(exactNumber('0')).toBeNull();
		expect(exactNumber('10a')).toBeNull();
		expect(exactNumber('')).toBeNull();
	});
});

describe('createPaletteSearch', () => {
	beforeEach(() => vi.useFakeTimers());
	afterEach(() => vi.useRealTimers());

	function setup() {
		const signals: AbortSignal[] = [];
		const deps = {
			listOrders: vi.fn(async (_q: unknown, signal?: AbortSignal) => {
				if (signal) signals.push(signal);
				return { items: [item(104)], total: 1 };
			}),
			getOrder: vi.fn<(id: number, signal?: AbortSignal) => Promise<Order>>(async (id) =>
				order(id)
			),
			listClients: vi.fn<(q: ClientListQuery, signal?: AbortSignal) => Promise<ClientPage>>(
				async () => ({ items: [clientListItem(7)], total: 1 })
			)
		};
		const results: (PaletteResults | null)[] = [];
		const errors: [unknown, string][] = [];
		const search = createPaletteSearch(deps, {
			result: (r) => results.push(r),
			error: (e, query) => errors.push([e, query])
		});
		return { deps, signals, results, errors, search };
	}

	it('waits 150 ms after the last key and asks orders, the exact number and clients', async () => {
		const { deps, results, search } = setup();
		search.search('1');
		search.search('10');
		search.search('104');
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS - 1);
		expect(deps.listOrders).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1);
		expect(deps.listOrders).toHaveBeenCalledTimes(1);
		expect(deps.listOrders.mock.calls[0]![0]).toEqual({ query: '104', limit: 5 });
		expect(deps.getOrder.mock.calls[0]![0]).toBe(104);
		expect(deps.listClients.mock.calls[0]![0]).toEqual({ query: '104', limit: 5 });
		expect(results).toEqual([
			{ query: '104', orders: [item(104)], exact: order(104), clients: [clientListItem(7)] }
		]);
	});

	it('a new query aborts the request in flight; its late answer is dropped', async () => {
		const { deps, signals, results, search } = setup();
		let release: () => void = () => {};
		deps.listOrders.mockImplementationOnce(async (_q: unknown, signal?: AbortSignal) => {
			if (signal) signals.push(signal);
			await new Promise<void>((done) => (release = done));
			return { items: [item(1)], total: 1 };
		});
		search.search('старый');
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS);
		search.search('новый');
		expect(signals[0]!.aborted).toBe(true);
		release();
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS);
		expect(results.map((r) => r?.query)).toEqual(['новый']);
	});

	it('a missing exact number is silent, a text query does not ask for it', async () => {
		const { deps, results, errors, search } = setup();
		deps.getOrder.mockRejectedValueOnce(new ApiError(404, 'not_found', 'Нет', null));
		search.search('999');
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS);
		expect(results[0]).toMatchObject({ exact: null });
		search.search('Петров');
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS);
		expect(deps.getOrder).toHaveBeenCalledTimes(1);
		expect(errors).toEqual([]);
	});

	it('an empty query answers null at once; other failures go to error', async () => {
		const { deps, results, errors, search } = setup();
		search.search('   ');
		expect(results).toEqual([null]);
		deps.listClients.mockRejectedValueOnce(
			new ApiError(0, 'network', 'Нет связи с сервером', null)
		);
		search.search('Петров');
		await vi.advanceTimersByTimeAsync(PALETTE_DELAY_MS);
		expect(errors).toHaveLength(1);
		expect(errors[0]![1]).toBe('Петров');
		expect(results).toEqual([null]);
	});
});

describe('loadRecent', () => {
	it('keeps the order of the numbers, adds the client and skips orders that are gone', async () => {
		const getOrder = vi.fn(async (id: number) => {
			if (id === 2) throw new ApiError(404, 'not_found', 'Нет', null);
			return order(id, { client_id: 70 });
		});
		const getClient = vi.fn(async (id: number) => client(id, { full_name: 'ПЕТРОВ ВИКТОР' }));
		const items = await loadRecent([3, 2, 1], { getOrder, getClient });
		expect(items.map((i) => i.id)).toEqual([3, 1]);
		expect(items[0]).toMatchObject({ client_id: 70, client_name: 'ПЕТРОВ ВИКТОР' });
		expect(getClient).toHaveBeenCalledTimes(1);
	});
});
