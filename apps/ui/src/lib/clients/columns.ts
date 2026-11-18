// Columns of the clients table (spec 6.6). The cells are snippets of the page; here are their ids,
// headers, widths and what the server can sort by.
import type { ClientListItem } from '$api/ClientListItem';
import type { ClientPhone } from '$api/ClientPhone';
import type { ClientSortField } from '$api/ClientSortField';
import type { Snippet } from 'svelte';
import { formatPhone } from '#lib/format/phone.js';
import type { Column } from '#lib/ui/table/types.js';

export const CLIENT_COLUMN_IDS = ['name', 'phones', 'orders', 'last', 'source', 'flags'] as const;

export type ClientColumnId = (typeof CLIENT_COLUMN_IDS)[number];

/** The server's sort field of each sortable column. */
export const CLIENT_SORT_FIELD: Record<string, ClientSortField> = {
	name: 'name',
	orders: 'orders_count',
	last: 'last_order'
};

type Spec = Omit<Column<ClientListItem>, 'cell' | 'sortable'>;

const SPECS: Record<ClientColumnId, Spec> = {
	// Text columns share the spare width down to their minimum (decision 51).
	name: { id: 'name', header: 'ФИО', width: 260, minWidth: 140, flex: true },
	phones: { id: 'phones', header: 'Телефоны', width: 190 },
	orders: { id: 'orders', header: 'Заказы', width: 96, align: 'end' },
	last: { id: 'last', header: 'Последний заказ', width: 176 },
	source: { id: 'source', header: 'Откуда узнали', width: 160, minWidth: 110, flex: true },
	flags: { id: 'flags', header: 'Отметки', width: 100 }
};

export interface ClientColumnsContext {
	cells: Record<ClientColumnId, Snippet<[ClientListItem]>>;
	/** False while the server keeps its own order («Похожие»): no header sorts. */
	sortable?: boolean;
}

/** The columns in their default order; the table keeps the user's order, widths and hidden ones. */
export function clientColumns(ctx: ClientColumnsContext): Column<ClientListItem>[] {
	const sorts = ctx.sortable ?? true;
	return CLIENT_COLUMN_IDS.map((id) => ({
		...SPECS[id],
		sortable: sorts && Object.hasOwn(CLIENT_SORT_FIELD, id),
		cell: ctx.cells[id]
	}));
}

/** «Телефоны»: the main number and «＋N» for the others. */
export function phonesText(phones: readonly ClientPhone[]): string {
	const main = phones[0];
	if (!main) return '';
	const more = phones.length - 1;
	return more > 0 ? `${formatPhone(main.phone)} ＋${more}` : formatPhone(main.phone);
}
