import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Stack from './SlideOverStack.test-helper.svelte';

/** The level of a panel over `--z-panel`: the panel opened last stands highest. */
function level(text: string): string {
	const panel = screen.getByText(text).closest<HTMLElement>('[role="dialog"]');
	if (!panel) throw new Error(`no panel with «${text}»`);
	return panel.style.getPropertyValue('--panel-level');
}

const none = { a: false, b: false, c: false };

describe('SlideOver over another', () => {
	// The order panel of the layout is in the page before the client's: opened from the client,
	// it went under the client's panel and «did not open».
	it('the panel opened last is on top, whatever their order in the page', async () => {
		const { rerender } = render(Stack, { ...none, b: true });
		expect(level('панель б')).toBe('0');
		await rerender({ ...none, a: true, b: true });
		expect(level('панель а')).toBe('1');
		expect(level('панель б')).toBe('0');
	});

	it('a panel opened after the lower one closed goes over the one left open', async () => {
		const { rerender } = render(Stack, { ...none, a: true });
		await rerender({ ...none, a: true, b: true });
		await rerender({ ...none, b: true });
		await rerender({ ...none, b: true, c: true });
		expect(level('панель б')).toBe('1');
		expect(level('панель в')).toBe('2');
	});

	it('with no panel open the next one starts at the bottom again', async () => {
		const { rerender } = render(Stack, { ...none, a: true });
		await rerender({ ...none, a: true, b: true });
		await rerender(none);
		await rerender({ ...none, c: true });
		expect(level('панель в')).toBe('0');
	});
});
