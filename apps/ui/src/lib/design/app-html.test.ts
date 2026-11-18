import { afterEach, describe, expect, it, vi } from 'vitest';
import html from '../../app.html?raw';
import { applyDisplay, autoScale, bandForWidth, computeDisplay, UI_SCALES } from './display.js';
import { parsePrefs } from './prefs.svelte.js';

const script = /<script>([\s\S]*?)<\/script>/.exec(html)?.[1];

interface Screen3 {
	sw: number;
	sh: number;
	ww: number;
}
// Every step of «Авто» (1920 → 100 %, 2560 → 110 %, 3008 → 125 %, 3840 → 150 %) and, with the stored
// sizes, every band of the width (500, 700, 950, 1100 and wider).
const SCREENS: Screen3[] = [
	{ sw: 1920, sh: 1080, ww: 1920 },
	{ sw: 3840, sh: 2160, ww: 1920 },
	{ sw: 1366, sh: 768, ww: 1366 },
	{ sw: 2560, sh: 1440, ww: 2560 },
	{ sw: 3008, sh: 1692, ww: 3008 },
	{ sw: 1920, sh: 1080, ww: 500 },
	{ sw: 1920, sh: 1080, ww: 700 },
	{ sw: 1920, sh: 1080, ww: 950 },
	{ sw: 1920, sh: 1080, ww: 1100 }
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
	it('the screens below reach every «Авто» step and, «Авто», every width band', () => {
		const steps = new Set(SCREENS.map((s) => autoScale(s.sw, s.sh)));
		expect([...steps].sort((a, b) => a - b)).toEqual(UI_SCALES.filter((s) => s >= 1));
		const bands = new Set(SCREENS.map((s) => bandForWidth(s.ww / autoScale(s.sw, s.sh))));
		expect([...bands].sort()).toEqual(['lg', 'md', 'sm', 'xl', 'xs']);
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
