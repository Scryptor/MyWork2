import { expect, it } from 'vitest';
import base from './base.css?raw';

const components = import.meta.glob('/src/**/*.svelte', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

/** The numbers of `size={…}` on a component tag: `size={16}`, `size={small ? 14 : 16}`. */
function iconSizes(source: string): number[] {
	const out: number[] = [];
	for (const tag of source.matchAll(/<[A-Z][\w.]*\b[^<>]*?\ssize=\{([^}]*)\}/g))
		for (const n of tag[1]!.matchAll(/(?<![\w.'"-])\d+(?:\.\d+)?(?![\w.'"-])/g))
			out.push(Number(n[0]));
	return out;
}

/** The sizes `base.css` maps to rem, on both attributes. */
function mapped(css: string, attr: 'width' | 'height'): Set<number> {
	return new Set(
		[...css.matchAll(new RegExp(String.raw`\.lucide\[${attr}='(\d+)'\]`, 'g'))].map((m) =>
			Number(m[1])
		)
	);
}

// Lucide writes `size` as width and height in px, which would not grow with the UI scale: base.css maps
// every size in use to rem. A new size needs its two rules there.
it('every icon size in use has its rem rules in base.css', () => {
	// The icons in use are found (dozens of them), so an empty list below means something.
	expect(Object.values(components).flatMap(iconSizes).length).toBeGreaterThan(40);
	const widths = mapped(base, 'width');
	const heights = mapped(base, 'height');
	const missing = Object.entries(components).flatMap(([path, src]) =>
		iconSizes(src)
			.filter((n) => !widths.has(n) || !heights.has(n))
			.map((n) => `${path}: size={${n}}`)
	);
	expect(missing).toEqual([]);
});

it('the size check reads what it should', () => {
	expect(iconSizes('<Phone size={16} aria-hidden="true" />')).toEqual([16]);
	expect(iconSizes(`<Icon size={size === 'sm' ? 14 : 16} />`)).toEqual([14, 16]);
	expect(iconSizes('<Phone\n\tsize={22}\n/>')).toEqual([22]);
	expect(iconSizes('<p style:font-size={`var(--text-${size})`}>')).toEqual([]);
	expect(iconSizes('<Button size="sm">')).toEqual([]);
	expect(mapped(".lucide[width='14'] {} .lucide[height='14'] {}", 'height')).toEqual(new Set([14]));
});
