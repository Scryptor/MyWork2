import type { OrderSummary } from '$api/OrderSummary';
import type { Status } from '$api/Status';
import { describe, expect, it } from 'vitest';
import { STATUSES, item } from '#lib/app/fixtures.test-helper.js';
import {
	appliedFilters,
	defaultView,
	deviceOf,
	displayedChips,
	estimateText,
	hasFilters,
	isOverdueDiagnostics,
	parseSavedViews,
	quickFilterItems,
	resetFilters,
	toPay,
	viewFromSearch,
	viewToQuery,
	viewToSearch,
	withChips,
	withoutFilter,
	type OrdersView
} from './view.js';

const summaryOf = (counts: Record<number, number>): OrderSummary => ({
	by_status: Object.entries(counts).map(([id, count]) => ({ status_id: Number(id), count })),
	need_call: 0,
	need_parts: 0,
	overdue_diagnostics: 0,
	stale_ready: 0
});

describe('viewToQuery', () => {
	it('work chip asks for open and diagnostics statuses', () => {
		const q = viewToQuery(defaultView(), STATUSES, null);
		expect(q.status_ids).toEqual(
			STATUSES.filter((s) => s.kind === 'open' || s.kind === 'diagnostics').map((s) => s.id)
		);
		expect(q).toMatchObject({ sort: 'received_at', descending: true });
	});

	it('attention replaces chips', () => {
		expect(viewToQuery({ ...defaultView(), attention: 'call' }, STATUSES, null)).toMatchObject({
			need_call: true,
			issued: 'in_service',
			status_ids: undefined
		});
		expect(viewToQuery({ ...defaultView(), attention: 'parts' }, STATUSES, null)).toMatchObject({
			need_parts: true,
			issued: 'in_service',
			status_ids: undefined
		});
		expect(
			viewToQuery({ ...defaultView(), attention: 'longDiagnostics' }, STATUSES, null)
		).toMatchObject({ overdue_diagnostics: true, status_ids: undefined });
		expect(viewToQuery({ ...defaultView(), attention: 'longReady' }, STATUSES, null)).toMatchObject(
			{ stale_ready: true }
		);
	});

	it('issued alone asks for issued orders; all asks for any; status chips are their ids', () => {
		const v = defaultView();
		expect(viewToQuery({ ...v, chips: ['issued'] }, STATUSES, null)).toMatchObject({
			issued: 'issued',
			status_ids: undefined
		});
		expect(viewToQuery({ ...v, chips: ['all'] }, STATUSES, null)).toMatchObject({
			issued: undefined,
			status_ids: undefined
		});
		expect(viewToQuery({ ...v, chips: [] }, STATUSES, null).status_ids).toBeUndefined();
		expect(viewToQuery({ ...v, chips: ['3'] }, STATUSES, null).status_ids).toEqual([3]);
		// Ctrl+click adds: the union of what each chip asks for.
		expect(viewToQuery({ ...v, chips: ['work', '3'] }, STATUSES, null).status_ids).toEqual([
			1, 2, 3
		]);
		expect(viewToQuery({ ...v, chips: ['issued', '3'] }, STATUSES, null)).toMatchObject({
			issued: undefined,
			status_ids: [4, 3]
		});
		expect(viewToQuery({ ...v, chips: ['3', 'all'] }, STATUSES, null).status_ids).toBeUndefined();
	});

	it('statuses of «Фильтры» take the place of the chips', () => {
		const q = viewToQuery({ ...defaultView(), filters: { statusIds: [3, 4] } }, STATUSES, null);
		expect(q.status_ids).toEqual([3, 4]);
		expect(q.issued).toBeUndefined();
	});

	it('the point of the filters wins over the top bar', () => {
		expect(viewToQuery(defaultView(), STATUSES, 7).service_point_id).toBe(7);
		expect(viewToQuery({ ...defaultView(), filters: { pointId: 2 } }, STATUSES, 7)).toMatchObject({
			service_point_id: 2
		});
		expect(viewToQuery(defaultView(), STATUSES, null).service_point_id).toBeUndefined();
	});

	it('filters, periods, query, client and sort go to the server', () => {
		const v: OrdersView = {
			...defaultView(),
			query: '  a52 ',
			clientId: 12,
			sort: { columnId: 'device', descending: false },
			filters: {
				deviceType: 'Смартфон',
				brand: 'SAMSUNG',
				master: 'Антон',
				received: ['2026-11-01', '2026-11-15'],
				issued: [null, '2026-11-16'],
				needCall: true,
				needParts: false
			}
		};
		expect(viewToQuery(v, STATUSES, null)).toMatchObject({
			query: 'a52',
			client_id: 12,
			device_type: 'Смартфон',
			brand: 'SAMSUNG',
			master: 'Антон',
			received_from: '2026-11-01T00:00:00',
			received_to: '2026-11-15T23:59:59',
			issued_from: undefined,
			issued_to: '2026-11-16T23:59:59',
			need_call: true,
			need_parts: false,
			sort: 'brand',
			descending: false
		});
		expect(viewToQuery(defaultView(), STATUSES, null)).toMatchObject({
			query: undefined,
			client_id: undefined
		});
	});
});

describe('the view in the address', () => {
	it('round-trips through the address', () => {
		const v = {
			...defaultView(),
			query: 'a52',
			filters: { brand: 'Samsung', received: ['2026-11-01', null] as [string, null] }
		};
		expect(viewFromSearch(new URLSearchParams(viewToSearch(v)))).toEqual(v);
	});

	it('round-trips everything', () => {
		const v: OrdersView = {
			chips: ['3', 'issued'],
			attention: 'longReady',
			query: 'Иванов 921',
			clientId: 5,
			sort: { columnId: 'device', descending: false },
			filters: {
				deviceType: 'Ноутбук',
				brand: 'Asus',
				master: 'Антон',
				pointId: 2,
				statusIds: [1, 3],
				received: [null, '2026-11-15'],
				issued: ['2026-10-01', '2026-10-31'],
				needCall: false,
				needParts: true
			}
		};
		expect(viewFromSearch(new URLSearchParams(viewToSearch(v)))).toEqual(v);
	});

	it('the default view is an empty address and an empty address is the default view', () => {
		expect(viewToSearch(defaultView())).toBe('');
		expect(viewFromSearch(new URLSearchParams(''))).toEqual(defaultView());
		expect(defaultView()).toEqual({
			chips: ['work'],
			attention: null,
			query: '',
			filters: {},
			sort: { columnId: 'received', descending: true },
			clientId: null
		});
	});

	it('the names of the address are short and fixed', () => {
		const v: OrdersView = {
			...defaultView(),
			chips: ['issued'],
			attention: 'call',
			query: 'a52',
			clientId: 5,
			sort: { columnId: 'id', descending: false },
			filters: { brand: 'Asus', statusIds: [1, 3], received: ['2026-11-01', null], needCall: true }
		};
		const p = new URLSearchParams(viewToSearch(v));
		expect(Object.fromEntries(p)).toEqual({
			chips: 'issued',
			attention: 'call',
			q: 'a52',
			client: '5',
			sort: 'id',
			desc: '0',
			brand: 'Asus',
			st: '1,3',
			rf: '2026-11-01',
			call: '1'
		});
	});

	it('garbage in the address gives the defaults', () => {
		const v = viewFromSearch(
			new URLSearchParams(
				'attention=later&sort=malfunction&desc=x&client=abc&point=-1&st=x,&rf=15.11.2026&call=yes&chips=work,,7'
			)
		);
		expect(v).toEqual({ ...defaultView(), chips: ['work', '7'] });
	});

	it('an empty chips parameter selects nothing', () => {
		expect(viewFromSearch(new URLSearchParams('chips=')).chips).toEqual([]);
		expect(viewToSearch({ ...defaultView(), chips: [] })).toBe('chips=');
	});

	it('the attention keys of the sidebar are read', () => {
		for (const key of ['call', 'parts', 'longDiagnostics', 'longReady'] as const) {
			expect(viewFromSearch(new URLSearchParams(`attention=${key}`)).attention).toBe(key);
		}
	});
});

describe('changing the view', () => {
	it('a chip drops attention and the statuses of «Фильтры», keeps the rest', () => {
		const v: OrdersView = {
			...defaultView(),
			attention: 'call',
			query: 'a52',
			filters: { brand: 'Asus', statusIds: [3] }
		};
		expect(withChips(v, ['3'])).toEqual({
			...v,
			chips: ['3'],
			attention: null,
			filters: { brand: 'Asus' }
		});
	});

	it('the pressed chips: none while attention or statuses of «Фильтры» decide', () => {
		expect(displayedChips(defaultView())).toEqual(['work']);
		expect(displayedChips({ ...defaultView(), attention: 'parts' })).toEqual([]);
		expect(displayedChips({ ...defaultView(), filters: { statusIds: [3] } })).toEqual([]);
	});

	it('reset keeps only the sort', () => {
		const v: OrdersView = {
			chips: ['3'],
			attention: 'call',
			query: 'a52',
			clientId: 3,
			sort: { columnId: 'device', descending: false },
			filters: { brand: 'Asus' }
		};
		expect(resetFilters(v)).toEqual({ ...defaultView(), sort: v.sort });
		expect(hasFilters(v)).toBe(true);
		expect(hasFilters(resetFilters(v))).toBe(false);
		expect(hasFilters({ ...defaultView(), chips: ['all'] })).toBe(true);
		expect(hasFilters({ ...defaultView(), sort: v.sort })).toBe(false);
	});
});

describe('applied filters', () => {
	const ctx = {
		statusName: (id: number) => STATUSES.find((s) => s.id === id)?.name,
		pointName: (id: number) => (id === 1 ? 'Центр' : undefined),
		clientName: 'Тестова Анна'
	};

	it('each chosen filter is a chip; removing one leaves the others', () => {
		const v: OrdersView = {
			...defaultView(),
			clientId: 3,
			filters: {
				deviceType: 'Ноутбук',
				brand: 'SAMSUNG',
				master: 'Антон',
				pointId: 1,
				statusIds: [2, 3],
				received: ['2026-11-01', '2026-11-15'],
				issued: [null, '2026-11-16'],
				needCall: true,
				needParts: false
			}
		};
		const chips = appliedFilters(v, ctx);
		expect(chips.map((c) => c.label)).toEqual([
			'Клиент: Тестова Анна',
			'Тип: Ноутбук',
			'Бренд: SAMSUNG',
			'Мастер: Антон',
			'Точка: Центр',
			'Статусы: Диагностика, Готов',
			'Принят: 01.11.2026 – 15.11.2026',
			'Выдан: по 16.11.2026',
			'Позвонить',
			'Заказать запчасть: нет'
		]);
		let rest = v;
		for (const c of chips) rest = withoutFilter(rest, c.id);
		expect(rest).toEqual(defaultView());
		const noBrand = withoutFilter(v, chips.find((c) => c.label.startsWith('Бренд'))!.id);
		expect(noBrand.filters.brand).toBeUndefined();
		expect(noBrand.filters.master).toBe('Антон');
	});

	it('one status, a period from a day, a client still loading', () => {
		const v: OrdersView = {
			...defaultView(),
			clientId: 3,
			filters: { statusIds: [3], received: ['2026-11-01', null] }
		};
		expect(appliedFilters(v, { ...ctx, clientName: null }).map((c) => c.label)).toEqual([
			'Клиент: …',
			'Статус: Готов',
			'Принят: с 01.11.2026'
		]);
	});
});

describe('quick filters', () => {
	const WITH_AGREED: Status[] = [
		...STATUSES.slice(0, 2),
		{ id: 5, name: 'Согласовано', kind: 'open', color: 'indigo', sort_order: 2, archived: false },
		{ id: 6, name: 'Старый', kind: 'open', color: 'gray', sort_order: 3, archived: true },
		...STATUSES.slice(2)
	];

	it('«В работе», statuses in order with counts, «Выданы», «Все»', () => {
		const { main, more } = quickFilterItems(WITH_AGREED, summaryOf({ 1: 4, 2: 3, 5: 1, 3: 10 }));
		expect(main).toEqual([
			{ id: 'work', label: 'В работе', count: 8 },
			{ id: '1', label: 'Принят', count: 4, color: 'gray' },
			{ id: '2', label: 'Диагностика', count: 3, color: 'blue' },
			{ id: '5', label: 'Согласовано', count: 1, color: 'indigo' },
			{ id: '3', label: 'Готов', count: 10, color: 'green' },
			{ id: 'issued', label: 'Выданы' },
			{ id: 'all', label: 'Все' }
		]);
		// Issued statuses are «Выданы»; each of them can still be picked.
		expect(more).toEqual([{ id: '4', label: 'Выдан', count: 0, color: 'slate' }]);
	});

	it('statuses without orders go to more', () => {
		const { main, more } = quickFilterItems(WITH_AGREED, summaryOf({ 1: 4, 2: 3, 3: 10, 4: 50 }));
		expect(main.map((i) => i.id)).toEqual(['work', '1', '2', '3', 'issued', 'all']);
		expect(more).toEqual([
			{ id: '5', label: 'Согласовано', count: 0, color: 'indigo' },
			{ id: '4', label: 'Выдан', count: 50, color: 'slate' }
		]);
	});

	it('before the summary comes: every status in work, no counts', () => {
		const { main, more } = quickFilterItems(WITH_AGREED, null);
		expect(main.map((i) => [i.id, i.count])).toEqual([
			['work', undefined],
			['1', undefined],
			['2', undefined],
			['5', undefined],
			['3', undefined],
			['issued', undefined],
			['all', undefined]
		]);
		expect(more.map((i) => i.id)).toEqual(['4']);
	});
});

describe('cells', () => {
	it('device is brand and model, or the type when both are empty', () => {
		expect(deviceOf(item(1, { brand: 'Samsung', model: 'Galaxy A52' }))).toBe('Samsung Galaxy A52');
		expect(deviceOf(item(1, { brand: '', model: '', device_type: 'Телефон' }))).toBe('Телефон');
	});

	it('estimate is a range, one sum, or empty', () => {
		expect(estimateText(item(1, { estimate_from: 600000, estimate_to: 700000 }))).toBe(
			'6\u202f000–7\u202f000'
		);
		expect(estimateText(item(1, { estimate_from: 600000, estimate_to: 600000 }))).toBe(
			'6\u202f000'
		);
		expect(estimateText(item(1, { estimate_from: 600000 }))).toBe('6\u202f000');
		expect(estimateText(item(1))).toBe('');
	});

	it('to pay is the final price minus prepayment and discount', () => {
		expect(toPay({ final_price: 650000, prepayment: 200000, discount: 0 })).toBe(450000);
		expect(toPay({ final_price: 650000, prepayment: 200000, discount: 50000 })).toBe(400000);
		expect(toPay({ final_price: 100000, prepayment: 200000, discount: 0 })).toBe(-100000);
		expect(toPay({ final_price: null, prepayment: 200000, discount: 0 })).toBeNull();
	});

	it('long diagnostics: a diagnostics order received more than the days ago', () => {
		const now = { year: 2026, month: 11, day: 16, hour: 12, minute: 0, second: 0 };
		const o = (received_at: string) => item(1, { received_at });
		expect(isOverdueDiagnostics(o('2026-11-11T11:59:59'), 'diagnostics', now, 5)).toBe(true);
		expect(isOverdueDiagnostics(o('2026-11-11T12:00:00'), 'diagnostics', now, 5)).toBe(false);
		expect(isOverdueDiagnostics(o('2026-11-01T10:00:00'), 'open', now, 5)).toBe(false);
		expect(isOverdueDiagnostics(o('не дата'), 'diagnostics', now, 5)).toBe(false);
	});
});

describe('saved views', () => {
	it('reads names and addresses, drops anything else', () => {
		expect(parseSavedViews(null)).toEqual([]);
		expect(parseSavedViews('{')).toEqual([]);
		expect(parseSavedViews('{"name":"x"}')).toEqual([]);
		expect(
			parseSavedViews(
				JSON.stringify([
					{ name: 'Готовые Samsung', search: 'chips=3&brand=Samsung' },
					{ name: '', search: 'chips=3' },
					{ name: 'Без адреса' },
					'строка',
					{ name: 'Все', search: '' }
				])
			)
		).toEqual([
			{ name: 'Готовые Samsung', search: 'chips=3&brand=Samsung' },
			{ name: 'Все', search: '' }
		]);
	});
});
