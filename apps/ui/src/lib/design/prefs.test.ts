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

describe('helpers', () => {
	it('statusHue', () => {
		expect(statusHue(' Amber ')).toBe('amber');
		expect(statusHue('nope')).toBe('gray');
	});
	it('contrastRatio parses rgb()', () => {
		expect(contrastRatio('rgb(0, 0, 0)', '#ffffff')).toBeCloseTo(21, 5);
	});
});
