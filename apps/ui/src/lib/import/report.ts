// The report of a transfer from MyWork 2 (spec 6.9): totals as tiles, the groups of warnings by what they
// need, the numbers of their orders.
import type { ImportReportView } from '$api/ImportReportView';
import type { ReportEntry } from '$api/ReportEntry';
import type { ReportGroup } from '$api/ReportGroup';
import type { WarningCode } from '$api/WarningCode';
import { formatBytes } from '#lib/format/bytes.js';
import { groupDigits } from '#lib/format/digits.js';
import { parseLocalDateTime } from '#lib/format/date.js';
import { ru } from '#lib/i18n/ru.js';

const t = ru.legacy;

/** The route of an import's report, for `resolve`; `latest`: the last transfer's (it waits for its end). */
export function reportRoute(id: number | 'latest'): `settings/import/${string}` {
	return `settings/import/${id}`;
}

/** Where the sign-in leads after a transfer that brought users of its own. */
export const LATEST_REPORT = '/settings/import/latest';

export interface ReportSection {
	id: 'attention' | 'info';
	title: string;
	groups: ReportGroup[];
}

/** «Требует внимания» (warnings), then «Сведения»; a section without groups is left out. */
export function reportSections(groups: readonly ReportGroup[]): ReportSection[] {
	const sections: ReportSection[] = [
		{
			id: 'attention',
			title: t.attentionTitle,
			groups: groups.filter((g) => g.severity === 'warning')
		},
		{ id: 'info', title: t.info, groups: groups.filter((g) => g.severity === 'info') }
	];
	return sections.filter((s) => s.groups.length > 0);
}

export interface Tile {
	label: string;
	value: string;
	hint?: string;
	tone?: 'warning';
}

const count = (n: number) => groupDigits(String(n));

/**
 * The totals: orders, clients, users, the items of the stock (when there were any), files with their size, files
 * not found, warnings to look at.
 */
export function reportTiles(r: ImportReportView): Tile[] {
	const attention = r.groups
		.filter((g) => g.severity === 'warning')
		.reduce((sum, g) => sum + g.count, 0);
	return [
		{ label: t.orders, value: count(r.orders) },
		{ label: t.clients, value: count(r.clients) },
		{ label: t.users, value: count(r.users) },
		...(r.stock_items > 0 ? [{ label: t.stockItems, value: count(r.stock_items) }] : []),
		{ label: t.files, value: count(r.files.copied), hint: formatBytes(r.files.bytes) },
		...(r.files.missing > 0
			? [{ label: t.missing, value: count(r.files.missing), tone: 'warning' as const }]
			: []),
		attention > 0
			? { label: t.attention, value: count(attention), tone: 'warning' }
			: { label: t.attention, value: '0' }
	];
}

/** How many numbers of a group are shown before «Показать все». */
export const ORDERS_SHOWN = 100;

/** The numbers of a group on the screen: the first ones, or all once asked for. */
export function visibleOrders(ids: readonly number[], all: boolean): readonly number[] {
	return all || ids.length <= ORDERS_SHOWN ? ids : ids.slice(0, ORDERS_SHOWN);
}

const p2 = (n: number) => String(n).padStart(2, '0');

/** «mywork2-import-2026-11-19-1405.txt»: the file of «Сохранить отчёт». */
export function reportFileName(r: Pick<ImportReportView, 'imported_at'>): string {
	const at = parseLocalDateTime(r.imported_at);
	const stamp = at
		? `${at.year}-${p2(at.month)}-${p2(at.day)}-${p2(at.hour)}${p2(at.minute)}`
		: r.imported_at.replace(/\D/g, '');
	return `mywork2-import-${stamp}.txt`;
}

const isNumber = (s: string) => /^\d+$/.test(s);

/**
 * Where an entry comes from, in the words of the program: «Заказ № 5 · Клиент», «Файл · 1/1/a.jpg».
 * The key goes into the name of an order or a client, else after the source; no text is said twice (the key that
 * is the source or the value, the field that is in the key or is the value is left out). Unknown names stay raw.
 * `code` is the group's: the key of a repeated barcode is the barcode, not a number of an order (as on the server).
 */
export function entryWhere(e: ReportEntry, code?: WarningCode): string {
	const rr = ru.importReport;
	const source = e.source.trim();
	const key = e.key.trim();
	const value = e.value.trim();
	const field = e.field.trim();
	const known = Object.hasOwn(rr.sources, source);
	let head = known ? rr.sources[source] : source;
	let keyUsed = key === '' || key === source || key === value;
	if (!keyUsed && isNumber(key) && source === 'Catalog' && code !== 'barcode_duplicate') {
		head = rr.orderNumber(key);
		keyUsed = true;
	} else if (!keyUsed && isNumber(key) && source === 'ClientsMap') {
		head = rr.clientNumber(key);
		keyUsed = true;
	} else if (!keyUsed && isNumber(key) && source === 'Stock') {
		head = rr.stockNumber(key);
		keyUsed = true;
	}
	const label = Object.hasOwn(rr.fields, field) ? rr.fields[field] : field;
	return [
		head,
		keyUsed ? '' : key,
		field === '' || key.includes(field) || field === value ? '' : label
	]
		.filter((part) => part !== '')
		.join(' · ');
}
