import { describe, expect, it } from 'vitest';
import { pageSelection } from './pages.js';

// The pages checked in the preview become `pages` of the document's request: «2-3,5». All of them is
// the whole document (no parameter), none of them is nothing to print.

describe('pageSelection', () => {
	it('every page checked is the whole document: no selection', () => {
		expect(pageSelection([true, true])).toBeNull();
		expect(pageSelection([true])).toBeNull();
	});

	it('a page alone is its number', () => {
		expect(pageSelection([true, false])).toBe('1');
		expect(pageSelection([false, true])).toBe('2');
	});

	it('pages in a row are a range, the rest go by commas', () => {
		expect(pageSelection([false, true, true, false, true])).toBe('2-3,5');
		expect(pageSelection([true, true, true, false, true, true])).toBe('1-3,5-6');
	});

	it('nothing checked is nothing to print', () => {
		expect(pageSelection([false, false])).toBe('');
	});
});
