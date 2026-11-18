import { afterEach, describe, expect, it, vi } from 'vitest';
import { scaleLabel } from './size.js';

describe('scaleLabel', () => {
	it('writes percents', () => {
		expect(scaleLabel(1.1)).toBe('110 %');
		expect(scaleLabel(0.9)).toBe('90 %');
		expect(scaleLabel(1.25)).toBe('125 %');
	});
});

/**
 * Fresh modules on a screen of this size (the store reads it when it loads) with these stored prefs:
 * the size keys, the store and the toasts.
 */
async function on(screen: { width: number; height: number }, stored: object = { v: 2 }) {
	vi.stubGlobal('screen', screen);
	vi.stubGlobal('innerWidth', screen.width);
	localStorage.setItem('mw.prefs', JSON.stringify(stored));
	vi.resetModules();
	const size = await import('./size.js');
	const { prefs, display } = await import('#lib/design/prefs.svelte.js');
	const { toast } = await import('#lib/ui/toast/toast.svelte.js');
	const messages = () => toast.items.map((t) => t.message);
	return { ...size, prefs, display, messages };
}

afterEach(() => {
	vi.unstubAllGlobals();
	vi.resetModules();
	localStorage.clear();
});

describe('size keys', () => {
	it('step from «Авто» from the size it gives on this screen', async () => {
		// 2560×1440: «Авто» is 110 %, one step up is 125 %, not 110 %.
		const s = await on({ width: 2560, height: 1440 });
		expect(s.prefs.value.scale).toBe('auto');
		s.stepSize(1);
		expect(s.prefs.value.scale).toBe(1.25);
		expect(s.messages()).toEqual(['Размер 125 %']);
	});

	it('at the end «Авто» stays «Авто» and the toast says so', async () => {
		// 4K: «Авто» is already 150 %, the largest size.
		const s = await on({ width: 3840, height: 2160 });
		s.stepSize(1);
		expect(s.prefs.value.scale).toBe('auto');
		expect(s.display.value.scale).toBe(1.5);
		expect(s.messages()).toEqual(['Размер: авто (150 %)']);
	});

	it('a manual size stops at the end', async () => {
		const s = await on({ width: 1920, height: 1080 }, { v: 2, scale: 0.9 });
		s.stepSize(-1);
		expect(s.prefs.value.scale).toBe(0.9);
		expect(s.messages()).toEqual(['Размер 90 %']);
		s.stepSize(1);
		expect(s.prefs.value.scale).toBe(1);
	});

	it('a new size toast replaces the previous one', async () => {
		const s = await on({ width: 1920, height: 1080 });
		s.stepSize(1);
		s.stepSize(1);
		expect(s.messages()).toEqual(['Размер 125 %']);
		s.autoSize();
		expect(s.prefs.value.scale).toBe('auto');
		expect(s.messages()).toEqual(['Размер: авто (100 %)']);
	});
});
