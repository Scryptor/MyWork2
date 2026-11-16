import { describe, expect, it } from 'vitest';
import type { OrderListItem } from '$api/OrderListItem';
import type { StatusKind } from '$api/StatusKind';
import { DEMO_NOW, DEMO_STATUSES, makeOrders } from './fake.js';
import {
	attentionCounts,
	daysSince,
	deviceOf,
	estimateText,
	matchesQuickFilter,
	matchesWords,
	needsAttention,
	orderSearchText,
	queryWords,
	quickFilterItems,
	resolveScan,
	searchClients,
	searchOrders,
	sortOrders,
	toPay,
	withDetails
} from './orders.js';

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
		estimate_from: null,
		estimate_to: null,
		prepayment: 0,
		final_price: null,
		need_call: false,
		need_parts: false,
		barcode: '200000010001',
		deleted_at: null,
		version: 1,
		...patch
	};
}

const kindOf = (id: number): StatusKind | undefined => DEMO_STATUSES.find((s) => s.id === id)?.kind;

describe('withDetails', () => {
	it('is deterministic and keeps the list fields', () => {
		const base = order({ status_id: 7, final_price: 650000 });
		expect(withDetails(base, 'ready')).toEqual(withDetails(base, 'ready'));
		expect(withDetails(base, 'ready')).toMatchObject(base);
	});

	it('orders in work have no work done yet; finished ones mostly have it', () => {
		const orders = makeOrders(400);
		const open = orders.filter(
			(o) => kindOf(o.status_id) !== 'ready' && kindOf(o.status_id) !== 'issued'
		);
		const done = orders.filter((o) => kindOf(o.status_id) === 'issued');
		for (const o of open) expect(withDetails(o, kindOf(o.status_id)!).work_done).toBe('');
		const withWork = done.filter((o) => withDetails(o, 'issued').work_done !== '');
		expect(withWork.length).toBeGreaterThan(done.length / 2);
		expect(withWork.length).toBeLessThan(done.length);
	});

	it('a prepayment has its date, the history starts with the intake', () => {
		const paid = withDetails(order({ prepayment: 200000 }), 'diagnostics');
		expect(paid.prepaid_at).toBe(paid.received_at);
		expect(withDetails(order(), 'diagnostics').prepaid_at).toBeNull();
		expect(paid.history[0]).toMatchObject({ at: paid.received_at, text: 'Принят' });
		expect(paid.discount).toBe(0);
		expect(paid.photos).toEqual([]);
	});

	it('an issued order has the issue in its history', () => {
		const issued = withDetails(order({ status_id: 8, issued_at: '2026-11-16T12:00:00' }), 'issued');
		expect(issued.history.map((h) => h.text)).toEqual(['Принят', 'Выдан']);
	});
});

describe('deviceOf and estimateText', () => {
	it('device is brand and model, or the type when both are empty', () => {
		expect(deviceOf(order())).toBe('Samsung Galaxy A52');
		expect(deviceOf(order({ brand: '', model: '' }))).toBe('Телефон');
	});

	it('estimate is a range, one sum, or empty', () => {
		expect(estimateText(order({ estimate_from: 600000, estimate_to: 700000 }))).toBe('6 000–7 000');
		expect(estimateText(order({ estimate_from: 600000, estimate_to: 600000 }))).toBe('6 000');
		expect(estimateText(order({ estimate_from: 600000 }))).toBe('6 000');
		expect(estimateText(order())).toBe('');
	});
});

describe('daysSince', () => {
	it('counts calendar days to DEMO_NOW', () => {
		expect(daysSince('2026-11-15T09:00:00', DEMO_NOW)).toBe(0);
		expect(daysSince('2026-11-14T23:59:00', DEMO_NOW)).toBe(1);
		expect(daysSince('2026-10-15T10:00:00', DEMO_NOW)).toBe(31);
		expect(daysSince('мусор', DEMO_NOW)).toBeNull();
	});
});

describe('attention', () => {
	it('call and parts follow the flags', () => {
		expect(needsAttention('call', order({ need_call: true }), 'open', DEMO_NOW)).toBe(true);
		expect(needsAttention('call', order(), 'open', DEMO_NOW)).toBe(false);
		expect(needsAttention('parts', order({ need_parts: true }), 'open', DEMO_NOW)).toBe(true);
	});

	it('long diagnostics: kind diagnostics received more than 5 days ago', () => {
		const at = (d: string) => order({ received_at: `${d}T10:00:00` });
		expect(needsAttention('longDiagnostics', at('2026-11-09'), 'diagnostics', DEMO_NOW)).toBe(true);
		expect(needsAttention('longDiagnostics', at('2026-11-10'), 'diagnostics', DEMO_NOW)).toBe(
			false
		);
		expect(needsAttention('longDiagnostics', at('2026-11-01'), 'open', DEMO_NOW)).toBe(false);
	});

	it('long ready: kind ready received more than 30 days ago', () => {
		const at = (d: string) => order({ received_at: `${d}T10:00:00` });
		expect(needsAttention('longReady', at('2026-10-15'), 'ready', DEMO_NOW)).toBe(true);
		expect(needsAttention('longReady', at('2026-10-16'), 'ready', DEMO_NOW)).toBe(false);
		expect(needsAttention('longReady', at('2025-10-15'), 'issued', DEMO_NOW)).toBe(false);
	});

	it('counts every kind of attention over a list', () => {
		const list = [
			order({ need_call: true }),
			order({ need_call: true, need_parts: true }),
			order({ status_id: 1, received_at: '2026-11-01T10:00:00' }),
			order({ status_id: 7, received_at: '2026-01-01T10:00:00' }),
			order({ status_id: 7, received_at: '2026-11-14T10:00:00' })
		];
		expect(attentionCounts(list, kindOf, DEMO_NOW)).toEqual({
			call: 2,
			parts: 1,
			longDiagnostics: 1,
			longReady: 1
		});
	});
});

describe('search', () => {
	const text = orderSearchText(order({ client_name: 'ЕЛКИН ПЁТР', serial: 'SABC123' }));

	it('finds names stored in capitals without case and ё', () => {
		expect(matchesWords(orderSearchText(order()), queryWords('иванов'))).toBe(true);
		expect(matchesWords(text, queryWords('ёлкин'))).toBe(true);
		expect(matchesWords(text, queryWords('петр'))).toBe(true);
		expect(matchesWords(text, queryWords('иванов'))).toBe(false);
	});

	it('finds the number, phone digits or the shown phone, model, serial and barcode', () => {
		for (const q of [
			'10001',
			'70001234567',
			'123-45-67',
			'galaxy a52',
			'sabc123',
			'200000010001'
		]) {
			expect(matchesWords(text, queryWords(q)), q).toBe(true);
		}
	});

	it('an empty query matches everything', () => {
		expect(queryWords('   ')).toEqual([]);
		expect(matchesWords(text, [])).toBe(true);
	});
});

describe('quick filters', () => {
	const list = [
		order({ id: 1, status_id: 1 }),
		order({ id: 2, status_id: 4 }),
		order({ id: 3, status_id: 7 }),
		order({ id: 4, status_id: 8 }),
		order({ id: 5, status_id: 9 })
	];

	it('«В работе», the statuses that have orders, «Выданы» and «Все»', () => {
		expect(quickFilterItems(list, DEMO_STATUSES)).toEqual([
			{ id: 'work', label: 'В работе', count: 2 },
			{ id: 'status:1', label: 'Диагностика', count: 1, color: 'amber' },
			{ id: 'status:4', label: 'Принят в работу', count: 1, color: 'blue' },
			{ id: 'status:7', label: 'Готов', count: 1, color: 'green' },
			{ id: 'issued', label: 'Выданы' },
			{ id: 'all', label: 'Все' }
		]);
	});

	it('a selection matches the union of its chips; nothing selected is everything', () => {
		const pick = (selected: string[]) =>
			list
				.filter((o) => matchesQuickFilter(selected, o.status_id, kindOf(o.status_id)))
				.map((o) => o.id);
		expect(pick(['work'])).toEqual([1, 2]);
		expect(pick(['status:7'])).toEqual([3]);
		expect(pick(['status:7', 'issued'])).toEqual([3, 4, 5]);
		expect(pick(['all'])).toEqual([1, 2, 3, 4, 5]);
		expect(pick([])).toEqual([1, 2, 3, 4, 5]);
	});
});

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

describe('toPay', () => {
	it('is the final price minus prepayment and discount', () => {
		expect(toPay({ final_price: 650000, prepayment: 200000, discount: 0 })).toBe(450000);
		expect(toPay({ final_price: 650000, prepayment: 200000, discount: 50000 })).toBe(400000);
		expect(toPay({ final_price: 100000, prepayment: 200000, discount: 0 })).toBe(-100000);
		expect(toPay({ final_price: null, prepayment: 200000, discount: 0 })).toBeNull();
	});
});

describe('palette search over all orders', () => {
	const orders = makeOrders(2000);

	it('a number shows that order first, then the newest matches', () => {
		const found = searchOrders(orders, '10500', 8);
		expect(found[0]?.id).toBe(10500);
		const rest = found.slice(1).map((o) => o.received_at);
		expect(rest).toEqual([...rest].sort().reverse());
	});

	it('words find the newest orders first and stop at the limit', () => {
		const found = searchOrders(orders, 'иванов', 5);
		expect(found).toHaveLength(5);
		for (const o of found) expect(o.client_name).toContain('ИВАНОВ');
		const at = found.map((o) => o.received_at);
		expect(at).toEqual([...at].sort().reverse());
		expect(searchOrders(orders, '', 5)).toEqual([]);
	});

	it('clients are found once each, with their number of orders', () => {
		const clients = searchClients(orders, 'елкин петр сергеевич', 5);
		const fixed = clients.find((c) => c.phone === '70000000001');
		expect(fixed?.name).toBe('ЕЛКИН ПЁТР СЕРГЕЕВИЧ');
		expect(fixed?.count).toBe(orders.filter((o) => o.client_id === fixed?.id).length);
		expect(new Set(clients.map((c) => c.id)).size).toBe(clients.length);
	});
});

describe('resolveScan', () => {
	const orders = makeOrders(20);

	it('an order number or a barcode opens the order', () => {
		expect(resolveScan({ kind: 'order', id: 10005 }, orders)).toEqual({ kind: 'open', id: 10005 });
		expect(resolveScan({ kind: 'barcode', value: '200000010007' }, orders)).toEqual({
			kind: 'open',
			id: 10007
		});
	});

	it('unknown numbers and barcodes are reported', () => {
		expect(resolveScan({ kind: 'order', id: 99 }, orders)).toEqual({
			kind: 'missing',
			message: 'Заказ № 99 не найден'
		});
		expect(resolveScan({ kind: 'barcode', value: '200000099999' }, orders)).toEqual({
			kind: 'missing',
			message: 'Заказ со штрихкодом 200000099999 не найден'
		});
	});

	it('other text goes to the search', () => {
		expect(resolveScan({ kind: 'text', value: 'SABC1234' }, orders)).toEqual({
			kind: 'search',
			query: 'SABC1234'
		});
	});
});
