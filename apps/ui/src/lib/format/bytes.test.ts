import { describe, expect, it } from 'vitest';
import { formatBytes } from './bytes.js';

describe('formatBytes', () => {
	it('uses Б, КБ, МБ with a decimal comma', () => {
		expect(formatBytes(900)).toBe('900 Б');
		expect(formatBytes(0)).toBe('0 Б');
		expect(formatBytes(1536)).toBe('1,5 КБ');
		expect(formatBytes(1_234_567)).toBe('1,2 МБ');
		expect(formatBytes(25 * 1024 * 1024)).toBe('25 МБ');
	});
});
