import { describe, expect, it } from 'vitest';
import { DEMO_POINTS, DEMO_STATUSES, makeDictionaries, makeOrders } from './fake.js';

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
			'master'
		] as const) {
			expect(d[kind].length, kind).toBeGreaterThan(0);
		}
		expect(d.master).toEqual(['АНТОН', 'СЕРГЕЙ', 'ИРИНА']);
		expect(d.completeness).toContain('Чехол');
	});
});
