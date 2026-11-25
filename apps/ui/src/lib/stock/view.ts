// The view of the stock list (decision 18): the filter text, the sort, «Докупить». It lives in the
// address like the clients' view; beside it `item` names the position open in the panel.
import type { StockListQuery } from '$api/StockListQuery';
import type { SortState } from '#lib/ui/table/types.js';
import type { StockCellRef } from '$api/StockCellRef';
import { STOCK_SORT_FIELD } from './columns.js';
import { NO_POINT, parseCellParam } from './map/racks.js';

export interface StockView {
	query: string;
	sort: SortState;
	/** «Докупить»: only the positions whose quantity is below their threshold. */
	restock: boolean;
	/** `text=1`: the text is only text — a code of a rack or a cell in it is not taken as a place. */
	asText: boolean;
	/** `view=map`: the map of the racks instead of the list. */
	map: boolean;
	/** `point=`: the service point whose racks the map shows; `'none'` — the racks without one. */
	point: number | typeof NO_POINT | null;
}

/** By name, A to Z. */
export const DEFAULT_STOCK_SORT: SortState = { columnId: 'name', descending: false };

export function defaultStockView(): StockView {
	return {
		query: '',
		sort: { ...DEFAULT_STOCK_SORT },
		restock: false,
		asText: false,
		map: false,
		point: null
	};
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
	v.asText = p.get('text') === '1';
	v.map = p.get('view') === 'map';
	const point = p.get('point');
	v.point = point === NO_POINT ? NO_POINT : positive(point);
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
	if (v.asText) out.push(['text', '1']);
	if (v.map) out.push(['view', 'map']);
	if (v.point !== null) out.push(['point', String(v.point)]);
	return out.map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join('&');
}

/** The position open in the panel (`?item=<id>`). */
export function openStockItemId(p: Pick<URLSearchParams, 'get'>): number | null {
	return positive(p.get('item'));
}

/** The cell open in the panel of the map (`?cell=3.2.4`). */
export function openStockCell(p: Pick<URLSearchParams, 'get'>): StockCellRef | null {
	return parseCellParam(p.get('cell'));
}

/** The address of the view with this position and this cell (`3.2.4`) open in their panels (or none). */
export function stockSearch(
	v: StockView,
	itemId: number | null,
	cell: string | null = null
): string {
	const parts = [stockViewToSearch(v)];
	if (itemId !== null) parts.push(`item=${itemId}`);
	if (cell !== null) parts.push(`cell=${cell}`);
	return parts.filter((s) => s !== '').join('&');
}

/** Where a position opens from elsewhere: «Склад» with its panel; for `resolve`. */
export function stockItemHref(id: number): `stock?item=${number}` {
	return `stock?item=${id}`;
}

/** A new text from the filter field: it is searched as it is read, so «Искать как текст» is let go. */
export function withStockQuery(v: StockView, query: string): StockView {
	return { ...v, query, asText: false };
}

/** The server's query of a view; paging is added by the list. */
export function stockViewToQuery(v: StockView): StockListQuery {
	return {
		query: v.query.trim() || undefined,
		restock: v.restock ? true : undefined,
		as_text: v.asText ? true : undefined,
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
