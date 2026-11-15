import { afterEach, describe, expect, it, vi } from 'vitest';
import html from '../../app.html?raw';
import { applyPrefs, parsePrefs } from './prefs.svelte.js';

const script = /<script>([\s\S]*?)<\/script>/.exec(html)?.[1];

function run(raw: string | null, systemDark: boolean, throwing = false): HTMLElement {
	const root = document.documentElement;
	root.removeAttribute('data-theme');
	root.removeAttribute('data-density');
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
	new Function(script!)();
	return root;
}

afterEach(() => vi.unstubAllGlobals());

describe('app.html inline script', () => {
	it('exists', () => {
		expect(script).toBeTruthy();
	});
	const cases: [string | null, boolean][] = [
		[null, false],
		['{bad', false],
		['{"theme":"dark"}', false],
		['{"theme":"system"}', true],
		['{"scale":1.25,"density":"compact"}', false]
	];
	for (const [raw, dark] of cases) {
		it(`matches applyPrefs for ${String(raw)} (system dark=${dark})`, () => {
			const got = run(raw, dark);
			const actual = {
				theme: got.getAttribute('data-theme'),
				density: got.getAttribute('data-density'),
				scale: got.style.getPropertyValue('--ui-scale')
			};
			const expected = document.createElement('html');
			applyPrefs(parsePrefs(raw), expected, dark);
			expect(actual).toEqual({
				theme: expected.getAttribute('data-theme'),
				density: expected.getAttribute('data-density'),
				scale: expected.style.getPropertyValue('--ui-scale')
			});
		});
	}
	it('does not throw when localStorage throws', () => {
		expect(() => run(null, false, true)).not.toThrow();
	});
});
