// Columns of the orders table (spec 6.1). The cells are snippets of the page; here are their ids,
// headers, widths, what is shown by default and what the server can sort by.
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderSortField } from '$api/OrderSortField';
import type { Snippet } from 'svelte';
import type { Column } from '#lib/ui/table/types.js';

export const ORDER_COLUMN_IDS = [
	'id',
	'received',
	'client',
	'device',
	'malfunction',
	'status',
	'master',
	'amount',
	'flags',
	'phone',
	'type',
	'serial',
	'point',
	'issued',
	'prepayment',
	'barcode'
] as const;

export type OrderColumnId = (typeof ORDER_COLUMN_IDS)[number];

/** The server's sort field of each sortable column; the other columns are not sorted. */
export const SORT_FIELD: Record<string, OrderSortField> = {
	id: 'id',
	received: 'received_at',
	client: 'client_name',
	device: 'brand',
	type: 'device_type',
	status: 'status',
	master: 'master',
	amount: 'final_price',
	issued: 'issued_at'
};

type Spec = Omit<Column<OrderListItem>, 'cell' | 'sortable'>;

const SPECS: Record<OrderColumnId, Spec> = {
	id: { id: 'id', header: '№', width: 84 },
	received: { id: 'received', header: 'Принят', width: 132 },
	client: { id: 'client', header: 'Клиент', width: 200, minWidth: 120 },
	device: { id: 'device', header: 'Устройство', width: 180 },
	malfunction: { id: 'malfunction', header: 'Неисправность', width: 200 },
	status: { id: 'status', header: 'Статус', width: 232 },
	master: { id: 'master', header: 'Мастер', width: 110 },
	amount: { id: 'amount', header: 'Сумма', width: 140, align: 'end' },
	flags: { id: 'flags', header: 'Отметки', width: 96 },
	phone: { id: 'phone', header: 'Телефон', width: 150, hiddenByDefault: true },
	type: { id: 'type', header: 'Тип', width: 120, hiddenByDefault: true },
	serial: { id: 'serial', header: 'Серийный/IMEI', width: 170, hiddenByDefault: true },
	point: { id: 'point', header: 'Точка', width: 110, hiddenByDefault: true },
	issued: { id: 'issued', header: 'Выдан', width: 132, hiddenByDefault: true },
	prepayment: {
		id: 'prepayment',
		header: 'Предоплата',
		width: 120,
		align: 'end',
		hiddenByDefault: true
	},
	barcode: { id: 'barcode', header: 'Штрихкод', width: 140, hiddenByDefault: true }
};

export interface OrderColumnsContext {
	cells: Record<OrderColumnId, Snippet<[OrderListItem]>>;
}

/** The columns in their default order; the table keeps the user's order, widths and hidden ones. */
export function orderColumns(ctx: OrderColumnsContext): Column<OrderListItem>[] {
	return ORDER_COLUMN_IDS.map((id) => ({
		...SPECS[id],
		sortable: Object.hasOwn(SORT_FIELD, id),
		cell: ctx.cells[id]
	}));
}
