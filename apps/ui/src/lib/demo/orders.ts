// Logic of the demo screens over the fictional orders: what the server will do in plan 5 (filters,
// sorting, search, «Требуют внимания»), done here on a local list.
import type { OrderListItem } from '$api/OrderListItem';
import type { Status } from '$api/Status';
import type { StatusKind } from '$api/StatusKind';
import type { ScanResult } from '#lib/design/scanner.js';
import { dayNumber, parseLocalDateTime, type LocalDateTime } from '#lib/format/date.js';
import { formatMoney } from '#lib/format/money.js';
import { formatPhone } from '#lib/format/phone.js';
import { normalizeSearch } from '#lib/format/search.js';
import type { SortState } from '#lib/ui/table/types.js';

export interface DemoPhoto {
	id: string;
	src: string;
	alt: string;
}

export interface HistoryEntry {
	id: number;
	at: string;
	text: string;
}

/** An order of the demo screens: a list row plus what its panel shows and edits. */
export interface DemoOrder extends OrderListItem {
	color: string;
	device_password: string;
	work_done: string;
	internal_comment: string;
	discount: number;
	extra_costs: number;
	warranty: string;
	prepaid_at: string | null;
	photos: DemoPhoto[];
	history: HistoryEntry[];
}

const COLORS = ['чёрный', 'белый', 'синий', 'серый', 'красный', 'зелёный', ''];
const WORKS = [
	'Замена разъёма зарядки',
	'Замена дисплея',
	'Замена аккумулятора',
	'Чистка после залития',
	'Замена микрофона',
	'Прошивка',
	'Замена кнопки питания',
	'Чистка, замена термопасты'
];

/** The same small number for the same order: details do not change between page loads. */
const hash = (id: number, salt: number) => (Math.imul(id ^ salt, 2654435761) >>> 0) % 1000;

/** The fields a list row lacks, made up from the order number. */
export function withDetails(o: OrderListItem, kind: StatusKind): DemoOrder {
	const done = kind === 'ready' || kind === 'issued';
	const history: HistoryEntry[] = [{ id: 1, at: o.received_at, text: 'Принят' }];
	if (o.issued_at !== null) history.push({ id: 2, at: o.issued_at, text: 'Выдан' });
	return {
		...o,
		color: o.device_type === 'Компьютер' ? '' : (COLORS[hash(o.id, 1) % COLORS.length] ?? ''),
		device_password: hash(o.id, 2) < 300 ? String(1000 + (hash(o.id, 3) % 9000)) : '',
		// Some finished orders were never filled in: the issue dialog asks for it.
		work_done: done && hash(o.id, 4) < 800 ? (WORKS[hash(o.id, 5) % WORKS.length] ?? '') : '',
		internal_comment: '',
		discount: 0,
		extra_costs: 0,
		warranty: '30 дней',
		prepaid_at: o.prepayment > 0 ? o.received_at : null,
		photos: [],
		history
	};
}

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

/** Calendar days from a local date to `now`; null when the date cannot be read. */
export function daysSince(value: string, now: LocalDateTime): number | null {
	const t = parseLocalDateTime(value);
	return t ? dayNumber(now) - dayNumber(t) : null;
}

export type AttentionKey = 'call' | 'parts' | 'longDiagnostics' | 'longReady';
export const ATTENTION_KEYS: readonly AttentionKey[] = [
	'call',
	'parts',
	'longDiagnostics',
	'longReady'
];
/** Defaults of the settings «срок долгой диагностики» and «давно готов» (spec 6.8). */
export const LONG_DIAGNOSTICS_DAYS = 5;
export const LONG_READY_DAYS = 30;

/** «Требуют внимания» (spec 6.1): one rule per key. */
export function needsAttention(
	key: AttentionKey,
	o: OrderListItem,
	kind: StatusKind | undefined,
	now: LocalDateTime
): boolean {
	switch (key) {
		case 'call':
			return o.need_call;
		case 'parts':
			return o.need_parts;
		case 'longDiagnostics':
			return kind === 'diagnostics' && (daysSince(o.received_at, now) ?? 0) > LONG_DIAGNOSTICS_DAYS;
		case 'longReady':
			return kind === 'ready' && (daysSince(o.received_at, now) ?? 0) > LONG_READY_DAYS;
	}
}

export function attentionCounts(
	orders: readonly OrderListItem[],
	kindOf: (statusId: number) => StatusKind | undefined,
	now: LocalDateTime
): Record<AttentionKey, number> {
	const counts: Record<AttentionKey, number> = {
		call: 0,
		parts: 0,
		longDiagnostics: 0,
		longReady: 0
	};
	for (const o of orders) {
		const kind = kindOf(o.status_id);
		for (const key of ATTENTION_KEYS) if (needsAttention(key, o, kind, now)) counts[key]++;
	}
	return counts;
}

/**
 * What the list filter and Ctrl K look in (the order's `search_text`, spec 6.1): number, name, phone
 * as digits and as shown, device, IMEI or serial, barcode — normalized like `mw_domain::search`.
 */
export function orderSearchText(o: OrderListItem): string {
	return normalizeSearch(
		[
			o.id,
			o.client_name,
			o.client_phone,
			formatPhone(o.client_phone),
			o.device_type,
			o.brand,
			o.model,
			o.serial,
			o.barcode
		].join(' ')
	);
}

// An order object never changes in place (edits make a new one): its text can be kept.
const texts = new WeakMap<OrderListItem, string>();
function textOf(o: OrderListItem): string {
	let text = texts.get(o);
	if (text === undefined) {
		text = orderSearchText(o);
		texts.set(o, text);
	}
	return text;
}

/** The normalized words of a query; none for a blank one. */
export function queryWords(query: string): string[] {
	return normalizeSearch(query)
		.split(' ')
		.filter((w) => w !== '');
}

/** Every word occurs in the normalized text. */
export function matchesWords(text: string, words: readonly string[]): boolean {
	return words.every((w) => text.includes(w));
}

/** The list filter over orders (spec 6.1, «внутри текущего вида»). */
export function matchesOrder(o: OrderListItem, words: readonly string[]): boolean {
	return words.length === 0 || matchesWords(textOf(o), words);
}

export interface QuickFilterItem {
	id: string;
	label: string;
	count?: number;
	color?: string;
}

const IN_WORK: readonly StatusKind[] = ['open', 'diagnostics'];

/**
 * Quick filters (spec 6.1): «В работе» (open and diagnostics), then every status that is not issued
 * and has orders, in its order, then «Выданы» and «Все».
 */
export function quickFilterItems(
	orders: readonly OrderListItem[],
	statuses: readonly Status[]
): QuickFilterItem[] {
	const count = new Map<number, number>();
	for (const o of orders) count.set(o.status_id, (count.get(o.status_id) ?? 0) + 1);
	const listed = [...statuses]
		.filter((s) => !s.archived && s.kind !== 'issued' && (count.get(s.id) ?? 0) > 0)
		.sort((a, b) => a.sort_order - b.sort_order);
	const inWork = statuses
		.filter((s) => IN_WORK.includes(s.kind))
		.reduce((n, s) => n + (count.get(s.id) ?? 0), 0);
	return [
		{ id: 'work', label: 'В работе', count: inWork },
		...listed.map((s) => ({
			id: `status:${s.id}`,
			label: s.name,
			count: count.get(s.id) ?? 0,
			color: s.color
		})),
		{ id: 'issued', label: 'Выданы' },
		{ id: 'all', label: 'Все' }
	];
}

/** The union of the selected quick filters; nothing selected shows everything. */
export function matchesQuickFilter(
	selected: readonly string[],
	statusId: number,
	kind: StatusKind | undefined
): boolean {
	if (selected.length === 0) return true;
	return selected.some((id) => {
		if (id === 'all') return true;
		if (id === 'work') return kind !== undefined && IN_WORK.includes(kind);
		if (id === 'issued') return kind === 'issued';
		return id === `status:${statusId}`;
	});
}

export interface SortContext {
	statusOrder: (statusId: number) => number;
	pointName: (pointId: number | null) => string;
}

const collator = new Intl.Collator('ru');

function sortKey(columnId: string, o: OrderListItem, ctx: SortContext): number | string {
	switch (columnId) {
		case 'received':
			return o.received_at;
		case 'client':
			return o.client_name;
		case 'device':
			return deviceOf(o);
		case 'status':
			return ctx.statusOrder(o.status_id);
		case 'master':
			return o.master;
		case 'sum':
			return o.final_price ?? o.estimate_from ?? -1;
		case 'type':
			return o.device_type;
		case 'point':
			return ctx.pointName(o.service_point_id);
		case 'issued':
			return o.issued_at ?? '';
		case 'prepayment':
			return o.prepayment;
		default:
			return o.id;
	}
}

/** What the server would do: the whole list sorted by a column, ties newest first. */
export function sortOrders<T extends OrderListItem>(
	list: readonly T[],
	sort: SortState,
	ctx: SortContext
): T[] {
	const dir = sort.descending ? -1 : 1;
	const keyed = list.map((o) => ({ o, k: sortKey(sort.columnId, o, ctx) }));
	keyed.sort((a, b) => {
		const c =
			typeof a.k === 'number' && typeof b.k === 'number'
				? a.k - b.k
				: collator.compare(String(a.k), String(b.k));
		return c !== 0 ? c * dir : b.o.id - a.o.id;
	});
	return keyed.map((x) => x.o);
}

/**
 * A new status for an order, with the fields that come with it (the issue dialog's); the issue date
 * follows the status kind, the change goes to the history. `undo` gives back the status, the issue
 * date and every patched field as they were and drops the history entry; later edits of other fields
 * stay.
 */
export function withStatus(
	o: DemoOrder,
	status: Pick<Status, 'id' | 'name' | 'kind'>,
	at: string,
	patch: Partial<DemoOrder> = {}
): { order: DemoOrder; undo: (current: DemoOrder) => DemoOrder } {
	const entry: HistoryEntry = {
		id: Math.max(0, ...o.history.map((h) => h.id)) + 1,
		at,
		text: `Статус «${status.name}»`
	};
	const before = Object.fromEntries(
		Object.keys(patch).map((key) => [key, o[key as keyof DemoOrder]])
	) as Partial<DemoOrder>;
	return {
		order: {
			...o,
			...patch,
			status_id: status.id,
			issued_at: status.kind === 'issued' ? (o.issued_at ?? at) : null,
			history: [...o.history, entry]
		},
		undo: (current) => ({
			...current,
			...before,
			status_id: o.status_id,
			issued_at: o.issued_at,
			history: current.history.filter((h) => h !== entry)
		})
	};
}

/** «К оплате» (spec 6.2): final price minus prepayment and discount; null without a final price. */
export function toPay(o: { final_price: number | null; prepayment: number; discount: number }) {
	return o.final_price === null ? null : o.final_price - o.prepayment - o.discount;
}

const newestFirst = (a: OrderListItem, b: OrderListItem) =>
	a.received_at < b.received_at ? 1 : a.received_at > b.received_at ? -1 : b.id - a.id;

/** Ctrl K over all orders (spec 6.10): an order number first, then the newest matches. */
export function searchOrders<T extends OrderListItem>(
	orders: readonly T[],
	query: string,
	limit: number
): T[] {
	const words = queryWords(query);
	if (words.length === 0) return [];
	const q = query.trim();
	const exact = /^\d{1,7}$/.test(q) ? orders.find((o) => o.id === Number(q)) : undefined;
	const rest = orders
		.filter((o) => o !== exact && matchesWords(textOf(o), words))
		.sort(newestFirst)
		.slice(0, exact ? limit - 1 : limit);
	return exact ? [exact, ...rest] : rest;
}

export interface ClientHit {
	id: number;
	name: string;
	phone: string;
	count: number;
}

/** Ctrl K, «Клиенты»: each client once, by name or phone, with their number of orders. */
export function searchClients(
	orders: readonly OrderListItem[],
	query: string,
	limit: number
): ClientHit[] {
	const words = queryWords(query);
	if (words.length === 0) return [];
	const clients = new Map<number, ClientHit & { text: string }>();
	for (const o of orders) {
		const known = clients.get(o.client_id);
		if (known) {
			known.count++;
			continue;
		}
		clients.set(o.client_id, {
			id: o.client_id,
			name: o.client_name,
			phone: o.client_phone,
			count: 1,
			text: normalizeSearch(`${o.client_name} ${o.client_phone} ${formatPhone(o.client_phone)}`)
		});
	}
	const hits: ClientHit[] = [];
	for (const { text, ...client } of clients.values()) {
		if (matchesWords(text, words)) hits.push(client);
	}
	return hits.slice(0, limit);
}

export type ScanAction =
	| { kind: 'open'; id: number }
	| { kind: 'missing'; message: string }
	| { kind: 'search'; query: string };

/**
 * A scan outside a field (spec 6.10): an order number or a barcode opens the order; nothing found is
 * reported. Several orders with one barcode and other text go to Ctrl K, which lists the matches.
 */
export function resolveScan(r: ScanResult, orders: readonly OrderListItem[]): ScanAction {
	switch (r.kind) {
		case 'order':
			return orders.some((o) => o.id === r.id)
				? { kind: 'open', id: r.id }
				: { kind: 'missing', message: `Заказ № ${r.id} не найден` };
		case 'barcode': {
			const found = orders.filter((o) => o.barcode === r.value);
			if (found.length === 1) return { kind: 'open', id: found[0]!.id };
			return found.length === 0
				? { kind: 'missing', message: `Заказ со штрихкодом ${r.value} не найден` }
				: { kind: 'search', query: r.value };
		}
		case 'text':
			return { kind: 'search', query: r.value };
	}
}
