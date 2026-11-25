// The view of «Отчёты»: the tab, the period, the grouping and the filters. It lives in the address like
// the other lists; `reportQuery` turns it into the server's query.
import type { MovesReportQuery } from '$api/MovesReportQuery';
import type { OrderReportQuery } from '$api/OrderReportQuery';
import type { ReportGroupBy } from '$api/ReportGroupBy';
import { parseLocalDateTime } from '#lib/format/date.js';
import { periodRange, type PeriodKind } from '#lib/ui/period.js';

export interface ReportView {
	/** `moves` is the transfer report of its own tab; the default is the orders. */
	tab: 'orders' | 'moves';
	period: PeriodKind;
	/** The first and the last day of `custom`; other periods have no days of their own. */
	from: string | null;
	to: string | null;
	groupBy: ReportGroupBy;
	masterUserId?: number;
	servicePointId?: number;
	deviceType?: string;
	brand?: string;
	source?: string;
}

export const DEFAULT_PERIOD: PeriodKind = 'month';

const PERIODS: readonly PeriodKind[] = [
	'today',
	'week',
	'month',
	'last_month',
	'year',
	'all',
	'custom'
];
/** The word of the address for each grouping; the master is the default and has none. */
const GROUP_PARAM: Record<Exclude<ReportGroupBy, 'master'>, string> = {
	service_point: 'point',
	device_type: 'type',
	brand: 'brand',
	source: 'source'
};

const positive = (s: string | null): number | undefined =>
	s !== null && /^[1-9]\d*$/.test(s) ? Number(s) : undefined;
const text = (s: string | null): string | undefined =>
	s !== null && s.trim() !== '' ? s : undefined;
const day = (s: string | null): string | null =>
	s !== null && /^\d{4}-\d{2}-\d{2}$/.test(s) && parseLocalDateTime(s) !== null ? s : null;

export function reportViewFromSearch(p: Pick<URLSearchParams, 'get'>): ReportView {
	const period = PERIODS.find((k) => k === p.get('period')) ?? DEFAULT_PERIOD;
	const by = p.get('by');
	const groupBy =
		(Object.keys(GROUP_PARAM) as (keyof typeof GROUP_PARAM)[]).find((k) => GROUP_PARAM[k] === by) ??
		'master';
	const view: ReportView = {
		tab: p.get('tab') === 'moves' ? 'moves' : 'orders',
		period,
		from: period === 'custom' ? day(p.get('from')) : null,
		to: period === 'custom' ? day(p.get('to')) : null,
		groupBy
	};
	const set = <K extends 'masterUserId' | 'servicePointId' | 'deviceType' | 'brand' | 'source'>(
		key: K,
		value: ReportView[K] | undefined
	) => {
		if (value !== undefined) view[key] = value;
	};
	set('masterUserId', positive(p.get('m')));
	set('servicePointId', positive(p.get('p')));
	set('deviceType', text(p.get('t')));
	set('brand', text(p.get('b')));
	set('source', text(p.get('s')));
	return view;
}

/** The search part of the address without `?`; the default view is ''. */
export function reportViewToSearch(v: ReportView): string {
	const out: [string, string][] = [];
	if (v.tab === 'moves') out.push(['tab', 'moves']);
	if (v.period !== DEFAULT_PERIOD) out.push(['period', v.period]);
	if (v.period === 'custom') {
		if (v.from !== null) out.push(['from', v.from]);
		if (v.to !== null) out.push(['to', v.to]);
	}
	if (v.groupBy !== 'master') out.push(['by', GROUP_PARAM[v.groupBy]]);
	if (v.masterUserId !== undefined) out.push(['m', String(v.masterUserId)]);
	if (v.servicePointId !== undefined) out.push(['p', String(v.servicePointId)]);
	if (v.deviceType) out.push(['t', v.deviceType]);
	if (v.brand) out.push(['b', v.brand]);
	if (v.source) out.push(['s', v.source]);
	return out.map(([key, value]) => `${key}=${encodeURIComponent(value)}`).join('&');
}

/** The days of the period are known only to the screen: `today` is the enterprise's local day. */
export function reportQuery(v: ReportView, today: string): OrderReportQuery {
	const [from, to] = v.period === 'custom' ? [v.from, v.to] : periodRange(v.period, today);
	const q: OrderReportQuery = { group_by: v.groupBy };
	if (from !== null) q.from = from;
	if (to !== null) q.to = to;
	if (v.masterUserId !== undefined) q.master_user_id = v.masterUserId;
	if (v.servicePointId !== undefined) q.service_point_id = v.servicePointId;
	if (v.deviceType) q.device_type = v.deviceType;
	if (v.brand) q.brand = v.brand;
	if (v.source) q.source = v.source;
	return q;
}

/** The transfers use the period and the point of the view; the other filters are the orders' own. */
export function movesQuery(v: ReportView, today: string): MovesReportQuery {
	const [from, to] = v.period === 'custom' ? [v.from, v.to] : periodRange(v.period, today);
	const q: MovesReportQuery = {};
	if (from !== null) q.from = from;
	if (to !== null) q.to = to;
	if (v.servicePointId !== undefined) q.service_point_id = v.servicePointId;
	return q;
}
