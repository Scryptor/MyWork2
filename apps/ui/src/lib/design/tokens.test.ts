import { describe, expect, it } from 'vitest';
import css from './tokens.css?raw';
import { contrastRatio } from './contrast.js';
import { STATUS_HUES } from './status-colors.js';

function blockBody(source: string, selector: string): string {
	const start = source.indexOf(selector + ' {');
	if (start < 0) throw new Error(`no block ${selector}`);
	const open = source.indexOf('{', start);
	const close = source.indexOf('\n}', open);
	return source.slice(open + 1, close);
}

function parseVars(body: string): Map<string, string> {
	const map = new Map<string, string>();
	for (const m of body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)) {
		map.set(m[1]!, m[2]!.trim());
	}
	return map;
}

const light = parseVars(blockBody(css, ':root'));
const dark = parseVars(blockBody(css, "[data-theme='dark']"));

function get(map: Map<string, string>, name: string): string {
	const v = map.get(name);
	if (v === undefined) throw new Error(`missing ${name}`);
	return v;
}

const themes = [
	['light', light],
	['dark', dark]
] as const;

describe('tokens: contrast pairs of spec 3.1', () => {
	for (const [name, map] of themes) {
		it(`text on backgrounds >= 4.5 (${name})`, () => {
			for (const fg of ['--text', '--text-2', '--text-3']) {
				for (const bg of ['--bg', '--surface', '--surface-2']) {
					expect(
						contrastRatio(get(map, fg), get(map, bg)),
						`${fg} on ${bg}`
					).toBeGreaterThanOrEqual(4.5);
				}
			}
		});
		it(`accent, danger and soft pairs >= 4.5 (${name})`, () => {
			const pairs: [string, string][] = [
				['--on-accent', '--accent'],
				['--on-danger', '--danger'],
				['--accent-text', '--accent-soft'],
				['--danger-text', '--danger-soft'],
				['--warning-text', '--warning-soft'],
				['--success-text', '--success-soft'],
				['--info-text', '--info-soft'],
				['--accent', '--surface']
			];
			for (const [fg, bg] of pairs) {
				expect(contrastRatio(get(map, fg), get(map, bg)), `${fg} on ${bg}`).toBeGreaterThanOrEqual(
					4.5
				);
			}
		});
		it(`border-strong on surface >= 1.5 (${name})`, () => {
			expect(
				contrastRatio(get(map, '--border-strong'), get(map, '--surface'))
			).toBeGreaterThanOrEqual(1.5);
		});
		it(`every status hue has bg/fg/dot, fg on bg >= 4.5 (${name})`, () => {
			for (const h of STATUS_HUES) {
				const bg = get(map, `--st-${h}-bg`);
				const fg = get(map, `--st-${h}-fg`);
				get(map, `--st-${h}-dot`);
				expect(contrastRatio(fg, bg), h).toBeGreaterThanOrEqual(4.5);
			}
		});
	}
});

describe('tokens: values match the spec', () => {
	it('has spec values', () => {
		expect(get(light, '--accent')).toBe('#0b7a6d');
		expect(get(dark, '--accent')).toBe('#2bb3a0');
		expect(get(light, '--bg')).toBe('#f4f3f1');
		expect(get(light, '--focus')).toBe('#0e8a7b');
		expect(get(light, '--st-amber-fg')).toBe('#7f5300');
		expect(get(dark, '--st-pink-bg')).toBe('#3a1a2b');
		expect(get(light, '--scrim')).toBe('rgb(34 32 28 / 0.28)');
		expect(get(light, '--space-4')).toBe('1rem');
		expect(get(light, '--z-tooltip')).toBe('80');
	});
	it('white on #0e8a7b justifies #0b7a6d', () => {
		const r = contrastRatio('#ffffff', '#0e8a7b');
		expect(r).toBeGreaterThan(4.24);
		expect(r).toBeLessThan(4.26);
	});
	it('reduced motion zeroes durations', () => {
		expect(css).toMatch(/prefers-reduced-motion: reduce[\s\S]*--duration-fast: 0\.01ms/);
	});
});
