import { render } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Kbd from './Kbd.svelte';

describe('Kbd', () => {
	it('is a hint by default: a touch screen does not show it', () => {
		const { container } = render(Kbd, { props: { shortcut: 'Mod+K' } });
		expect(container.querySelector('kbd')?.classList.contains('hint')).toBe(true);
		// The rule that hides a hint: a coarse pointer (not a width), as the rule of widths allows.
		const css = Object.values(
			import.meta.glob('./Kbd.svelte', { query: '?raw', import: 'default', eager: true })
		)[0] as string;
		expect(css).toMatch(/@media \(pointer: coarse\) \{\s*\.hint \{\s*display: none;/);
	});

	it('the list of the shortcuts shows the keys everywhere', () => {
		const { container } = render(Kbd, { props: { shortcut: 'Mod+K', hint: false } });
		expect(container.querySelector('kbd')?.classList.contains('hint')).toBe(false);
	});
});
