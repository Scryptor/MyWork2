import { describe, expect, it } from 'vitest';
import { asPrimary, keptPhones, savedPhones } from './phones.js';

describe('savedPhones', () => {
	it('keeps the rows with a number: digits only, the note trimmed', () => {
		expect(
			savedPhones([
				{ phone: '+7 921 123-45-67', note: ' жена ' },
				{ phone: '', note: '' },
				{ phone: '  ', note: 'раб' },
				{ phone: '12-34-56', note: '' }
			])
		).toEqual([
			{ phone: '79211234567', note: 'жена' },
			{ phone: '123456', note: '' }
		]);
	});
});

describe('keptPhones', () => {
	it('a list sent is the server one when the server only cleaned it', () => {
		const server = [{ phone: '79211234567', note: 'жена' }];
		expect(
			keptPhones(
				[
					{ phone: '79211234567', note: 'жена ' },
					{ phone: '', note: '' }
				],
				server
			)
		).toBe(true);
		expect(keptPhones([{ phone: '79211234567', note: 'мама' }], server)).toBe(false);
		expect(keptPhones('text', server)).toBe(false);
	});
});

describe('asPrimary', () => {
	it('moves the number to the front, the others keep their order', () => {
		const rows = [
			{ phone: '1', note: '' },
			{ phone: '2', note: 'жена' },
			{ phone: '3', note: '' }
		];
		expect(asPrimary(rows, 2).map((p) => p.phone)).toEqual(['3', '1', '2']);
		expect(asPrimary(rows, 1).map((p) => p.phone)).toEqual(['2', '1', '3']);
		expect(asPrimary(rows, 0)).toEqual(rows);
		expect(asPrimary(rows, 5)).toEqual(rows);
	});
});
