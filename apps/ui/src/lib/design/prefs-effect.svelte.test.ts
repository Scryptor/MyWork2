import { afterEach, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import { DEFAULT_PREFS, display, prefs } from './prefs.svelte.js';

afterEach(() => vi.unstubAllGlobals());

function stubMatchMedia() {
	const added: Record<string, number> = {};
	const removed: Record<string, number> = {};
	vi.stubGlobal('matchMedia', (q: string) => ({
		matches: false,
		addEventListener: () => (added[q] = (added[q] ?? 0) + 1),
		removeEventListener: () => (removed[q] = (removed[q] ?? 0) + 1)
	}));
	return { added, removed };
}

function stubStorage() {
	const getItem = vi.fn(() => {
		throw new Error('denied');
	});
	vi.stubGlobal('localStorage', {
		getItem,
		setItem: () => {
			throw new Error('denied');
		}
	});
	return getItem;
}

const nextFrame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
const root = () => document.documentElement;

it('init() inside $effect is not re-run by pref changes', () => {
	const getItem = stubStorage();
	const { added } = stubMatchMedia();

	const stop = $effect.root(() => {
		$effect(() => prefs.init());
	});
	flushSync();
	expect(prefs.value).toEqual(DEFAULT_PREFS);
	expect(getItem).toHaveBeenCalledTimes(1);

	prefs.set({ density: 'compact' });
	flushSync();
	expect(prefs.value.density).toBe('compact');
	expect(getItem).toHaveBeenCalledTimes(1);
	expect(added['(prefers-color-scheme: dark)']).toBe(1);
	stop();
	prefs.set({ density: 'comfortable' });
});

it('set() applies the scale and band synchronously', () => {
	stubStorage();
	stubMatchMedia();
	vi.stubGlobal('innerWidth', 1600);
	const stop = prefs.init();
	prefs.set({ scale: 1.25 });
	expect(root().style.getPropertyValue('--ui-scale')).toBe('1.25');
	expect(root().getAttribute('data-width')).toBe('xl'); // 1280
	expect(display.value.scale).toBe(1.25);
	stop();
	prefs.set({ scale: 'auto' });
});

it('environment events are coalesced and applied after a frame; stop() unsubscribes', async () => {
	stubStorage();
	stubMatchMedia();
	vi.stubGlobal('innerWidth', 1920);
	vi.stubGlobal('screen', { width: 1920, height: 1080 });
	const stop = prefs.init();
	expect(root().getAttribute('data-width')).toBe('xl');
	expect(root().style.getPropertyValue('--ui-scale')).toBe('1');
	expect(display.phone).toBe(false);
	expect(display.cardColumns).toBe(true);

	vi.stubGlobal('innerWidth', 500);
	vi.stubGlobal('screen', { width: 3840, height: 2160 });
	window.dispatchEvent(new Event('resize'));
	window.dispatchEvent(new Event('focus'));
	expect(root().getAttribute('data-width')).toBe('xl'); // not yet
	await nextFrame();
	expect(root().style.getPropertyValue('--ui-scale')).toBe('1.5');
	expect(root().getAttribute('data-width')).toBe('xs'); // 500 / 1.5
	expect(display.phone).toBe(true);
	expect(display.narrow).toBe(true);
	expect(display.cardColumns).toBe(false);

	stop();
	vi.stubGlobal('innerWidth', 1920);
	window.dispatchEvent(new Event('resize'));
	await nextFrame();
	expect(root().getAttribute('data-width')).toBe('xs');
});
