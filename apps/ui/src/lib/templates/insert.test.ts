import { describe, expect, it } from 'vitest';
import { insertAtCaret } from './insert.js';

describe('insertAtCaret', () => {
	it('puts the text in the middle', () => {
		expect(insertAtCaret('abcd', { start: 2, end: 2 }, '{x}')).toEqual({
			value: 'ab{x}cd',
			caret: 5
		});
	});
	it('puts the text at the end and at the start', () => {
		expect(insertAtCaret('ab', { start: 2, end: 2 }, '{x}')).toEqual({ value: 'ab{x}', caret: 5 });
		expect(insertAtCaret('ab', { start: 0, end: 0 }, '{x}')).toEqual({ value: '{x}ab', caret: 3 });
	});
	it('replaces the selection', () => {
		expect(insertAtCaret('abcd', { start: 1, end: 3 }, '{x}')).toEqual({
			value: 'a{x}d',
			caret: 4
		});
	});
	it('cuts a selection out of the text to the text', () => {
		expect(insertAtCaret('ab', { start: 9, end: 12 }, '!')).toEqual({ value: 'ab!', caret: 3 });
	});
});
