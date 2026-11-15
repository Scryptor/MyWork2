import { expect, it } from 'vitest';

const files = import.meta.glob('/src/**/*.{svelte,css}', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;

function stripComments(s: string): string {
	return s.replace(/\/\*[\s\S]*?\*\//g, '').replace(/<!--[\s\S]*?-->/g, '');
}

it('has no raw colors outside tokens.css', () => {
	const checked = Object.entries(files).filter(([p]) => p !== '/src/lib/design/tokens.css');
	expect(checked.length).toBeGreaterThan(0);
	const bad = checked
		.filter(([, src]) => /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/.test(stripComments(src)))
		.map(([p]) => p);
	expect(bad).toEqual([]);
});
