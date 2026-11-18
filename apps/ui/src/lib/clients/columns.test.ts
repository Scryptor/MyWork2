import type { ClientListItem } from '$api/ClientListItem';
import type { Snippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import {
	CLIENT_COLUMN_IDS,
	CLIENT_SORT_FIELD,
	clientColumns,
	phonesText,
	type ClientColumnId
} from './columns.js';

const cell = (() => {}) as unknown as Snippet<[ClientListItem]>;
const cells = Object.fromEntries(CLIENT_COLUMN_IDS.map((id) => [id, cell])) as Record<
	ClientColumnId,
	Snippet<[ClientListItem]>
>;

describe('clientColumns', () => {
	it('has the columns of spec 6.6 in Russian, each with its cell', () => {
		const columns = clientColumns({ cells });
		expect(columns.map((c) => c.id)).toEqual([
			'name',
			'phones',
			'orders',
			'last',
			'source',
			'flags'
		]);
		expect(columns.map((c) => c.header)).toEqual([
			'ФИО',
			'Телефоны',
			'Заказы',
			'Последний заказ',
			'Откуда узнали',
			'Отметки'
		]);
		for (const c of columns) expect(c.cell).toBe(cell);
		expect(columns.some((c) => c.hiddenByDefault)).toBe(false);
		expect(columns.find((c) => c.id === 'orders')?.align).toBe('end');
	});

	it('sorts by name, the number of orders and the last order, as the server does', () => {
		expect(CLIENT_SORT_FIELD).toEqual({ name: 'name', orders: 'orders_count', last: 'last_order' });
		for (const c of clientColumns({ cells })) {
			expect(!!c.sortable, c.id).toBe(c.id in CLIENT_SORT_FIELD);
		}
	});

	it('nothing is sorted while the server keeps its own order («Похожие»)', () => {
		expect(clientColumns({ cells, sortable: false }).some((c) => c.sortable)).toBe(false);
	});
});

describe('phonesText', () => {
	it('the main number and how many more', () => {
		expect(phonesText([])).toBe('');
		expect(phonesText([{ phone: '79211234567', note: '' }])).toBe('+7 921 123-45-67');
		expect(
			phonesText([
				{ phone: '79211234567', note: '' },
				{ phone: '123456', note: 'раб' },
				{ phone: '79001112233', note: 'жена' }
			])
		).toBe('+7 921 123-45-67 ＋2');
	});
});
