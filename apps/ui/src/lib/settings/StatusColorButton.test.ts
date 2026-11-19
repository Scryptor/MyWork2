import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import StatusColorButton from './StatusColorButton.svelte';

// jsdom has no layout: bits-ui keeps the floating content `visibility: hidden`, and a hidden element has no
// accessible name: find the hues by their value.
const HUE: Record<string, string> = { Серый: 'gray', Красный: 'red' };
const radio = (name: string) =>
	document.querySelector<HTMLInputElement>(`input[type="radio"][value="${HUE[name]}"]`)!;

async function openPalette(onchange = vi.fn()) {
	render(StatusColorButton, { color: 'gray', name: 'Принят', onchange });
	const trigger = screen.getByRole('button', { name: /^Цвет „Принят“/ });
	await userEvent.click(trigger);
	await vi.waitFor(() =>
		expect(screen.getByRole('radiogroup', { hidden: true })).toBeInTheDocument()
	);
	const group = screen.getByRole('radiogroup', { hidden: true });
	return { onchange, group, trigger };
}

describe('StatusColorButton', () => {
	it('arrows move through the palette without choosing or closing it', async () => {
		const { onchange, group } = await openPalette();
		radio('Серый').focus();
		await userEvent.keyboard('{ArrowRight}');
		expect(radio('Красный')).toHaveFocus();
		await userEvent.keyboard('{ArrowRight}{ArrowDown}{ArrowLeft}');
		expect(group).toBeInTheDocument();
		expect(onchange).not.toHaveBeenCalled();
		expect(radio('Серый')).toBeChecked();
	});

	it('Enter on the focused hue chooses it and closes the palette', async () => {
		const { onchange, trigger } = await openPalette();
		radio('Серый').focus();
		await userEvent.keyboard('{ArrowRight}{Enter}');
		expect(onchange).toHaveBeenCalledTimes(1);
		expect(onchange).toHaveBeenCalledWith('red');
		expect(trigger).toHaveAttribute('aria-expanded', 'false');
	});

	it('a click chooses a hue and closes the palette', async () => {
		const { onchange, trigger } = await openPalette();
		await userEvent.click(radio('Красный').closest('label')!);
		expect(onchange).toHaveBeenCalledWith('red');
		expect(trigger).toHaveAttribute('aria-expanded', 'false');
	});

	it('Esc closes the palette and changes nothing', async () => {
		const { onchange, trigger } = await openPalette();
		radio('Серый').focus();
		await userEvent.keyboard('{ArrowRight}{Escape}');
		expect(trigger).toHaveAttribute('aria-expanded', 'false');
		expect(onchange).not.toHaveBeenCalled();
	});
});
