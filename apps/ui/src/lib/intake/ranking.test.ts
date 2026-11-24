import { describe, expect, it } from 'vitest';
import { rankByUsage } from './ranking.js';

const items = [
	{ id: 1, value: 'Чехол' },
	{ id: 2, value: 'Коробка' },
	{ id: 3, value: 'Шнур' }
];

describe('rankByUsage', () => {
	it('the most used first, the unused after them in the order of the dictionary', () => {
		expect(
			rankByUsage(items, [
				{ id: 3, count: 5 },
				{ id: 2, count: 9 }
			])
		).toEqual(['Коробка', 'Шнур', 'Чехол']);
	});

	it('without counts the order of the dictionary', () => {
		expect(rankByUsage(items, [])).toEqual(['Чехол', 'Коробка', 'Шнур']);
	});

	it('equal counts keep the order of the dictionary', () => {
		expect(
			rankByUsage(items, [
				{ id: 3, count: 2 },
				{ id: 1, count: 2 }
			])
		).toEqual(['Чехол', 'Шнур', 'Коробка']);
	});
});
