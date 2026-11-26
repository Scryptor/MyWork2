import { describe, expect, it } from 'vitest';
import { formatRelative } from '#lib/format/date.js';
import {
	DEMO_NOW,
	DEMO_POINTS,
	DEMO_STATUSES,
	makeDictionaries,
	makeOrders,
	sortOrders
} from './fake.js';
import type { OrderListItem } from '$api/OrderListItem';

const orders = makeOrders(2000);

describe('makeOrders', () => {
	it('is deterministic for a seed', () => {
		expect(makeOrders(3, 1)).toEqual(makeOrders(3, 1));
		expect(makeOrders(3, 1)).not.toEqual(makeOrders(3, 2));
	});

	it('makes the requested number of orders with unique ascending numbers', () => {
		expect(orders).toHaveLength(2000);
		const ids = orders.map((o) => o.id);
		expect(ids).toEqual([...ids].sort((a, b) => a - b));
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('takes orders in time order', () => {
		const received = orders.map((o) => o.received_at);
		expect(received).toEqual([...received].sort());
	});

	it('every phone starts with 7000', () => {
		for (const o of orders) expect(o.client_phone).toMatch(/^7000\d{7}$/);
	});

	it('every name is in capitals', () => {
		for (const o of orders) {
			expect(o.client_name).not.toBe('');
			expect(o.client_name).toBe(o.client_name.toUpperCase());
			expect(o.master).toBe(o.master.toUpperCase());
		}
	});

	it('every order has a status from DEMO_STATUSES', () => {
		const ids = new Set(DEMO_STATUSES.map((s) => s.id));
		for (const o of orders) expect(ids.has(o.status_id)).toBe(true);
	});

	it('only issued statuses carry an issue date', () => {
		const kind = new Map(DEMO_STATUSES.map((s) => [s.id, s.kind]));
		for (const o of orders) {
			expect(o.issued_at !== null, `№ ${o.id}`).toBe(kind.get(o.status_id) === 'issued');
		}
	});

	it('has the fixed fictional client with phone 70000000001', () => {
		const own = orders.filter((o) => o.client_phone === '70000000001');
		expect(own.length).toBeGreaterThan(0);
		expect(new Set(own.map((o) => o.client_id)).size).toBe(1);
		expect(makeOrders(1)[0]?.client_phone).toBe('70000000001');
	});

	it('uses only demo service points', () => {
		const points = new Set(DEMO_POINTS.map((p) => p.id));
		for (const o of orders) {
			if (o.service_point_id !== null) expect(points.has(o.service_point_id)).toBe(true);
		}
	});
});

describe('DEMO_NOW', () => {
	it('is the moment the data is made around: 15 November 2026, 18:00', () => {
		expect(DEMO_NOW).toEqual({ year: 2026, month: 11, day: 15, hour: 18, minute: 0, second: 0 });
	});

	it('no order is received after it, the newest ones are «сегодня»', () => {
		const now = '2026-11-15T18:00:00';
		for (const o of orders) {
			expect(o.received_at <= now, `№ ${o.id}`).toBe(true);
			if (o.issued_at !== null) expect(o.issued_at <= now, `№ ${o.id}`).toBe(true);
		}
		expect(formatRelative(orders[orders.length - 1]!.received_at, DEMO_NOW)).toMatch(/^сегодня /);
	});
});

describe('DEMO_STATUSES', () => {
	it('starts with the eight default statuses, then «Куплен»', () => {
		expect(DEMO_STATUSES.map((s) => [s.id, s.name, s.kind, s.color])).toEqual([
			[1, 'Диагностика', 'diagnostics', 'amber'],
			[2, 'Согласование с клиентом', 'open', 'violet'],
			[3, 'Согласовано', 'open', 'indigo'],
			[4, 'Принят в работу', 'open', 'blue'],
			[5, 'Ждёт запчасть', 'open', 'red'],
			[6, 'Принят по гарантии', 'open', 'teal'],
			[7, 'Готов', 'ready', 'green'],
			[8, 'Выдан', 'issued', 'gray'],
			[9, 'Куплен', 'issued', 'gray']
		]);
	});
});

describe('DEMO_POINTS and makeDictionaries', () => {
	it('lists the three demo points', () => {
		expect(DEMO_POINTS.map((p) => p.name)).toEqual(['Центр', 'Северная', 'Южная']);
	});

	it('gives every dictionary and the three masters', () => {
		const d = makeDictionaries();
		for (const kind of [
			'device_type',
			'brand',
			'malfunction',
			'completeness',
			'condition',
			'master',
			'warranty',
			'source'
		] as const) {
			expect(d[kind].length, kind).toBeGreaterThan(0);
		}
		expect(d.master).toEqual(['АНТОН', 'СЕРГЕЙ', 'ИРИНА']);
		expect(d.completeness).toContain('Чехол');
		expect(d.warranty).toContain('30 дней');
	});
});

function order(patch: Partial<OrderListItem> = {}): OrderListItem {
	return {
		id: 10001,
		received_at: '2026-11-15T10:00:00',
		issued_at: null,
		client_id: 2,
		client_name: 'ИВАНОВ ИВАН',
		client_phone: '70001234567',
		client_blacklisted: false,
		service_point_id: 1,
		status_id: 1,
		device_type: 'Телефон',
		brand: 'Samsung',
		model: 'Galaxy A52',
		serial: '356789104512345',
		malfunction: 'Не включается',
		master: 'АНТОН',
		master_user_id: 1,
		estimate_from: null,
		estimate_to: null,
		prepayment: 0,
		paid: 0,
		final_price: null,
		discount: 0,
		need_call: false,
		need_parts: false,
		barcode: '200000010001',
		deleted_at: null,
		version: 1,
		...patch
	};
}

describe('sortOrders', () => {
	const ctx = {
		statusOrder: (id: number) => DEMO_STATUSES.find((s) => s.id === id)?.sort_order ?? 0,
		pointName: () => ''
	};
	const list = [
		order({ id: 1, received_at: '2026-11-10T10:00:00', status_id: 7, client_name: 'БЕЛОВ' }),
		order({ id: 2, received_at: '2026-11-12T10:00:00', status_id: 1, client_name: 'АНДРЕЕВ' }),
		order({ id: 3, received_at: '2026-11-12T10:00:00', status_id: 4, client_name: 'ВОЛКОВ' })
	];
	const ids = (s: { columnId: string; descending: boolean }) =>
		sortOrders(list, s, ctx).map((o) => o.id);

	it('by «Принят», ties newest first', () => {
		expect(ids({ columnId: 'received', descending: true })).toEqual([3, 2, 1]);
		expect(ids({ columnId: 'received', descending: false })).toEqual([1, 3, 2]);
	});

	it('by client in Russian order and by the status order', () => {
		expect(ids({ columnId: 'client', descending: false })).toEqual([2, 1, 3]);
		expect(ids({ columnId: 'status', descending: false })).toEqual([2, 3, 1]);
	});
});
