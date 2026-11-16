import type { OrderListItem } from '$api/OrderListItem';
import type { Snippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { ORDER_COLUMN_IDS, SORT_FIELD, orderColumns, type OrderColumnId } from './columns.js';

const cell = (() => {}) as unknown as Snippet<[OrderListItem]>;
const cells = Object.fromEntries(ORDER_COLUMN_IDS.map((id) => [id, cell])) as Record<
	OrderColumnId,
	Snippet<[OrderListItem]>
>;

describe('orderColumns', () => {
	const columns = orderColumns({ cells });

	it('shows the main columns and hides the rest', () => {
		expect(columns.filter((c) => !c.hiddenByDefault).map((c) => c.id)).toEqual([
			'id',
			'received',
			'client',
			'device',
			'malfunction',
			'status',
			'master',
			'amount',
			'flags'
		]);
		expect(columns.filter((c) => c.hiddenByDefault).map((c) => c.id)).toEqual([
			'phone',
			'type',
			'serial',
			'point',
			'issued',
			'prepayment',
			'barcode'
		]);
	});

	it('sorts by the columns the server sorts by, and only those', () => {
		expect(SORT_FIELD).toEqual({
			id: 'id',
			received: 'received_at',
			client: 'client_name',
			device: 'brand',
			type: 'device_type',
			status: 'status',
			master: 'master',
			amount: 'final_price',
			issued: 'issued_at'
		});
		for (const c of columns) expect(!!c.sortable, c.id).toBe(c.id in SORT_FIELD);
	});

	it('headers are in Russian, every column has its cell', () => {
		expect(columns.map((c) => c.header)).toEqual([
			'№',
			'Принят',
			'Клиент',
			'Устройство',
			'Неисправность',
			'Статус',
			'Мастер',
			'Сумма',
			'Отметки',
			'Телефон',
			'Тип',
			'Серийный/IMEI',
			'Точка',
			'Выдан',
			'Предоплата',
			'Штрихкод'
		]);
		for (const c of columns) expect(c.cell).toBe(cell);
		expect(columns.find((c) => c.id === 'amount')?.align).toBe('end');
	});
});
