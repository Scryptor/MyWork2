import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ColorSwatches from './ColorSwatches.svelte';

describe('ColorSwatches', () => {
	it('twelve labelled radios, the current one checked', () => {
		render(ColorSwatches, { value: 'gray', onchange: vi.fn(), label: 'Цвет' });
		const group = screen.getByRole('radiogroup', { name: 'Цвет' });
		const radios = within(group).getAllByRole('radio');
		expect(radios).toHaveLength(12);
		for (const r of radios) expect(r).toHaveAccessibleName();
		expect(within(group).getByRole('radio', { name: 'Серый' })).toBeChecked();
	});

	it('ArrowRight selects the next hue and reports it', async () => {
		const onchange = vi.fn();
		render(ColorSwatches, { value: 'gray', onchange, label: 'Цвет' });
		screen.getByRole('radio', { name: 'Серый' }).focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onchange).toHaveBeenCalledWith('red');
		expect(screen.getByRole('radio', { name: 'Красный' })).toBeChecked();
	});
});
