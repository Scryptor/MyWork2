import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import InsertChip from './InsertChipHarness.test-helper.svelte';

const props = { label: 'Номер заказа', code: '#v("order.number")' };

describe('InsertChip', () => {
	it('is a button named «Вставить: <label>» that shows the label', () => {
		render(InsertChip, { ...props, onclick: () => {} });
		const button = screen.getByRole('button', { name: 'Вставить: Номер заказа' });
		expect(button).toHaveTextContent('Номер заказа');
	});

	it('shows the code in a tooltip on hover', async () => {
		render(InsertChip, { ...props, onclick: () => {} });
		await userEvent.hover(screen.getByRole('button', { name: 'Вставить: Номер заказа' }));
		const tip = await screen.findByRole('tooltip', {}, { timeout: 3000 });
		expect(tip).toHaveTextContent('#v("order.number")');
	});

	// The tooltip of a keyboard focus is bits-ui's (`:focus-visible`, which jsdom has no notion of): here
	// the badge is only reachable by Tab, the way it opens is checked by the «icon button» test of e2e.
	it('is reachable by Tab', async () => {
		render(InsertChip, { ...props, onclick: () => {} });
		await userEvent.tab();
		expect(screen.getByRole('button', { name: 'Вставить: Номер заказа' })).toHaveFocus();
	});

	it('a click calls onclick', async () => {
		const onclick = vi.fn();
		render(InsertChip, { ...props, onclick });
		await userEvent.click(screen.getByRole('button', { name: 'Вставить: Номер заказа' }));
		expect(onclick).toHaveBeenCalledOnce();
	});
});
