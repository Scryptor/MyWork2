import { afterEach, describe, expect, it, vi } from 'vitest';
import { applyPrefs, DEFAULT_PREFS, parsePrefs, prefs, PREFS_KEY } from './prefs.svelte.js';
import { layoutForWidth } from './breakpoints.js';
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
	it('replaces invalid fields only', () => {
		expect(parsePrefs('{"theme":"dark","scale":3,"density":"x"}')).toEqual({
			theme: 'dark',
			scale: 1,
			density: 'comfortable'
		});
	});
});

describe('applyPrefs', () => {
	it('resolves the theme and sets attributes', () => {
		const root = document.createElement('html');
		applyPrefs({ ...DEFAULT_PREFS, theme: 'system' }, root, true);
		expect(root.dataset.theme).toBe('dark');
		applyPrefs({ ...DEFAULT_PREFS, theme: 'light' }, root, true);
		expect(root.dataset.theme).toBe('light');
		applyPrefs({ theme: 'system', scale: 1.125, density: 'compact' }, root, false);
		expect(root.style.getPropertyValue('--ui-scale')).toBe('1.125');
		expect(root.dataset.density).toBe('compact');
		expect(root.dataset.theme).toBe('light');
	});
});

describe('prefs store', () => {
	it('writes JSON to localStorage', () => {
		prefs.set({ theme: 'dark' });
		expect(JSON.parse(localStorage.getItem(PREFS_KEY)!)).toMatchObject({ theme: 'dark' });
		expect(prefs.value.theme).toBe('dark');
		prefs.set({ theme: 'system' });
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
	it('layoutForWidth', () => {
		expect(layoutForWidth(599)).toBe('phone');
		expect(layoutForWidth(600)).toBe('rail');
		expect(layoutForWidth(1279)).toBe('rail');
		expect(layoutForWidth(1280)).toBe('full');
	});
	it('statusHue', () => {
		expect(statusHue(' Amber ')).toBe('amber');
		expect(statusHue('nope')).toBe('gray');
	});
	it('contrastRatio parses rgb()', () => {
		expect(contrastRatio('rgb(0, 0, 0)', '#ffffff')).toBeCloseTo(21, 5);
	});
});
