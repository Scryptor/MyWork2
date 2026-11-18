// The view of the clients list (spec 6.6): the list filter, «Все» / «Похожие», the sort. It lives in the
// address like the orders' view; beside it `client` names the client open in the panel.
import type { ClientListQuery } from '$api/ClientListQuery';
import type { SortState } from '#lib/ui/table/types.js';
import { CLIENT_SORT_FIELD } from './columns.js';

export interface ClientsView {
	query: string;
	sort: SortState;
	/** «Похожие»: only clients that have a twin by name or phone, in the server's order (by name). */
	duplicates: boolean;
}

/** «Последний заказ», newest first: the clients one deals with now are on top. */
export const DEFAULT_CLIENTS_SORT: SortState = { columnId: 'last', descending: true };

export function defaultClientsView(): ClientsView {
	return { query: '', sort: { ...DEFAULT_CLIENTS_SORT }, duplicates: false };
}

const sortable = (columnId: string) => Object.hasOwn(CLIENT_SORT_FIELD, columnId);
const positive = (s: string | null): number | null =>
	s !== null && /^[1-9]\d*$/.test(s) ? Number(s) : null;

export function clientsViewFromSearch(p: Pick<URLSearchParams, 'get'>): ClientsView {
	const v = defaultClientsView();
	v.query = p.get('q') ?? '';
	const sort = p.get('sort');
	if (sort !== null && sortable(sort)) v.sort.columnId = sort;
	const desc = p.get('desc');
	if (desc === '1' || desc === '0') v.sort.descending = desc === '1';
	v.duplicates = p.get('dup') === '1';
	return v;
}

/** The search part of the address without `?`; the default view is ''. */
export function clientsViewToSearch(v: ClientsView): string {
	const out: [string, string][] = [];
	if (v.query !== '') out.push(['q', v.query]);
	const s = v.sort;
	if (
		s.columnId !== DEFAULT_CLIENTS_SORT.columnId ||
		s.descending !== DEFAULT_CLIENTS_SORT.descending
	) {
		out.push(['sort', s.columnId], ['desc', s.descending ? '1' : '0']);
	}
	if (v.duplicates) out.push(['dup', '1']);
	return out.map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join('&');
}

/** The client open in the panel (`?client=<id>`). */
export function openClientId(p: Pick<URLSearchParams, 'get'>): number | null {
	return positive(p.get('client'));
}

/** The address of the list with this client open in the panel (or none). */
export function clientsSearch(v: ClientsView, clientId: number | null): string {
	const view = clientsViewToSearch(v);
	if (clientId === null) return view;
	return view ? `${view}&client=${clientId}` : `client=${clientId}`;
}

/** Where a client opens from elsewhere (Ctrl K, an order): «Клиенты» with their panel; for `resolve`. */
export function clientHref(id: number): `clients?client=${number}` {
	return `clients?client=${id}`;
}

/** The server's query of a view; paging is added by the list. */
export function clientsViewToQuery(v: ClientsView): ClientListQuery {
	const query = v.query.trim() || undefined;
	if (v.duplicates) return { query, duplicates: true };
	return {
		query,
		sort: sortable(v.sort.columnId) ? CLIENT_SORT_FIELD[v.sort.columnId] : 'last_order',
		descending: v.sort.descending,
		duplicates: undefined
	};
}

// Chips «Все» / «Похожие»: exactly one of them is pressed.

export type ClientChip = 'all' | 'similar';

export function shownChips(v: ClientsView): ClientChip[] {
	return [v.duplicates ? 'similar' : 'all'];
}

/**
 * The chips after a click: a plain click picks one; Ctrl+click adds the other one, which then is the one
 * picked; taking the pressed one away changes nothing.
 */
export function withChips(v: ClientsView, chips: readonly string[]): ClientsView {
	const now: ClientChip = v.duplicates ? 'similar' : 'all';
	const picked = chips.find((c) => c !== now) ?? (chips.includes(now) ? now : null);
	if (picked !== 'all' && picked !== 'similar') return v;
	const duplicates = picked === 'similar';
	return duplicates === v.duplicates ? v : { ...v, duplicates };
}
