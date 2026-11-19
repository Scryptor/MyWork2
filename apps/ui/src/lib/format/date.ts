export interface LocalDateTime {
	year: number;
	month: number;
	day: number;
	hour: number;
	minute: number;
	second: number;
}

const SHORT_MONTHS = [
	'янв',
	'фев',
	'мар',
	'апр',
	'мая',
	'июн',
	'июл',
	'авг',
	'сен',
	'окт',
	'ноя',
	'дек'
];
const FULL_MONTHS = [
	'января',
	'февраля',
	'марта',
	'апреля',
	'мая',
	'июня',
	'июля',
	'августа',
	'сентября',
	'октября',
	'ноября',
	'декабря'
];

function daysInMonth(year: number, month: number): number {
	return new Date(Date.UTC(year, month, 0)).getUTCDate();
}

const RE = /^(\d{4})-(\d{2})-(\d{2})(?:[T ](\d{2}):(\d{2})(?::(\d{2})(?:\.\d+)?)?)?$/;

export function parseLocalDateTime(value: string): LocalDateTime | null {
	const m = RE.exec(value.trim());
	if (!m) return null;
	const [year, month, day, hour, minute, second] = [m[1], m[2], m[3], m[4], m[5], m[6]].map((x) =>
		x === undefined ? 0 : Number(x)
	) as [number, number, number, number, number, number];
	if (month < 1 || month > 12 || day < 1 || day > daysInMonth(year, month)) return null;
	if (hour > 23 || minute > 59 || second > 59) return null;
	return { year, month, day, hour, minute, second };
}

export function nowLocal(): LocalDateTime {
	const d = new Date();
	return {
		year: d.getFullYear(),
		month: d.getMonth() + 1,
		day: d.getDate(),
		hour: d.getHours(),
		minute: d.getMinutes(),
		second: d.getSeconds()
	};
}

const p2 = (n: number) => String(n).padStart(2, '0');
const time = (t: LocalDateTime) => `${p2(t.hour)}:${p2(t.minute)}`;
/** Day number for differences in calendar days; the time of day is ignored. */
export const dayNumber = (t: LocalDateTime) => Date.UTC(t.year, t.month - 1, t.day) / 86_400_000;

function dotted(t: LocalDateTime): string {
	return `${p2(t.day)}.${p2(t.month)}.${t.year}`;
}

export function formatRelative(value: string, now: LocalDateTime): string {
	const t = parseLocalDateTime(value);
	if (!t) return value;
	const diff = dayNumber(now) - dayNumber(t);
	if (diff === 0) return `сегодня ${time(t)}`;
	if (diff === 1) return `вчера ${time(t)}`;
	if (t.year !== now.year) return dotted(t);
	return `${t.day} ${SHORT_MONTHS[t.month - 1]} ${time(t)}`;
}

export function formatFull(value: string): string {
	const t = parseLocalDateTime(value);
	if (!t) return value;
	return `${t.day} ${FULL_MONTHS[t.month - 1]} ${t.year}, ${time(t)}`;
}

/** «07.03.2026» of a moment already parsed (one computed here, such as the end of a warranty). */
export function formatDay(t: LocalDateTime): string {
	return dotted(t);
}

export function formatDate(value: string): string {
	const t = parseLocalDateTime(value);
	return t ? dotted(t) : value;
}

/** «2026-03-07» of a moment: the day as a date field and the server write it. */
export function isoDay(t: LocalDateTime): string {
	return `${t.year}-${p2(t.month)}-${p2(t.day)}`;
}

/** The first moment of a day «2026-03-07», as the server takes a local time. */
export const startOfDay = (day: string): string => `${day}T00:00:00`;
