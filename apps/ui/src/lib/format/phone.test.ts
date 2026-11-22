import { describe, expect, it } from 'vitest';
import { dialNumber, formatPhone, phoneDigits, phoneKey } from './phone.js';

/** The same examples are in `crates/mw-domain/src/phone.rs` (`format_phone`): documents print numbers as here. */
const FORMAT_EXAMPLES: [string, string][] = [
	['89211234567', '+7 921 123-45-67'],
	['79211234567', '+7 921 123-45-67'],
	['9211234567', '+7 921 123-45-67'],
	['123456', '12-34-56'],
	['12345', '12345'],
	['19211234567', '19211234567'],
	['4951234567890', '4951234567890'],
	['', '']
];

describe('phone', () => {
	it.each(FORMAT_EXAMPLES)('formats «%s»', (digits, shown) => {
		expect(formatPhone(digits)).toBe(shown);
	});
	it('extracts digits', () => {
		expect(phoneDigits('+7 (921) 123-45-67')).toBe('79211234567');
	});
	it('one number written as +7, 8 or without the code has one key, as the server matches it', () => {
		expect(phoneKey('+7 (921) 123-45-67')).toBe('9211234567');
		expect(phoneKey('8 921 123 45 67')).toBe('9211234567');
		expect(phoneKey('9211234567')).toBe('9211234567');
		expect(phoneKey('2-12-34')).toBe('21234');
	});
	it('dials with the country code', () => {
		expect(dialNumber('79211234567')).toBe('+79211234567');
		expect(dialNumber('89211234567')).toBe('+79211234567');
		expect(dialNumber('9211234567')).toBe('+79211234567');
		expect(dialNumber('123456')).toBe('123456');
		expect(dialNumber('')).toBe('');
	});
});
