import { afterEach, describe, expect, it } from 'vitest';
import { setCurrencySymbol } from './currency.svelte.js';
import { formatMoney, MAX_MONEY_KOPECKS, parseMoney } from './money.js';

const N = ' ';
const B = ' ';

afterEach(() => setCurrencySymbol(null));

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

describe('formatMoney: the currency of the settings', () => {
	it('uses the symbol of the settings when none is given, and ₽ until they are loaded', () => {
		expect(formatMoney(650000)).toBe(`6${N}500${B}₽`);
		setCurrencySymbol('$');
		expect(formatMoney(650000)).toBe(`6${N}500${B}$`);
		expect(formatMoney(650000, { currency: '€' })).toBe(`6${N}500${B}€`);
		expect(formatMoney(650000, { currency: null })).toBe(`6${N}500`);
		setCurrencySymbol('   ');
		expect(formatMoney(650000)).toBe(`6${N}500${B}₽`);
	});
});

/** The same examples are in `crates/mw-domain/src/money.rs` (`format_money_doc`): documents print sums as here. */
const SHARED_EXAMPLES: [number, string, string][] = [
	[650000, '$', `6${N}500${B}$`],
	[650050, '$', `6${N}500,50${B}$`],
	[0, '€', `0${B}€`],
	[5, '$', `0,05${B}$`],
	[12345678900, '$', `123${N}456${N}789${B}$`],
	[-50000, '$', `−500${B}$`],
	[650000, '', `6${N}500`]
];

describe('formatMoney: examples shared with printed documents', () => {
	it.each(SHARED_EXAMPLES)('%i %s', (kopecks, currency, shown) => {
		expect(formatMoney(kopecks, { currency })).toBe(shown);
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
	it('rejects amounts over the maximum', () => {
		// 1 000 000 000 ₽ is 100 000 000 000 kopecks, one kopeck over the maximum.
		expect(parseMoney('1000000000')).toBeNull();
		expect(parseMoney('999999999,99')).toBe(MAX_MONEY_KOPECKS);
	});
});
