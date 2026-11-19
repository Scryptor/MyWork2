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

	it('ArrowRight reports the next hue; the parent that takes it shows it', async () => {
		const onchange = vi.fn((hue: string) => void view.rerender({ value: hue as 'red' }));
		const view = render(ColorSwatches, { value: 'gray', onchange, label: 'Цвет' });
		screen.getByRole('radio', { name: 'Серый' }).focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(onchange).toHaveBeenCalledWith('red');
		expect(screen.getByRole('radio', { name: 'Красный' })).toBeChecked();
	});

	it('stays controlled: a parent that refuses keeps the old hue checked', async () => {
		const onchange = vi.fn();
		render(ColorSwatches, { value: 'gray', onchange, label: 'Цвет' });
		await userEvent.click(screen.getByRole('radio', { name: 'Красный' }));
		expect(onchange).toHaveBeenCalledWith('red');
		expect(screen.getByRole('radio', { name: 'Серый' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'Красный' })).not.toBeChecked();
	});

	it('disabled is shown on the group', () => {
		render(ColorSwatches, { value: 'gray', onchange: vi.fn(), label: 'Цвет', disabled: true });
		expect(screen.getByRole('radiogroup', { name: 'Цвет' })).toHaveAttribute(
			'aria-disabled',
			'true'
		);
	});

	it('selectOnArrow=false: arrows only move focus; Enter confirms, also the hue already checked', async () => {
		const onchange = vi.fn();
		render(ColorSwatches, { value: 'gray', onchange, label: 'Цвет', selectOnArrow: false });
		screen.getByRole('radio', { name: 'Серый' }).focus();
		await userEvent.keyboard('{Enter}');
		expect(onchange).toHaveBeenCalledTimes(1);
		expect(onchange).toHaveBeenCalledWith('gray');
		await userEvent.keyboard('{ArrowRight}');
		expect(onchange).toHaveBeenCalledTimes(1);
		expect(screen.getByRole('radio', { name: 'Красный' })).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		expect(onchange).toHaveBeenLastCalledWith('red');
	});
});
