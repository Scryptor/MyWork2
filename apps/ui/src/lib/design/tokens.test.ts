import { describe, expect, it } from 'vitest';
import css from './tokens.css?raw';
import { contrastRatio } from './contrast.js';
import { STATUS_HUES } from './status-colors.js';
import { ROW_REM } from '../ui/table/sizes.js';

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

const light = parseVars(blockBody(css, ":root,\n[data-theme='light']"));
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
		it(`text on a status filled with its dot >= 4.5 (${name})`, () => {
			const fg = get(map, '--st-on-dot');
			for (const h of STATUS_HUES) {
				expect(contrastRatio(fg, get(map, `--st-${h}-dot`)), h).toBeGreaterThanOrEqual(4.5);
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

describe('tokens: denser defaults (spec 3.3, 3.4, decision 48)', () => {
	it('has the new sizes', () => {
		const want: Record<string, string> = {
			'--text-xs': '0.6875rem',
			'--text-sm': '0.75rem',
			'--text-md': '0.8125rem',
			'--text-lg': '0.9375rem',
			'--text-xl': '1.125rem',
			'--text-2xl': '1.375rem',
			'--control-sm': '1.5rem',
			'--control-md': '1.875rem',
			'--control-lg': '2.25rem',
			'--row-height': '2rem',
			'--topbar-height': '2.75rem',
			'--sidebar-width': '13rem',
			'--rail-width': '3rem',
			'--topbar-named-max': '14rem',
			'--panel-width': '28rem',
			'--sheet-width': '64rem'
		};
		for (const [name, value] of Object.entries(want)) expect(get(light, name), name).toBe(value);
	});
	it('keeps the coarse-pointer controls at least 44px, also at 90 %', () => {
		const coarse = parseVars(blockBody(css, '@media (pointer: coarse)'));
		for (const name of ['--control-sm', '--control-md', '--control-lg'])
			expect(get(coarse, name), name).toBe('var(--touch-target)');
		expect(get(light, '--touch-target')).toBe('max(2.75rem, 44px)');
		// The narrow menu stays wider than its touch-sized icons.
		expect(get(coarse, '--rail-width')).toBe('max(3rem, calc(var(--control-lg) + var(--space-1)))');
	});
	it('ROW_REM matches --row-height of both densities', () => {
		const compact = parseVars(blockBody(css, ":root[data-density='compact']"));
		expect(`${ROW_REM.comfortable}rem`).toBe(get(light, '--row-height'));
		expect(`${ROW_REM.compact}rem`).toBe(get(compact, '--row-height'));
	});
});

describe('tokens: sizes live only here', () => {
	const files = import.meta.glob('/src/**/*.{svelte,css}', {
		query: '?raw',
		import: 'default',
		eager: true
	}) as Record<string, string>;
	const strip = (s: string) => s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/<!--[\s\S]*?-->/g, '');
	const outside = Object.entries(files).filter(([p]) => p !== '/src/lib/design/tokens.css');

	it('has no 2.75rem literal outside tokens.css: touch sizes come from --control-*', () => {
		expect(outside.length).toBeGreaterThan(0);
		const bad = outside.filter(([, src]) => /2\.75rem/.test(strip(src))).map(([p]) => p);
		expect(bad).toEqual([]);
	});
	it('has no literal widths of the top bar name, side panel and sheet in components', () => {
		const bad = outside
			.filter(([p]) => /\/(AppTopBar|SlideOver|Sheet)\.svelte$/.test(p))
			.filter(([, src]) => /\b(?:14|28|64)rem\b/.test(strip(src)))
			.map(([p]) => p);
		expect(bad).toEqual([]);
	});
	// The unconditional finger sizes: the phone-only parts, whatever the pointer (a mouse on a narrow window).
	it('uses --touch-target unconditionally only in the status choice rows and the bottom navigation', () => {
		const users = outside.filter(([, src]) => /--touch-target/.test(strip(src))).map(([p]) => p);
		expect(users.sort()).toEqual([
			'/src/lib/ui/StatusPicker.svelte',
			'/src/lib/ui/shell/BottomNav.svelte'
		]);
		const picker = strip(files['/src/lib/ui/StatusPicker.svelte']!);
		const rule = (sel: string) => new RegExp(`\\n\\t${sel} \\{[^}]*\\}`).exec(picker)?.[0] ?? '';
		expect(rule('\\.choice')).toContain('min-height: var(--touch-target)');
		expect(rule('\\.trigger')).toContain('min-height: var(--control-md)');
		const nav = strip(files['/src/lib/ui/shell/BottomNav.svelte']!);
		expect(/\n\t\.item \{[^}]*min-height: var\(--touch-target\)/.test(nav)).toBe(true);
	});
	it('styles no global .btn.named', () => {
		const bad = outside.filter(([, src]) => /\.btn\.named/.test(strip(src))).map(([p]) => p);
		expect(bad).toEqual([]);
	});
});
