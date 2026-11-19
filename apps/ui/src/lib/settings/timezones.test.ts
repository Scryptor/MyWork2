import { afterEach, describe, expect, it, vi } from 'vitest';
import { timezoneChoices } from './timezones.js';

afterEach(() => vi.unstubAllGlobals());

describe('timezoneChoices', () => {
	it('lists the zones of the engine and adds the current one when it is missing', () => {
		const c = timezoneChoices('Etc/Custom');
		expect(c.free).toBe(false);
		expect(c.items[0]).toEqual({ value: 'Etc/Custom' });
		expect(c.items.length).toBeGreaterThan(100);
		expect(
			timezoneChoices('Europe/Moscow').items.filter((i) => i.value === 'Europe/Moscow')
		).toHaveLength(1);
	});

	it('without Intl.supportedValuesOf offers the current zone and lets the name be typed', () => {
		vi.stubGlobal('Intl', { ...Intl, supportedValuesOf: undefined });
		expect(timezoneChoices('Europe/Moscow')).toEqual({
			items: [{ value: 'Europe/Moscow' }],
			free: true
		});
	});
});
