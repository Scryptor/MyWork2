import { describe, expect, it } from 'vitest';
import { rangeSelect } from './selection.js';

describe('rangeSelect', () => {
	it('selects from the anchor down to the target', () => {
		expect(rangeSelect([1, 2, 3, 4, 5], 2, 4)).toEqual([2, 3, 4]);
	});

	it('selects from the anchor up to the target, in list order', () => {
		expect(rangeSelect([1, 2, 3, 4, 5], 4, 2)).toEqual([2, 3, 4]);
	});

	it('anchor not in the list: only the target', () => {
		expect(rangeSelect([1, 2, 3, 4, 5], 9, 3)).toEqual([3]);
	});

	it('anchor equal to the target: only the target', () => {
		expect(rangeSelect(['a', 'b', 'c'], 'b', 'b')).toEqual(['b']);
	});

	it('target not in the list: nothing', () => {
		expect(rangeSelect([1, 2, 3], 1, 9)).toEqual([]);
	});
});
