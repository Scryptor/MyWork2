import { describe, expect, it } from 'vitest';
import { asPrimary, keptPhones, MIN_PHONE_DIGITS, savedPhones, typedNumber } from './phones.js';

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

describe('typedNumber', () => {
	const list = [
		{ phone: '79211234567', note: '' },
		{ phone: '79001112233', note: 'раб' }
	];

	it('a number of 5 digits and more goes into the list', () => {
		expect(MIN_PHONE_DIGITS).toBe(5);
		expect(typedNumber(list, null, 1, '79005554433', '79001112233')).toEqual({
			phones: [list[0], { phone: '79005554433', note: 'раб' }],
			draft: null
		});
		expect(typedNumber(list, null, 0, '12345', '79211234567').phones[0]!.phone).toBe('12345');
	});

	it('a shorter one (erased with Backspace) leaves the number it had before typing', () => {
		for (const digits of ['7921', '79', '7', '']) {
			expect(typedNumber(list, null, 0, digits, '79211234567')).toEqual({
				phones: list,
				draft: null
			});
		}
	});

	it('a row of «＋ номер» joins the list at 5 digits and leaves it again below', () => {
		const draft = { phone: '', note: 'жена' };
		expect(typedNumber(list, draft, 2, '7900', null)).toEqual({
			phones: list,
			draft: { phone: '7900', note: 'жена' }
		});
		const joined = typedNumber(list, { phone: '7900', note: 'жена' }, 2, '79005', null);
		expect(joined).toEqual({ phones: [...list, { phone: '79005', note: 'жена' }], draft: null });
		// Backspace in the same typing: back to a row of its own, nothing of it in the list.
		expect(typedNumber(joined.phones, null, 2, '7900', null)).toEqual({
			phones: list,
			draft: { phone: '7900', note: 'жена' }
		});
	});
});
