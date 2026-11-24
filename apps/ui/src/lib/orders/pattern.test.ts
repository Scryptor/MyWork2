import { describe, expect, it } from 'vitest';
import { addPoint, isPattern, patternReady, patternText } from './pattern.js';

describe('addPoint', () => {
	it('adds a point at the end', () => {
		expect(addPoint('', 5)).toBe('5');
		expect(addPoint('15', 6)).toBe('156');
	});

	it('a step over a point in a line adds that point first, as on Android', () => {
		// A row, a diagonal, a column, the other diagonal.
		expect(addPoint('1', 3)).toBe('123');
		expect(addPoint('1', 9)).toBe('159');
		expect(addPoint('2', 8)).toBe('258');
		expect(addPoint('7', 3)).toBe('753');
	});

	it('the point in between that is already there is not added again', () => {
		expect(addPoint('21', 3)).toBe('213');
	});

	it('a step to a neighbour or a knight step has no point in between', () => {
		expect(addPoint('1', 6)).toBe('16');
		expect(addPoint('1', 8)).toBe('18');
		expect(addPoint('5', 9)).toBe('59');
	});

	it('a point already there or not on the grid changes nothing', () => {
		expect(addPoint('5', 5)).toBe('5');
		expect(addPoint('159', 1)).toBe('159');
		for (const point of [0, 10, 1.5, -1]) expect(addPoint('1', point), String(point)).toBe('1');
	});
});

describe('patternText', () => {
	it('the points with dots between them, as in the documents', () => {
		expect(patternText('1236')).toBe('1 · 2 · 3 · 6');
		expect(patternText('5')).toBe('5');
		expect(patternText('')).toBe('');
	});
});

describe('patternReady', () => {
	it('a pattern is ready from four points', () => {
		expect(patternReady('')).toBe(false);
		expect(patternReady('123')).toBe(false);
		expect(patternReady('1236')).toBe(true);
		expect(patternReady('123456789')).toBe(true);
	});
});

describe('isPattern', () => {
	it('empty, or four to nine points 1–9 without repeats: what the server takes', () => {
		for (const ok of ['', '1596', '123456789']) expect(isPattern(ok), ok).toBe(true);
		for (const bad of ['158', '1123', '1580', 'a123', '1234567891', ' 1596'])
			expect(isPattern(bad), bad).toBe(false);
	});
});
