import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_PREFS, parsePrefs, prefs, PREFS_KEY } from './prefs.svelte.js';
import { statusHue } from './status-colors.js';
import { contrastRatio } from './contrast.js';

afterEach(() => {
	vi.unstubAllGlobals();
	localStorage.clear();
});

describe('parsePrefs', () => {
	it('defaults for null and garbage', () => {
		expect(parsePrefs(null)).toEqual(DEFAULT_PREFS);
		expect(parsePrefs('{bad')).toEqual(DEFAULT_PREFS);
		expect(parsePrefs('[]')).toEqual(DEFAULT_PREFS);
	});
	it('ignores scale without v: 2', () => {
		expect(parsePrefs('{"theme":"dark","scale":1}')).toEqual({
			theme: 'dark',
			scale: 'auto',
			density: 'comfortable'
		});
	});
	it('reads v: 2 scale', () => {
		expect(parsePrefs('{"v":2,"scale":1.25}').scale).toBe(1.25);
		expect(parsePrefs('{"v":2,"scale":0.9375}').scale).toBe('auto');
		expect(parsePrefs('{"v":2,"scale":"auto","density":"compact"}')).toEqual({
			theme: 'system',
			scale: 'auto',
			density: 'compact'
		});
	});
	it('replaces invalid fields only', () => {
		expect(parsePrefs('{"v":2,"theme":"dark","scale":3,"density":"x"}')).toEqual({
			theme: 'dark',
			scale: 'auto',
			density: 'comfortable'
		});
	});
});

describe('prefs store', () => {
	it('writes JSON to localStorage', () => {
		prefs.set({ theme: 'dark' });
		expect(JSON.parse(localStorage.getItem(PREFS_KEY)!)).toMatchObject({ theme: 'dark' });
		expect(prefs.value.theme).toBe('dark');
		prefs.set({ theme: 'system' });
	});
	it('stores a versioned object', () => {
		prefs.set({ scale: 1.1 });
		expect(JSON.parse(localStorage.getItem(PREFS_KEY)!)).toMatchObject({ v: 2, scale: 1.1 });
		prefs.set({ scale: 'auto' });
	});
	it('survives a throwing storage', () => {
		vi.stubGlobal('localStorage', {
			getItem: () => {
				throw new Error('denied');
			},
			setItem: () => {
				throw new Error('denied');
			}
		});
		let stop: () => void = () => {};
		expect(() => (stop = prefs.init())).not.toThrow();
		expect(prefs.value).toEqual(DEFAULT_PREFS);
		expect(() => prefs.set({ density: 'compact' })).not.toThrow();
		expect(prefs.value.density).toBe('compact');
		stop();
		prefs.set({ density: 'comfortable' });
	});
});

describe('the store before init()', () => {
	// The first render is already right: the store reads the window and the storage when it loads, not
	// only when the root layout's effect calls init().
	afterEach(() => vi.resetModules());

	it('starts from the stored prefs and the real window', async () => {
		localStorage.setItem(PREFS_KEY, JSON.stringify({ v: 2, scale: 1.25, density: 'compact' }));
		vi.stubGlobal('innerWidth', 700);
		vi.stubGlobal('screen', { width: 1366, height: 768 });
		vi.resetModules();
		const fresh = await import('./prefs.svelte.js');
		expect(fresh.prefs.value).toEqual({ theme: 'system', scale: 1.25, density: 'compact' });
		// 700 / 1.25 = 560: a phone.
		expect(fresh.display.value.band).toBe('xs');
		expect(fresh.display.phone).toBe(true);
	});

	it('«Авто» follows the screen it starts on', async () => {
		vi.stubGlobal('innerWidth', 1920);
		vi.stubGlobal('screen', { width: 3840, height: 2160 });
		vi.resetModules();
		const fresh = await import('./prefs.svelte.js');
		expect(fresh.display.value.scale).toBe(1.5);
		expect(fresh.display.value.band).toBe('xl');
	});

	it('a throwing storage starts from the defaults', async () => {
		vi.stubGlobal('localStorage', {
			getItem: () => {
				throw new Error('denied');
			}
		});
		vi.resetModules();
		const fresh = await import('./prefs.svelte.js');
		expect(fresh.prefs.value).toEqual(DEFAULT_PREFS);
	});
});

describe('helpers', () => {
	it('statusHue', () => {
		expect(statusHue(' Amber ')).toBe('amber');
		expect(statusHue('nope')).toBe('gray');
	});
	it('contrastRatio parses rgb()', () => {
		expect(contrastRatio('rgb(0, 0, 0)', '#ffffff')).toBeCloseTo(21, 5);
	});
});
