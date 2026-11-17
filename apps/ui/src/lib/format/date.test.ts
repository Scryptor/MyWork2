import { describe, expect, it } from 'vitest';
import {
	dayNumber,
	formatDate,
	formatDay,
	formatFull,
	formatRelative,
	parseLocalDateTime
} from './date.js';

const now = { year: 2026, month: 11, day: 15, hour: 12, minute: 0, second: 0 };

describe('parseLocalDateTime', () => {
	it('parses', () => {
		expect(parseLocalDateTime('2026-11-15T10:24:00')).toEqual({
			year: 2026,
			month: 11,
			day: 15,
			hour: 10,
			minute: 24,
			second: 0
		});
		expect(parseLocalDateTime('2026-11-15T10:24:00.123')?.minute).toBe(24);
		expect(parseLocalDateTime('2026-11-15T10:24')).not.toBeNull();
		expect(parseLocalDateTime('2026-11-15 10:24:00')).not.toBeNull();
		expect(parseLocalDateTime('2026-11-15')).toEqual({
			year: 2026,
			month: 11,
			day: 15,
			hour: 0,
			minute: 0,
			second: 0
		});
	});
	it('rejects', () => {
		expect(parseLocalDateTime('2026-02-30T10:00:00')).toBeNull();
		expect(parseLocalDateTime('мусор')).toBeNull();
	});
});

describe('formatRelative', () => {
	it('formats', () => {
		expect(formatRelative('2026-11-15T10:24:00', now)).toBe('сегодня 10:24');
		expect(formatRelative('2026-11-14T17:02:00', now)).toBe('вчера 17:02');
		expect(formatRelative('2026-11-12T09:51:00', now)).toBe('12 ноя 09:51');
		expect(formatRelative('2026-05-03T09:05:00', now)).toBe('3 мая 09:05');
		expect(formatRelative('2026-11-16T09:00:00', now)).toBe('16 ноя 09:00');
		expect(formatRelative('2024-11-12T09:51:00', now)).toBe('12.11.2024');
		expect(formatRelative('мусор', now)).toBe('мусор');
		expect(
			formatRelative('2025-12-31T23:00:00', {
				year: 2026,
				month: 1,
				day: 1,
				hour: 8,
				minute: 0,
				second: 0
			})
		).toBe('вчера 23:00');
	});
});

describe('formatFull / formatDate', () => {
	it('formats', () => {
		expect(formatFull('2024-11-12T09:51:00')).toBe('12 ноября 2024, 09:51');
		expect(formatDate('2024-11-12T09:51:00')).toBe('12.11.2024');
	});
});

describe('dayNumber', () => {
	it('differs by one for consecutive days and ignores the time', () => {
		const a = { ...now, hour: 23, minute: 59 };
		const b = { ...now, day: 14, hour: 0, minute: 0 };
		expect(dayNumber(a) - dayNumber(b)).toBe(1);
	});
});

describe('formatDay', () => {
	it('formats a moment already parsed as the date', () => {
		expect(formatDay({ year: 2026, month: 3, day: 7, hour: 9, minute: 5, second: 0 })).toBe(
			'07.03.2026'
		);
	});
});
