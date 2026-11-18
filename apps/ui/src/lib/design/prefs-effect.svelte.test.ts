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

it('stop() removes every listener it added', () => {
	stubStorage();
	const { added, removed } = stubMatchMedia();
	const screenObj = { addEventListener: vi.fn(), removeEventListener: vi.fn() };
	vi.stubGlobal('screen', Object.assign(screenObj, { width: 1920, height: 1080 }));
	const addWin = vi.spyOn(window, 'addEventListener');
	const removeWin = vi.spyOn(window, 'removeEventListener');
	const stop = prefs.init();
	const winAdded = addWin.mock.calls
		.map((c) => c[0])
		.filter((t) => t === 'resize' || t === 'focus');
	expect(winAdded.sort()).toEqual(['focus', 'resize']);
	expect(screenObj.addEventListener).toHaveBeenCalledWith('change', expect.any(Function));
	stop();
	const winRemoved = removeWin.mock.calls.map((c) => c[0]);
	expect(winRemoved).toContain('resize');
	expect(winRemoved).toContain('focus');
	expect(screenObj.removeEventListener).toHaveBeenCalledWith('change', expect.any(Function));
	expect(removed['(prefers-color-scheme: dark)']).toBe(1);
	const res = Object.keys(added).find((q) => q.startsWith('(resolution'));
	expect(res).toBeDefined();
	expect(removed[res!]).toBe(1);
	addWin.mockRestore();
	removeWin.mockRestore();
});

it('a screen change event updates data-width and --ui-scale', async () => {
	stubStorage();
	stubMatchMedia();
	const listeners: (() => void)[] = [];
	const screenObj = {
		width: 1920,
		height: 1080,
		addEventListener: (_t: string, fn: () => void) => listeners.push(fn),
		removeEventListener: () => {}
	};
	vi.stubGlobal('screen', screenObj);
	vi.stubGlobal('innerWidth', 1920);
	const stop = prefs.init();
	expect(root().style.getPropertyValue('--ui-scale')).toBe('1');
	expect(root().getAttribute('data-width')).toBe('xl');

	screenObj.width = 3840;
	screenObj.height = 2160;
	for (const fn of listeners) fn();
	await nextFrame();
	expect(root().style.getPropertyValue('--ui-scale')).toBe('1.5');
	expect(root().getAttribute('data-width')).toBe('xl'); // 1920 / 1.5 = 1280
	vi.stubGlobal('innerWidth', 1800);
	for (const fn of listeners) fn();
	await nextFrame();
	expect(root().getAttribute('data-width')).toBe('lg'); // 1200
	stop();
});

it('a resolution change re-subscribes to the query for the new ratio', async () => {
	stubStorage();
	const handlers: Record<string, () => void> = {};
	const added: string[] = [];
	const removed: string[] = [];
	vi.stubGlobal('matchMedia', (q: string) => ({
		matches: false,
		addEventListener: (_t: string, fn: () => void) => {
			added.push(q);
			handlers[q] = fn;
		},
		removeEventListener: () => removed.push(q)
	}));
	vi.stubGlobal('devicePixelRatio', 1);
	vi.stubGlobal('innerWidth', 1280);
	const stop = prefs.init();
	expect(added).toContain('(resolution: 1dppx)');

	vi.stubGlobal('devicePixelRatio', 2);
	handlers['(resolution: 1dppx)']!();
	expect(removed).toContain('(resolution: 1dppx)');
	expect(added).toContain('(resolution: 2dppx)');
	await nextFrame();
	stop();
	expect(removed).toContain('(resolution: 2dppx)');
});

it('an unchanged environment keeps the same display object', async () => {
	stubStorage();
	stubMatchMedia();
	vi.stubGlobal('innerWidth', 1280);
	vi.stubGlobal('screen', { width: 1920, height: 1080 });
	const stop = prefs.init();
	const before = display.value;
	window.dispatchEvent(new Event('focus'));
	await nextFrame();
	expect(display.value).toBe(before);
	vi.stubGlobal('innerWidth', 1000);
	window.dispatchEvent(new Event('resize'));
	await nextFrame();
	expect(display.value).not.toBe(before);
	stop();
});
