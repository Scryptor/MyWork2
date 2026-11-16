import { afterEach, describe, expect, it } from 'vitest';
import { RECENT_KEY, pushRecent, recentIds } from './recent.js';

afterEach(() => localStorage.clear());

describe('recent orders', () => {
	it('keeps 8 unique ids, newest first', () => {
		for (const id of [1, 2, 3, 4, 5, 6, 7, 8, 9]) pushRecent(id);
		pushRecent(5);
		expect(recentIds()).toEqual([5, 9, 8, 7, 6, 4, 3, 2]);
		expect(JSON.parse(localStorage.getItem(RECENT_KEY)!)).toEqual([5, 9, 8, 7, 6, 4, 3, 2]);
	});

	it('ignores a damaged record', () => {
		localStorage.setItem(RECENT_KEY, '{"x":1}');
		expect(recentIds()).toEqual([]);
		localStorage.setItem(RECENT_KEY, '[3, "x", -1, 3, 2.5, 4]');
		expect(recentIds()).toEqual([3, 4]);
		localStorage.setItem(RECENT_KEY, 'not json');
		expect(recentIds()).toEqual([]);
	});
});
