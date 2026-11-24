// The view of the stock list (decision 18): the filter text, the sort, «Докупить». It lives in the
// address like the clients' view; beside it `item` names the position open in the panel.
import type { StockListQuery } from '$api/StockListQuery';
import type { SortState } from '#lib/ui/table/types.js';
import { STOCK_SORT_FIELD } from './columns.js';

export interface StockView {
	query: string;
	sort: SortState;
	/** «Докупить»: only the positions whose quantity is below their threshold. */
	restock: boolean;
}

/** By name, A to Z. */
export const DEFAULT_STOCK_SORT: SortState = { columnId: 'name', descending: false };

export function defaultStockView(): StockView {
	return { query: '', sort: { ...DEFAULT_STOCK_SORT }, restock: false };
}

const sortable = (columnId: string) => Object.hasOwn(STOCK_SORT_FIELD, columnId);
const positive = (s: string | null): number | null =>
	s !== null && /^[1-9]\d*$/.test(s) ? Number(s) : null;

export function stockViewFromSearch(p: Pick<URLSearchParams, 'get'>): StockView {
	const v = defaultStockView();
	v.query = p.get('q') ?? '';
	const sort = p.get('sort');
	if (sort !== null && sortable(sort)) v.sort.columnId = sort;
	const desc = p.get('desc');
	if (desc === '1' || desc === '0') v.sort.descending = desc === '1';
	v.restock = p.get('restock') === '1';
	return v;
}

/** The search part of the address without `?`; the default view is ''. */
export function stockViewToSearch(v: StockView): string {
	const out: [string, string][] = [];
	if (v.query !== '') out.push(['q', v.query]);
	const s = v.sort;
	if (
		s.columnId !== DEFAULT_STOCK_SORT.columnId ||
		s.descending !== DEFAULT_STOCK_SORT.descending
	) {
		out.push(['sort', s.columnId], ['desc', s.descending ? '1' : '0']);
	}
	if (v.restock) out.push(['restock', '1']);
	return out.map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join('&');
}

/** The position open in the panel (`?item=<id>`). */
export function openStockItemId(p: Pick<URLSearchParams, 'get'>): number | null {
	return positive(p.get('item'));
}

/** The address of the list with this position open in the panel (or none). */
export function stockSearch(v: StockView, itemId: number | null): string {
	const view = stockViewToSearch(v);
	if (itemId === null) return view;
	return view ? `${view}&item=${itemId}` : `item=${itemId}`;
}

/** Where a position opens from elsewhere: «Склад» with its panel; for `resolve`. */
export function stockItemHref(id: number): `stock?item=${number}` {
	return `stock?item=${id}`;
}

/** The server's query of a view; paging is added by the list. */
export function stockViewToQuery(v: StockView): StockListQuery {
	return {
		query: v.query.trim() || undefined,
		restock: v.restock ? true : undefined,
		sort: sortable(v.sort.columnId) ? STOCK_SORT_FIELD[v.sort.columnId] : 'name',
		descending: v.sort.descending
	};
}

// Chips «Все» / «Докупить»: exactly one of them is pressed.

export type StockChip = 'all' | 'restock';

export function shownStockChips(v: StockView): StockChip[] {
	return [v.restock ? 'restock' : 'all'];
}

/** The chips after a click: the one that was not pressed is picked; taking the pressed one away changes nothing. */
export function withStockChips(v: StockView, chips: readonly string[]): StockView {
	const now: StockChip = v.restock ? 'restock' : 'all';
	const picked = chips.find((c) => c !== now) ?? (chips.includes(now) ? now : null);
	if (picked !== 'all' && picked !== 'restock') return v;
	const restock = picked === 'restock';
	return restock === v.restock ? v : { ...v, restock };
}
