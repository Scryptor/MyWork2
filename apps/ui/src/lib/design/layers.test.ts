import { describe, expect, it, vi } from 'vitest';
import { layers } from './layers.svelte.js';

function escape() {
	window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', code: 'Escape' }));
}

describe('layers', () => {
	it('starts empty', () => {
		expect(layers.topKind).toBeNull();
		expect(layers.blocking).toBe(false);
	});

	it('popup above panel blocks Escape; after it closes, Escape closes the panel once', () => {
		const closeA = vi.fn();
		const closeB = vi.fn();
		const offA = layers.open('panel', closeA);
		const offB = layers.open('popup', closeB);
		expect(layers.topKind).toBe('popup');
		expect(layers.blocking).toBe(true);
		escape();
		expect(closeA).not.toHaveBeenCalled();
		expect(closeB).not.toHaveBeenCalled();
		offB();
		expect(layers.topKind).toBe('panel');
		expect(layers.blocking).toBe(false);
		escape();
		expect(closeA).toHaveBeenCalledTimes(1);
		offA();
		expect(layers.topKind).toBeNull();
	});

	it('modal blocks too', () => {
		const off = layers.open('modal', () => {});
		expect(layers.blocking).toBe(true);
		off();
	});
});
