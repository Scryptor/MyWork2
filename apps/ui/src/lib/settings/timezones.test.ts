import { afterEach, describe, expect, it, vi } from 'vitest';
import { canListTimezones, timezoneOptions, type TimezoneGroup } from './timezones.js';

afterEach(() => vi.unstubAllGlobals());

// Winter: no summer time in Europe, the offsets below are the standard ones.
const NOW = new Date('2026-01-15T12:00:00Z');

/** «Москва (UTC+03:00)» → minutes east of UTC. */
function minutes(label: string): number {
	const m = /\(UTC([+-])(\d{2}):(\d{2})\)/.exec(label);
	if (!m) throw new Error(`no offset in «${label}»`);
	return (m[1] === '-' ? -1 : 1) * (Number(m[2]) * 60 + Number(m[3]));
}

const labels = (g: TimezoneGroup) => g.options.map((o) => o.label);
const find = (groups: TimezoneGroup[], value: string) =>
	groups.flatMap((g) => g.options).find((o) => o.value === value);

describe('timezoneOptions', () => {
	it('«Россия» first: its own names, west to east, the offset after the name', () => {
		const groups = timezoneOptions(NOW, 'Europe/Moscow');
		expect(groups[0]!.label).toBe('Россия');
		expect(labels(groups[0]!)).toEqual([
			'Калининград (UTC+02:00)',
			'Москва (UTC+03:00)',
			'Самара (UTC+04:00)',
			'Екатеринбург (UTC+05:00)',
			'Омск (UTC+06:00)',
			'Новосибирск (UTC+07:00)',
			'Красноярск (UTC+07:00)',
			'Иркутск (UTC+08:00)',
			'Якутск (UTC+09:00)',
			'Владивосток (UTC+10:00)',
			'Магадан (UTC+11:00)',
			'Камчатка (UTC+12:00)'
		]);
		expect(groups[0]!.options[1]).toEqual({ value: 'Europe/Moscow', label: 'Москва (UTC+03:00)' });
	});

	it('the rest by the part of the name before «/», in Russian; a zone of Russia is in no other group', () => {
		const groups = timezoneOptions(NOW, 'Europe/Moscow');
		const names = groups.map((g) => g.label);
		const order = [
			'Россия',
			'Европа',
			'Азия',
			'Америка',
			'Африка',
			'Австралия',
			'Тихий океан',
			'Атлантика',
			'Индийский океан',
			'Антарктида',
			'Арктика',
			'Другие'
		];
		// Every group is one of these, in this order (an engine may have no zone for some of them).
		expect(names.filter((n) => order.includes(n))).toEqual(names);
		expect([...names].sort((a, b) => order.indexOf(a) - order.indexOf(b))).toEqual(names);
		const all = groups.flatMap((g) => g.options.map((o) => o.value));
		expect(new Set(all).size).toBe(all.length);
		expect(all.filter((v) => v === 'Europe/Moscow')).toHaveLength(1);
		expect(find(groups, 'Europe/Berlin')).toEqual({
			value: 'Europe/Berlin',
			label: 'Центральная Европа, стандартное время (UTC+01:00) — Europe/Berlin'
		});
		expect(groups.find((g) => g.label === 'Европа')!.options.map((o) => o.value)).toContain(
			'Europe/Berlin'
		);
		expect(groups.find((g) => g.label === 'Америка')!.options.map((o) => o.value)).toContain(
			'America/New_York'
		);
	});

	it('inside a group: by the offset, then by the label', () => {
		for (const g of timezoneOptions(NOW, 'Europe/Moscow')) {
			const offsets = g.options.map((o) => minutes(o.label));
			expect(offsets, g.label).toEqual([...offsets].sort((a, b) => a - b));
			if (g.label === 'Россия') continue;
			for (let i = 1; i < g.options.length; i++) {
				const [a, b] = [g.options[i - 1]!.label, g.options[i]!.label];
				if (minutes(a) === minutes(b))
					expect(a.localeCompare(b, 'ru'), `${a} | ${b}`).toBeLessThan(0);
			}
		}
	});

	it('a negative offset and a zero one are written as UTC', () => {
		const groups = timezoneOptions(NOW, 'Europe/Moscow');
		expect(find(groups, 'America/New_York')!.label).toMatch(
			/^[^(]+ \(UTC-05:00\) — America\/New_York$/
		);
		expect(find(groups, 'Africa/Abidjan')!.label).toMatch(/ \(UTC\+00:00\) — Africa\/Abidjan$/);
	});

	it('a saved zone that is not in the list stays: a group of its own, first, named by the zone', () => {
		const groups = timezoneOptions(NOW, 'Etc/Custom');
		expect(groups[0]).toEqual({
			label: 'Сейчас выбран',
			options: [{ value: 'Etc/Custom', label: 'Etc/Custom' }]
		});
		expect(groups[1]!.label).toBe('Россия');
		// A zone that is in the list has no such group; nor has an empty one.
		expect(timezoneOptions(NOW, 'Europe/Berlin')[0]!.label).toBe('Россия');
		expect(timezoneOptions(NOW, '')[0]!.label).toBe('Россия');
	});

	it('«UTC» is always offered, under «Другие», also by an engine that does not list it', () => {
		const groups = timezoneOptions(NOW, 'UTC');
		expect(groups[0]!.label).toBe('Россия');
		const other = groups.find((g) => g.label === 'Другие')!;
		expect(other.options.filter((o) => o.value === 'UTC')).toEqual([
			{ value: 'UTC', label: 'Всемирное координированное время (UTC+00:00) — UTC' }
		]);
	});

	it('a name that is only an offset again is left out: the zone, then the offset', () => {
		// The engine names some zones by the offset alone («GMT+01:00»); the label then starts with the zone.
		for (const o of timezoneOptions(NOW, '').flatMap((g) => g.options))
			expect(o.label, o.value).not.toMatch(/^(GMT|UTC)[+-]/);
	});

	it('typing the first letters finds a zone: every label starts with its name', () => {
		for (const g of timezoneOptions(NOW, ''))
			for (const o of g.options) expect(o.label, o.value).toMatch(/^[^(\s]/);
	});

	it('the list is made once a day: another saved zone only adds or drops «Сейчас выбран»', () => {
		const base = timezoneOptions(NOW, 'Europe/Moscow');
		const spy = vi.spyOn(Intl, 'DateTimeFormat');
		const other = timezoneOptions(new Date('2026-01-15T18:00:00Z'), 'Europe/Berlin');
		const custom = timezoneOptions(NOW, 'Etc/Custom');
		expect(spy).not.toHaveBeenCalled();
		spy.mockRestore();
		expect(other).toEqual(base);
		other.forEach((g, i) => expect(g).toBe(base[i]));
		custom.slice(1).forEach((g, i) => expect(g).toBe(base[i]));
		// Another day: made again, with the offsets of that day (summer time in Berlin).
		const summer = timezoneOptions(new Date('2026-07-15T12:00:00Z'), 'Europe/Moscow');
		expect(find(summer, 'Europe/Berlin')!.label).toContain('(UTC+02:00)');
	});

	it('without Intl.supportedValuesOf nothing is listed: the name is typed as before', () => {
		expect(canListTimezones()).toBe(true);
		vi.stubGlobal('Intl', { ...Intl, supportedValuesOf: undefined });
		expect(canListTimezones()).toBe(false);
		expect(timezoneOptions(NOW, 'Europe/Moscow')).toEqual([]);
	});
});
