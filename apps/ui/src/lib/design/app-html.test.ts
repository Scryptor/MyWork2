import { afterEach, describe, expect, it, vi } from 'vitest';
import html from '../../app.html?raw';
import { applyDisplay, computeDisplay } from './display.js';
import { parsePrefs } from './prefs.svelte.js';

const script = /<script>([\s\S]*?)<\/script>/.exec(html)?.[1];

interface Screen3 {
	sw: number;
	sh: number;
	ww: number;
}
const SCREENS: Screen3[] = [
	{ sw: 1920, sh: 1080, ww: 1920 },
	{ sw: 3840, sh: 2160, ww: 1920 },
	{ sw: 1366, sh: 768, ww: 1366 }
];

function run(
	raw: string | null,
	systemDark: boolean,
	throwing = false,
	scr: Screen3 = { sw: 1920, sh: 1080, ww: 1920 }
): HTMLElement {
	const root = document.documentElement;
	root.removeAttribute('data-theme');
	root.removeAttribute('data-density');
	root.removeAttribute('data-width');
	root.removeAttribute('style');
	vi.stubGlobal(
		'localStorage',
		throwing
			? {
					getItem: () => {
						throw new Error('denied');
					}
				}
			: { getItem: () => raw }
	);
	vi.stubGlobal('matchMedia', () => ({ matches: systemDark }));
	vi.stubGlobal('screen', { width: scr.sw, height: scr.sh });
	vi.stubGlobal('innerWidth', scr.ww);
	new Function(script!)();
	return root;
}

afterEach(() => vi.unstubAllGlobals());

describe('app.html inline script', () => {
	it('exists', () => {
		expect(script).toBeTruthy();
	});
	const raws: (string | null)[] = [
		null,
		'{bad',
		'{"theme":"dark"}',
		'{"theme":"system"}',
		'{"scale":1.25,"density":"compact"}',
		'{"v":2,"scale":1.25,"density":"compact"}',
		'{"v":2,"scale":"auto"}',
		'{"v":2,"scale":0.9375}'
	];
	for (const raw of raws)
		for (const dark of [true, false])
			for (const scr of SCREENS) {
				it(`matches applyDisplay for ${String(raw)} dark=${dark} ${scr.sw}x${scr.sh}/${scr.ww}`, () => {
					const got = run(raw, dark, false, scr);
					const expected = document.createElement('html');
					applyDisplay(
						computeDisplay(parsePrefs(raw), {
							systemDark: dark,
							screenWidth: scr.sw,
							screenHeight: scr.sh,
							windowWidth: scr.ww
						}),
						expected
					);
					const pick = (el: HTMLElement) => ({
						theme: el.getAttribute('data-theme'),
						density: el.getAttribute('data-density'),
						width: el.getAttribute('data-width'),
						scale: el.style.getPropertyValue('--ui-scale')
					});
					expect(pick(got)).toEqual(pick(expected));
				});
			}
	it('does not throw when localStorage throws', () => {
		expect(() => run(null, false, true)).not.toThrow();
		const root = document.documentElement;
		expect(root.getAttribute('data-theme')).toBe('light');
		expect(root.getAttribute('data-density')).toBe('comfortable');
		expect(root.getAttribute('data-width')).toBe('xl');
		expect(root.style.getPropertyValue('--ui-scale')).toBe('1');
	});
});
