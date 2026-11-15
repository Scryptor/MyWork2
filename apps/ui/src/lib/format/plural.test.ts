import { describe, expect, it } from 'vitest';
import { countLabel, plural } from './plural.js';

const F = ['заказ', 'заказа', 'заказов'] as const;

describe('plural', () => {
	it('picks the form', () => {
		expect(plural(1, F)).toBe('заказ');
		expect(plural(2, F)).toBe('заказа');
		expect(plural(5, F)).toBe('заказов');
		expect(plural(11, F)).toBe('заказов');
		expect(plural(21, F)).toBe('заказ');
		expect(plural(22, F)).toBe('заказа');
		expect(plural(0, F)).toBe('заказов');
		expect(plural(1.5, F)).toBe('заказа');
	});
	it('countLabel', () => {
		expect(countLabel(1234, F)).toBe('1 234 заказа');
	});
});
