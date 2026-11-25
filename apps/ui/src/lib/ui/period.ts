/** Quick periods of a report; `custom` carries its own two days. */
export type PeriodKind = 'today' | 'week' | 'month' | 'last_month' | 'year' | 'all' | 'custom';

function pad(n: number, width = 2): string {
	return String(n).padStart(width, '0');
}

function iso(y: number, m: number, d: number): string {
	return `${pad(y, 4)}-${pad(m)}-${pad(d)}`;
}

/** ISO day as a UTC date: no time zone or daylight saving can shift the day. */
function parse(day: string): Date {
	const [y, m, d] = day.split('-').map(Number) as [number, number, number];
	return new Date(Date.UTC(y, m - 1, d));
}

function fromDate(d: Date): string {
	return iso(d.getUTCFullYear(), d.getUTCMonth() + 1, d.getUTCDate());
}

/**
 * The days a quick period covers, both ends included, for the enterprise's local `today` (`YYYY-MM-DD`).
 * The week starts on Monday; `all` has no bounds.
 */
export function periodRange(
	kind: Exclude<PeriodKind, 'custom'>,
	today: string
): [string | null, string | null] {
	const t = parse(today);
	const y = t.getUTCFullYear();
	const m = t.getUTCMonth() + 1;
	switch (kind) {
		case 'today':
			return [today, today];
		case 'week': {
			const sinceMonday = (t.getUTCDay() + 6) % 7;
			return [fromDate(new Date(t.getTime() - sinceMonday * 86_400_000)), today];
		}
		case 'month':
			return [iso(y, m, 1), today];
		case 'last_month': {
			const first = new Date(Date.UTC(y, m - 2, 1));
			const last = new Date(Date.UTC(y, m - 1, 0));
			return [fromDate(first), fromDate(last)];
		}
		case 'year':
			return [iso(y, 1, 1), today];
		case 'all':
			return [null, null];
	}
}
