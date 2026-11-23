import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createHistory } from './history.svelte.js';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createHistory', () => {
	it('pushes, undoes and redoes', () => {
		const h = createHistory(0);
		expect([h.canUndo, h.canRedo]).toEqual([false, false]);
		h.push(1);
		h.push(2);
		expect(h.current).toBe(2);
		expect(h.undo()).toBe(1);
		expect(h.undo()).toBe(0);
		expect(h.undo()).toBeUndefined();
		expect(h.canRedo).toBe(true);
		expect(h.redo()).toBe(1);
		expect(h.redo()).toBe(2);
		expect(h.redo()).toBeUndefined();
		expect(h.current).toBe(2);
	});

	it('a push after undo drops the redo branch', () => {
		const h = createHistory(0);
		h.push(1);
		h.push(2);
		h.undo();
		h.push(9);
		expect(h.canRedo).toBe(false);
		expect(h.undo()).toBe(1);
	});

	it('the 101st entry pushes out the first', () => {
		const h = createHistory(0);
		for (let i = 1; i <= 101; i++) h.push(i);
		let last: number | undefined;
		let steps = 0;
		for (let v = h.undo(); v !== undefined; v = h.undo()) {
			last = v;
			steps++;
		}
		expect(steps).toBe(100);
		expect(last).toBe(1);
	});

	it('the same key within 1000 ms is one entry, later it is two', () => {
		const h = createHistory('a');
		h.push('b', 'blocks.0.text');
		vi.advanceTimersByTime(500);
		h.push('c', 'blocks.0.text');
		expect(h.undo()).toBe('a');

		const g = createHistory('a');
		g.push('b', 'blocks.0.text');
		vi.advanceTimersByTime(1500);
		g.push('c', 'blocks.0.text');
		expect(g.undo()).toBe('b');
		expect(g.undo()).toBe('a');
	});

	it('different keys, or no key, do not merge', () => {
		const h = createHistory(0);
		h.push(1, 'x');
		h.push(2, 'y');
		h.push(3);
		h.push(4);
		expect(h.undo()).toBe(3);
		expect(h.undo()).toBe(2);
		expect(h.undo()).toBe(1);
		expect(h.undo()).toBe(0);
	});

	it('reset starts over', () => {
		const h = createHistory(0);
		h.push(1);
		h.reset(5);
		expect(h.current).toBe(5);
		expect([h.canUndo, h.canRedo]).toEqual([false, false]);
	});

	it('a push exactly mergeMs after the last one still merges, one millisecond later it does not', () => {
		const at = (gap: number) => {
			const h = createHistory<number>(0, { mergeMs: 1000 });
			h.push(1, 'k');
			vi.advanceTimersByTime(10);
			h.push(2, 'k');
			vi.advanceTimersByTime(gap);
			h.push(3, 'k');
			return h;
		};
		// Merged: 0 → 3, one step; not merged: 0 → 2 → 3.
		expect(at(1000).undo()).toBe(0);
		expect(at(1001).undo()).toBe(2);
	});
});
