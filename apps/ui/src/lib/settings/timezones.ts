// The time zones of «Общие» as a list: «Москва (UTC+03:00)», the name first so that typing finds it.
import { ru } from '#lib/i18n/ru.js';

export interface TimezoneOption {
	value: string;
	label: string;
}
export interface TimezoneGroup {
	label: string;
	options: TimezoneOption[];
}

const t = ru.timezones;

/** Russia first, with the names of its cities, west to east. */
const RUSSIA = Object.keys(t.russiaZones);

/** The part of a name before «/», in the order of the groups; any other zone goes to «Другие». */
const REGIONS = Object.keys(t.regions);

/** Every engine knows it (ECMA-402), but Chromium does not list it. */
const UTC = 'UTC';

/** `Intl.supportedValuesOf` is missing in older engines (WebKit before 15.4): the name is typed then. */
function listed(): string[] | null {
	const list: unknown =
		typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : null;
	return Array.isArray(list) && list.length > 0 ? (list as string[]) : null;
}

export function canListTimezones(): boolean {
	return listed() !== null;
}

function zoneName(zone: string, style: 'long' | 'longOffset', locale: string, now: Date): string {
	const parts = new Intl.DateTimeFormat(locale, {
		timeZone: zone,
		timeZoneName: style
	}).formatToParts(now);
	return parts.find((p) => p.type === 'timeZoneName')?.value ?? '';
}

/** «GMT+05:30» → «UTC+05:30» and 330 minutes; «GMT» alone is «UTC+00:00». */
function offsetOf(zone: string, now: Date): { text: string; minutes: number } {
	const raw = zoneName(zone, 'longOffset', 'en-US', now);
	const m = /^GMT(?:([+\-−])(\d{1,2})(?::(\d{2}))?)?$/.exec(raw);
	if (!m) throw new RangeError(`unknown offset ${raw}`);
	const sign = m[1] && m[1] !== '+' ? -1 : 1;
	const hours = Number(m[2] ?? 0);
	const mins = Number(m[3] ?? 0);
	const text = `UTC${sign < 0 ? '-' : '+'}${String(hours).padStart(2, '0')}:${String(mins).padStart(2, '0')}`;
	return { text, minutes: sign * (hours * 60 + mins) };
}

/** A name that is only an offset again («GMT+01:00») says nothing: the label then starts with the zone. */
const NO_NAME = /^(GMT|UTC)([+\-−]\d|$)/;

type Entry = TimezoneOption & { minutes: number };

/**
 * The name first, so that the first letters typed in the open list find it: «Москва (UTC+03:00)»,
 * «Центральная Европа, стандартное время (UTC+01:00) — Europe/Berlin».
 */
function entry(zone: string, now: Date, city?: string): Entry | null {
	try {
		const offset = offsetOf(zone, now);
		if (city) return { value: zone, label: `${city} (${offset.text})`, minutes: offset.minutes };
		const name = zoneName(zone, 'long', 'ru-RU', now);
		const label = NO_NAME.test(name)
			? `${zone} (${offset.text})`
			: `${name} (${offset.text}) — ${zone}`;
		return { value: zone, label, minutes: offset.minutes };
	} catch {
		// A zone the engine lists but cannot format is not offered.
		return null;
	}
}

const byOffset = (a: Entry, b: Entry) =>
	a.minutes - b.minutes || a.label.localeCompare(b.label, 'ru');
const plain = ({ value, label }: Entry): TimezoneOption => ({ value, label });

/** The groups of a day: some eight hundred formatters, so made once and kept until the date changes. */
let made: { day: string; groups: TimezoneGroup[]; offered: Set<string> } | null = null;

function groupsOf(now: Date, names: string[]): { groups: TimezoneGroup[]; offered: Set<string> } {
	const day = now.toISOString().slice(0, 10);
	if (made?.day === day) return made;
	const all = names.includes(UTC) ? names : [...names, UTC];
	const russia: Entry[] = [];
	for (const zone of RUSSIA) {
		const e = all.includes(zone) ? entry(zone, now, t.russiaZones[zone]) : null;
		if (e) russia.push(e);
	}
	const regions = new Map<string, Entry[]>();
	for (const zone of all) {
		if (RUSSIA.includes(zone)) continue;
		const e = entry(zone, now);
		if (!e) continue;
		const head = zone.split('/')[0] ?? '';
		const region = zone.includes('/') && REGIONS.includes(head) ? head : '';
		const list = regions.get(region);
		if (list) list.push(e);
		else regions.set(region, [e]);
	}
	const groups: TimezoneGroup[] = [];
	// Russia keeps the order of its list: west to east, and so by the offset.
	if (russia.length > 0) groups.push({ label: t.russia, options: russia.map(plain) });
	for (const region of [...REGIONS, '']) {
		const list = regions.get(region);
		if (!list) continue;
		groups.push({
			label: region ? t.regions[region]! : t.other,
			options: [...list].sort(byOffset).map(plain)
		});
	}
	const offered = new Set(groups.flatMap((g) => g.options.map((o) => o.value)));
	made = { day, groups, offered };
	return made;
}

/**
 * The zones grouped: «Россия» first, then by the part of the name before «/» (Европа, Азия, …, Другие),
 * inside a group by the offset at `now` and then by the label. A saved zone that is not in the list is a
 * group of its own, first: it stays shown and chosen. Empty when the engine cannot list the zones. The
 * groups are made once a day; another saved zone only adds or drops its own group.
 */
export function timezoneOptions(now: Date, saved: string): TimezoneGroup[] {
	const names = listed();
	if (!names) return [];
	const { groups, offered } = groupsOf(now, names);
	if (!saved || offered.has(saved)) return groups;
	return [{ label: t.current, options: [{ value: saved, label: saved }] }, ...groups];
}
