import { describe, expect, it } from 'vitest';
import { formatMoney, MAX_MONEY_KOPECKS, parseMoney } from './money.js';

const N = ' ';
const B = ' ';

describe('formatMoney', () => {
	it('formats kopecks', () => {
		expect(formatMoney(650000)).toBe(`6${N}500${B}₽`);
		expect(formatMoney(650050)).toBe(`6${N}500,50${B}₽`);
		expect(formatMoney(0)).toBe(`0${B}₽`);
		expect(formatMoney(12345678900)).toBe(`123${N}456${N}789${B}₽`);
		expect(formatMoney(-50000)).toBe(`−500${B}₽`);
		expect(formatMoney(650000, { currency: null })).toBe(`6${N}500`);
	});
});

describe('parseMoney', () => {
	it('parses valid input', () => {
		expect(parseMoney('6500')).toBe(650000);
		expect(parseMoney('6 500,5')).toBe(650050);
		expect(parseMoney('6 500.50')).toBe(650050);
		expect(parseMoney(`6${N}500.50`)).toBe(650050);
		expect(parseMoney(' 0 ')).toBe(0);
		expect(parseMoney('999999999,99')).toBe(MAX_MONEY_KOPECKS);
	});
	it('rejects invalid input', () => {
		for (const bad of ['', 'abc', '-100', '−100', '1e3', '10,555', '1,2,3', '999999999999'])
			expect(parseMoney(bad)).toBeNull();
	});
});
