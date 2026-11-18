import { describe, expect, it } from 'vitest';
import { dialNumber, formatPhone, phoneDigits, phoneKey } from './phone.js';

describe('phone', () => {
	it('formats', () => {
		expect(formatPhone('89211234567')).toBe('+7 921 123-45-67');
		expect(formatPhone('79211234567')).toBe('+7 921 123-45-67');
		expect(formatPhone('9211234567')).toBe('+7 921 123-45-67');
		expect(formatPhone('123456')).toBe('12-34-56');
		expect(formatPhone('12345')).toBe('12345');
		expect(formatPhone('4951234567890')).toBe('4951234567890');
		expect(formatPhone('')).toBe('');
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
