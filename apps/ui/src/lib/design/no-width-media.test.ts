import { expect, it } from 'vitest';

const styles = import.meta.glob('/src/**/*.{svelte,css}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;
const scripts = import.meta.glob('/src/**/*.{svelte,ts}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

function stripComments(s: string): string {
	return s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/<!--[\s\S]*?-->/g, '');
}

// The layout follows the UI scale: widths come from `display` and `[data-width]`, never from the window.
it('has no width media queries in styles', () => {
	expect(Object.keys(styles).length).toBeGreaterThan(0);
	const bad = Object.entries(styles)
		.filter(([, src]) => /@media[^{]*\b(?:min|max)?-?width\b/.test(stripComments(src)))
		.map(([p]) => p);
	expect(bad).toEqual([]);
});

it('does not build MediaQuery objects', () => {
	const bad = Object.entries(scripts)
		.filter(([p]) => p !== '/src/lib/design/no-width-media.test.ts')
		.filter(([, src]) => /new\s+MediaQuery\s*\(/.test(stripComments(src)))
		.map(([p]) => p);
	expect(bad).toEqual([]);
});

// Every matchMedia call takes one of these literals: a variable or any other string could hide a width query.
const ALLOWED_QUERIES = ['(prefers-color-scheme: dark)', '`(resolution: ${resDppx}dppx)`'];

/** The arguments of the matchMedia calls that are not one of the allowed literals. */
export function badMatchMediaCalls(src: string): string[] {
	const bad: string[] = [];
	for (const m of src.matchAll(/matchMedia\(\s*('[^']*'|"[^"]*"|`[^`]*`|[^)]*?)\s*\)/g)) {
		const arg = m[1]!.replace(/^(['"])(.*)\1$/, "'$2'");
		if (!ALLOWED_QUERIES.some((q) => q === arg || `'${q}'` === arg)) bad.push(m[1]!);
	}
	return bad;
}

it('calls matchMedia only with the allowed literal queries (a variable counts as a violation)', () => {
	const bad = Object.entries(scripts)
		.filter(([p]) => !/\.test\.ts$/.test(p))
		.filter(([, src]) => badMatchMediaCalls(stripComments(src)).length > 0)
		.map(([p]) => p);
	expect(bad).toEqual([]);
});

it('the matchMedia check catches what it should', () => {
	expect(badMatchMediaCalls("window.matchMedia('(min-width: 600px)')")).toHaveLength(1);
	expect(badMatchMediaCalls('matchMedia(`(max-height: ${h}px)`)')).toHaveLength(1);
	expect(badMatchMediaCalls('matchMedia(query)')).toEqual(['query']);
	expect(badMatchMediaCalls('matchMedia(q + "(min-width: 1px)")')).toHaveLength(1);
	expect(badMatchMediaCalls("matchMedia('(prefers-color-scheme: dark)')")).toEqual([]);
	expect(badMatchMediaCalls('matchMedia(`(resolution: ${resDppx}dppx)`)')).toEqual([]);
});
