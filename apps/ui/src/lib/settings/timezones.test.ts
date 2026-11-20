import { afterEach, describe, expect, it, vi } from 'vitest';
import { canListTimezones, timezoneOptions, type TimezoneGroup } from './timezones.js';

afterEach(() => vi.unstubAllGlobals());

// Winter: no summer time in Europe, the offsets below are the standard ones.
const NOW = new Date('2026-01-15T12:00:00Z');

/** «(UTC+03:00) …» → minutes east of UTC. */
function minutes(label: string): number {
	const m = /^\(UTC([+-])(\d{2}):(\d{2})\)/.exec(label);
	if (!m) throw new Error(`no offset in «${label}»`);
	return (m[1] === '-' ? -1 : 1) * (Number(m[2]) * 60 + Number(m[3]));
}

const labels = (g: TimezoneGroup) => g.options.map((o) => o.label);
const find = (groups: TimezoneGroup[], value: string) =>
	groups.flatMap((g) => g.options).find((o) => o.value === value);

describe('timezoneOptions', () => {
	it('«Россия» first: its own names, west to east, with the offset in front', () => {
		const groups = timezoneOptions(NOW, 'Europe/Moscow');
		expect(groups[0]!.label).toBe('Россия');
		expect(labels(groups[0]!)).toEqual([
			'(UTC+02:00) Калининград',
			'(UTC+03:00) Москва',
			'(UTC+04:00) Самара',
			'(UTC+05:00) Екатеринбург',
			'(UTC+06:00) Омск',
			'(UTC+07:00) Новосибирск',
			'(UTC+07:00) Красноярск',
			'(UTC+08:00) Иркутск',
			'(UTC+09:00) Якутск',
			'(UTC+10:00) Владивосток',
			'(UTC+11:00) Магадан',
			'(UTC+12:00) Камчатка'
		]);
		expect(groups[0]!.options[1]).toEqual({ value: 'Europe/Moscow', label: '(UTC+03:00) Москва' });
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
			label: '(UTC+01:00) Центральная Европа, стандартное время — Europe/Berlin'
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
			/^\(UTC-05:00\) .* — America\/New_York$/
		);
		expect(find(groups, 'Africa/Abidjan')!.label).toMatch(/^\(UTC\+00:00\) /);
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
			{ value: 'UTC', label: '(UTC+00:00) Всемирное координированное время — UTC' }
		]);
	});

	it('without Intl.supportedValuesOf nothing is listed: the name is typed as before', () => {
		expect(canListTimezones()).toBe(true);
		vi.stubGlobal('Intl', { ...Intl, supportedValuesOf: undefined });
		expect(canListTimezones()).toBe(false);
		expect(timezoneOptions(NOW, 'Europe/Moscow')).toEqual([]);
	});
});
