// The view of the orders list (spec 6.1): quick filters or an item of «Требуют внимания», the list
// filter, «Фильтры ▾», the client, the sort. The view lives in the address, so back and forward and
// «Мои виды» work; here it is read, written and turned into the server's query.
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderSummary } from '$api/OrderSummary';
import type { Status } from '$api/Status';
import type { StatusKind } from '$api/StatusKind';
import type { WarrantyTerm } from '$api/WarrantyTerm';
import { formatDate, parseLocalDateTime, type LocalDateTime } from '#lib/format/date.js';
import { formatMoney } from '#lib/format/money.js';
import { ru } from '#lib/i18n/ru.js';
import type { SortState } from '#lib/ui/table/types.js';
import { SORT_FIELD } from './columns.js';

export type AttentionKey = 'call' | 'parts' | 'longDiagnostics' | 'longReady';
export const ATTENTION_KEYS: readonly AttentionKey[] = [
	'call',
	'parts',
	'longDiagnostics',
	'longReady'
];

/** Days `YYYY-MM-DD`, both ends included; null is an open end. */
export type Period = [string | null, string | null];

export interface OrderFilters {
	deviceType?: string;
	brand?: string;
	/** A master user; the filter of a saved view of the old kind has the name in `master`. */
	masterId?: number;
	master?: string;
	pointId?: number;
	statusIds?: number[];
	received?: Period;
	issued?: Period;
	needCall?: boolean;
	needParts?: boolean;
}

export interface OrdersView {
	/** Quick filters: `work`, `issued`, `all` or status ids; several are a union. */
	chips: string[];
	attention: AttentionKey | null;
	query: string;
	filters: OrderFilters;
	sort: SortState;
	clientId: number | null;
}

export const DEFAULT_SORT: SortState = { columnId: 'received', descending: true };
const DEFAULT_CHIPS = ['work'];

/** «В работе», sorted by «Принят», newest first. */
export function defaultView(): OrdersView {
	return {
		chips: [...DEFAULT_CHIPS],
		attention: null,
		query: '',
		filters: {},
		sort: { ...DEFAULT_SORT },
		clientId: null
	};
}

const IN_WORK: readonly StatusKind[] = ['open', 'diagnostics'];

/** «В работе»: statuses of kinds `open` and `diagnostics`, archived ones too. */
export function workStatusIds(statuses: readonly Status[]): number[] {
	return statuses.filter((s) => IN_WORK.includes(s.kind)).map((s) => s.id);
}

const isStatusChip = (c: string) => /^[1-9]\d*$/.test(c);
const isChip = (c: string) => c === 'work' || c === 'issued' || c === 'all' || isStatusChip(c);

function chipsQuery(
	chips: readonly string[],
	statuses: readonly Status[]
): Pick<OrderListQuery, 'status_ids' | 'issued'> {
	if (chips.length === 0 || chips.includes('all')) return {};
	if (chips.length === 1 && chips[0] === 'issued') return { issued: 'issued' };
	const ids: number[] = [];
	for (const c of chips) {
		const add =
			c === 'work'
				? workStatusIds(statuses)
				: c === 'issued'
					? statuses.filter((s) => s.kind === 'issued').map((s) => s.id)
					: isStatusChip(c)
						? [Number(c)]
						: [];
		for (const id of add) if (!ids.includes(id)) ids.push(id);
	}
	return ids.length > 0 ? { status_ids: ids } : {};
}

const sortable = (columnId: string) => Object.hasOwn(SORT_FIELD, columnId);

const dayStart = (d: string | null | undefined) => (d ? `${d}T00:00:00` : undefined);
const dayEnd = (d: string | null | undefined) => (d ? `${d}T23:59:59` : undefined);

/**
 * The server's query of a view. «Требуют внимания» takes the place of the quick filters, and so do the
 * statuses of «Фильтры». The point of the top bar and the point of «Фильтры» are one field: the
 * filter wins. Paging is added by the list.
 */
export function viewToQuery(
	v: OrdersView,
	statuses: readonly Status[],
	pointId: number | null
): OrderListQuery {
	const f = v.filters;
	const q: OrderListQuery = {
		query: v.query.trim() || undefined,
		status_ids: undefined,
		issued: undefined,
		need_call: f.needCall,
		need_parts: f.needParts,
		overdue_diagnostics: undefined,
		stale_ready: undefined,
		service_point_id: f.pointId ?? pointId ?? undefined,
		client_id: v.clientId ?? undefined,
		device_type: f.deviceType || undefined,
		brand: f.brand || undefined,
		master: f.master || undefined,
		master_user_id: f.masterId,
		received_from: dayStart(f.received?.[0]),
		received_to: dayEnd(f.received?.[1]),
		issued_from: dayStart(f.issued?.[0]),
		issued_to: dayEnd(f.issued?.[1]),
		sort: sortable(v.sort.columnId) ? SORT_FIELD[v.sort.columnId] : 'received_at',
		descending: v.sort.descending
	};
	if (v.attention === 'call') Object.assign(q, { need_call: true, issued: 'in_service' });
	else if (v.attention === 'parts') Object.assign(q, { need_parts: true, issued: 'in_service' });
	else if (v.attention === 'longDiagnostics') q.overdue_diagnostics = true;
	else if (v.attention === 'longReady') q.stale_ready = true;
	else if (!f.statusIds?.length) Object.assign(q, chipsQuery(v.chips, statuses));
	if (f.statusIds?.length) q.status_ids = [...f.statusIds];
	return q;
}

// The address: short fixed names, defaults left out, lists comma-separated.

const DAY = /^\d{4}-\d{2}-\d{2}$/;
const isDay = (s: string | null): s is string =>
	s !== null && DAY.test(s) && parseLocalDateTime(s) !== null;
const positive = (s: string | null): number | undefined =>
	s !== null && /^[1-9]\d*$/.test(s) ? Number(s) : undefined;
const flag = (s: string | null): boolean | undefined =>
	s === '1' ? true : s === '0' ? false : undefined;
const text = (s: string | null): string | undefined => (s ? s : undefined);

function period(from: string | null, to: string | null): Period | undefined {
	const p: Period = [isDay(from) ? from : null, isDay(to) ? to : null];
	return p[0] === null && p[1] === null ? undefined : p;
}

/** Only the keys that are set: an empty filter is `{}`. */
function compact(f: OrderFilters): OrderFilters {
	return Object.fromEntries(
		Object.entries(f).filter(([, value]) => value !== undefined)
	) as OrderFilters;
}

export function viewFromSearch(p: Pick<URLSearchParams, 'get'>): OrdersView {
	const v = defaultView();
	const chips = p.get('chips');
	if (chips === '') v.chips = [];
	else if (chips !== null) {
		const list = [...new Set(chips.split(',').filter(isChip))];
		if (list.length > 0) v.chips = list;
	}
	v.attention = ATTENTION_KEYS.find((k) => k === p.get('attention')) ?? null;
	v.query = p.get('q') ?? '';
	v.clientId = positive(p.get('client')) ?? null;
	const sort = p.get('sort');
	if (sort !== null && sortable(sort)) v.sort.columnId = sort;
	v.sort.descending = flag(p.get('desc')) ?? DEFAULT_SORT.descending;
	const statusIds = (p.get('st') ?? '').split(',').flatMap((s) => positive(s) ?? []);
	v.filters = compact({
		deviceType: text(p.get('type')),
		brand: text(p.get('brand')),
		master: text(p.get('master')),
		masterId: positive(p.get('master_id')),
		pointId: positive(p.get('point')),
		statusIds: statusIds.length > 0 ? [...new Set(statusIds)] : undefined,
		received: period(p.get('rf'), p.get('rt')),
		issued: period(p.get('if'), p.get('it')),
		needCall: flag(p.get('call')),
		needParts: flag(p.get('parts'))
	});
	return v;
}

/** The search part of the address without `?`; the default view is ''. Commas stay readable. */
export function viewToSearch(v: OrdersView): string {
	const out: [string, string][] = [];
	const put = (key: string, value: string | number | undefined | null) => {
		if (value !== undefined && value !== null) out.push([key, String(value)]);
	};
	const sameChips =
		v.chips.length === DEFAULT_CHIPS.length && v.chips.every((c, i) => c === DEFAULT_CHIPS[i]);
	if (!sameChips) put('chips', v.chips.join(','));
	put('attention', v.attention);
	if (v.query !== '') put('q', v.query);
	put('client', v.clientId);
	if (v.sort.columnId !== DEFAULT_SORT.columnId || v.sort.descending !== DEFAULT_SORT.descending) {
		put('sort', v.sort.columnId);
		put('desc', v.sort.descending ? 1 : 0);
	}
	const f = v.filters;
	put('type', f.deviceType || undefined);
	put('brand', f.brand || undefined);
	put('master', f.master || undefined);
	put('master_id', f.masterId);
	put('point', f.pointId);
	if (f.statusIds?.length) put('st', f.statusIds.join(','));
	put('rf', f.received?.[0]);
	put('rt', f.received?.[1]);
	put('if', f.issued?.[0]);
	put('it', f.issued?.[1]);
	if (f.needCall !== undefined) put('call', f.needCall ? 1 : 0);
	if (f.needParts !== undefined) put('parts', f.needParts ? 1 : 0);
	return out
		.map(([key, value]) => `${key}=${encodeURIComponent(value).replace(/%2C/g, ',')}`)
		.join('&');
}

// Changes of the view.

/** The quick filters chosen: «Требуют внимания» and the statuses of «Фильтры» give way. */
export function withChips(v: OrdersView, chips: string[]): OrdersView {
	const filters = { ...v.filters };
	delete filters.statusIds;
	return { ...v, chips, attention: null, filters };
}

/** The chips shown pressed: none while «Требуют внимания» or the statuses of «Фильтры» decide. */
export function displayedChips(v: OrdersView): string[] {
	return v.attention !== null || v.filters.statusIds?.length ? [] : v.chips;
}

/** Anything narrower than the default view (the sort is not a filter). */
export function hasFilters(v: OrdersView): boolean {
	return viewToSearch({ ...v, sort: DEFAULT_SORT }) !== '';
}

/** «Сбросить фильтры»: the default view, in the same sort. */
export function resetFilters(v: OrdersView): OrdersView {
	return { ...defaultView(), sort: { ...v.sort } };
}

export interface AppliedFilter {
	id: keyof OrderFilters | 'client';
	label: string;
}

export interface AppliedContext {
	statusName: (id: number) => string | undefined;
	pointName: (id: number) => string | undefined;
	/** The name of a master user; before the masters are loaded it is unknown. */
	masterName?: (id: number) => string | undefined;
	/** null while the client is being loaded. */
	clientName: string | null;
}

function periodLabel(name: string, p: Period): string {
	const [from, to] = p.map((d) => (d ? formatDate(d) : null));
	if (from && to) return `${name}: ${from} – ${to}`;
	return from ? `${name}: ${ru.orders.from} ${from}` : `${name}: ${ru.orders.to} ${to ?? ''}`;
}

const flagLabel = (name: string, on: boolean) => (on ? name : `${name}: ${ru.orders.no}`);

/** The chips with «✕» under the filter row: the client and what was chosen in «Фильтры». */
export function appliedFilters(v: OrdersView, ctx: AppliedContext): AppliedFilter[] {
	const t = ru.orders;
	const f = v.filters;
	const out: AppliedFilter[] = [];
	if (v.clientId !== null)
		out.push({ id: 'client', label: `${t.client}: ${ctx.clientName ?? '…'}` });
	if (f.deviceType) out.push({ id: 'deviceType', label: `${t.deviceType}: ${f.deviceType}` });
	if (f.brand) out.push({ id: 'brand', label: `${t.brand}: ${f.brand}` });
	if (f.masterId !== undefined) {
		const name = ctx.masterName?.(f.masterId) ?? `№ ${f.masterId}`;
		out.push({ id: 'masterId', label: `${t.master}: ${name}` });
	}
	if (f.master) out.push({ id: 'master', label: `${t.master}: ${f.master}` });
	if (f.pointId !== undefined) {
		out.push({
			id: 'pointId',
			label: `${t.point}: ${ctx.pointName(f.pointId) ?? `№ ${f.pointId}`}`
		});
	}
	if (f.statusIds?.length) {
		const names = f.statusIds.map((id) => ctx.statusName(id) ?? `№ ${id}`).join(', ');
		out.push({
			id: 'statusIds',
			label: `${f.statusIds.length === 1 ? t.status : t.statuses}: ${names}`
		});
	}
	if (f.received) out.push({ id: 'received', label: periodLabel(t.received, f.received) });
	if (f.issued) out.push({ id: 'issued', label: periodLabel(t.issuedAt, f.issued) });
	if (f.needCall !== undefined) {
		out.push({ id: 'needCall', label: flagLabel(ru.flags.needCall, f.needCall) });
	}
	if (f.needParts !== undefined) {
		out.push({ id: 'needParts', label: flagLabel(ru.flags.needParts, f.needParts) });
	}
	return out;
}

export function withoutFilter(v: OrdersView, id: AppliedFilter['id']): OrdersView {
	if (id === 'client') return { ...v, clientId: null };
	const filters = { ...v.filters };
	delete filters[id];
	return { ...v, filters };
}

// Quick filters.

export interface QuickFilterItem {
	id: string;
	label: string;
	count?: number;
	color?: string;
	/** A status of the «Выдан» kind: listed in «Ещё» under its own heading. */
	issued?: boolean;
}

/**
 * Quick filters (spec 6.1): «В работе», every status that is not archived in its order with its count,
 * «Выданные» (the sum of the issued statuses), «Все». Statuses without orders and issued statuses go to `more`.
 * Before the summary comes there are no counts and nothing is hidden for being empty.
 */
export function quickFilterItems(
	statuses: readonly Status[],
	summary: OrderSummary | null
): { main: QuickFilterItem[]; more: QuickFilterItem[] } {
	const counts = new Map((summary?.by_status ?? []).map((c) => [c.status_id, c.count]));
	const countOf = (id: number) => (summary ? (counts.get(id) ?? 0) : undefined);
	const inWork = summary
		? workStatusIds(statuses).reduce((n, id) => n + (counts.get(id) ?? 0), 0)
		: undefined;
	const main: QuickFilterItem[] = [{ id: 'work', label: ru.orders.inWork, count: inWork }];
	const more: QuickFilterItem[] = [];
	const listed = statuses.filter((s) => !s.archived).sort((a, b) => a.sort_order - b.sort_order);
	for (const s of listed) {
		const entry: QuickFilterItem = {
			id: String(s.id),
			label: s.name,
			count: countOf(s.id),
			color: s.color
		};
		if (s.kind === 'issued') more.push({ ...entry, issued: true });
		else if (entry.count === 0) more.push(entry);
		else main.push(entry);
	}
	const issued = summary
		? statuses.filter((s) => s.kind === 'issued').reduce((n, s) => n + (counts.get(s.id) ?? 0), 0)
		: undefined;
	main.push(
		{ id: 'issued', label: ru.orders.issued, count: issued },
		{ id: 'all', label: ru.orders.all }
	);
	return { main, more };
}

/**
 * The «Ещё» menu: statuses without orders first, then the issued ones under their heading.
 * Empty when the only thing left would be one issued status: «Выданные» already covers it.
 */
export function moreFilterMenu(
	more: readonly QuickFilterItem[],
	selected: readonly string[]
): ({ item: QuickFilterItem } | { heading: string })[] {
	const rest = more.filter((i) => !selected.includes(i.id));
	const plain = rest.filter((i) => !i.issued);
	const issued = rest.filter((i) => i.issued);
	if (plain.length === 0 && issued.length <= 1) return [];
	return [
		...plain.map((item) => ({ item })),
		...(issued.length > 0 ? [{ heading: ru.orders.issuedByStatus }] : []),
		...issued.map((item) => ({ item }))
	];
}

// Cells.

/** «Устройство» of the table: brand and model, or the type when both are empty. */
export function deviceOf(o: Pick<OrderListItem, 'brand' | 'model' | 'device_type'>): string {
	return [o.brand, o.model].filter(Boolean).join(' ') || o.device_type;
}

/** «6 000–7 000», «6 000» or '' without an estimate. */
export function estimateText(o: Pick<OrderListItem, 'estimate_from' | 'estimate_to'>): string {
	const [from, to] = [o.estimate_from, o.estimate_to].filter((v): v is number => v !== null);
	if (from === undefined) return '';
	const money = (v: number) => formatMoney(v, { currency: null });
	return to === undefined || to === from ? money(from) : `${money(from)}–${money(to)}`;
}

/** «К оплате» (spec 6.2): final price minus prepayment and discount; null without a final price. */
export function toPay(o: { final_price: number | null; prepayment: number; discount: number }) {
	return o.final_price === null ? null : o.final_price - o.prepayment - o.discount;
}

/** «Без гарантии» by the term the server derived from the text: «0 мес» and «0 дней» alike. */
export function isNoWarranty(o: { warranty_term: WarrantyTerm | null }): boolean {
	return o.warranty_term?.value === 0;
}

/** The warranty «Выдать» starts with: the order's own, the default of the settings when it has none. */
export function issueWarranty(
	o: { warranty: string; warranty_term: WarrantyTerm | null },
	fallback: string
): string {
	return o.warranty.trim() !== '' || isNoWarranty(o) ? o.warranty : fallback;
}

const DAY_MS = 86_400_000;
const asMillis = (t: LocalDateTime) =>
	Date.UTC(t.year, t.month - 1, t.day, t.hour, t.minute, t.second);

/** The clock at «Статус»: in diagnostics and received more than `days` days before `now`. */
export function isOverdueDiagnostics(
	o: Pick<OrderListItem, 'received_at'>,
	kind: StatusKind | undefined,
	now: LocalDateTime,
	days: number
): boolean {
	if (kind !== 'diagnostics') return false;
	const t = parseLocalDateTime(o.received_at);
	return t !== null && asMillis(t) < asMillis(now) - days * DAY_MS;
}

// «Мои виды».

export interface SavedView {
	name: string;
	/** `viewToSearch` of the view. */
	search: string;
}

/** Stored views; anything that is not a named view is dropped. */
export function parseSavedViews(raw: string | null): SavedView[] {
	let data: unknown;
	try {
		data = raw === null ? [] : JSON.parse(raw);
	} catch {
		return [];
	}
	if (!Array.isArray(data)) return [];
	return data.flatMap((x: unknown) => {
		if (typeof x !== 'object' || x === null) return [];
		const { name, search } = x as Record<string, unknown>;
		return typeof name === 'string' && name.trim() !== '' && typeof search === 'string'
			? [{ name, search }]
			: [];
	});
}
