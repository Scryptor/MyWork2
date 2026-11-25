// Columns of the stock table (decision 18). The cells are snippets of the page; here are their ids,
// headers, widths and what the server can sort by.
import type { StockItem } from '$api/StockItem';
import type { StockSortField } from '$api/StockSortField';
import type { Snippet } from 'svelte';
import type { Column } from '#lib/ui/table/types.js';

export const STOCK_COLUMN_IDS = [
	'id',
	'name',
	'quantity',
	'place',
	'price',
	'restock',
	'part_type',
	'device_type',
	'brand',
	'model',
	'color'
] as const;

export type StockColumnId = (typeof STOCK_COLUMN_IDS)[number];

/** The server's sort field of each sortable column. */
export const STOCK_SORT_FIELD: Record<string, StockSortField> = {
	id: 'id',
	name: 'name',
	quantity: 'quantity',
	price: 'price'
};

type Spec = Omit<Column<StockItem>, 'cell' | 'sortable'>;

const SPECS: Record<StockColumnId, Spec> = {
	id: { id: 'id', header: 'Арт.', width: 80, align: 'end' },
	// Text columns share the spare width down to their minimum.
	name: { id: 'name', header: 'Наименование', width: 320, minWidth: 160, flex: true },
	quantity: { id: 'quantity', header: 'Кол-во', width: 130, align: 'end' },
	// The codes of the cells it lies in; the server does not sort by them.
	place: { id: 'place', header: 'Место', width: 140 },
	price: { id: 'price', header: 'Цена', width: 120, align: 'end' },
	restock: { id: 'restock', header: 'Докупить при', width: 130, align: 'end' },
	part_type: {
		id: 'part_type',
		header: 'Тип запчасти',
		width: 160,
		minWidth: 110,
		flex: true,
		hiddenByDefault: true
	},
	device_type: {
		id: 'device_type',
		header: 'Тип устройства',
		width: 160,
		minWidth: 110,
		flex: true,
		hiddenByDefault: true
	},
	brand: { id: 'brand', header: 'Бренд', width: 140, hiddenByDefault: true },
	model: { id: 'model', header: 'Модель', width: 160, hiddenByDefault: true },
	color: { id: 'color', header: 'Цвет', width: 120, hiddenByDefault: true }
};

export interface StockColumnsContext {
	cells: Record<StockColumnId, Snippet<[StockItem]>>;
}

/** The columns in their default order; the table keeps the user's order, widths and hidden ones. */
export function stockColumns(ctx: StockColumnsContext): Column<StockItem>[] {
	return STOCK_COLUMN_IDS.map((id) => ({
		...SPECS[id],
		sortable: Object.hasOwn(STOCK_SORT_FIELD, id),
		cell: ctx.cells[id]
	}));
}
