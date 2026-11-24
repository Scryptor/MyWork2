import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import Harness from './PatternFieldHarness.test-helper.svelte';

// What bits-ui calls on a pointer press and jsdom does not have.
beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

const field = () => screen.getByRole('button', { name: 'Графический ключ' });
const out = () => screen.getByTestId('out').textContent;

/**
 * The field opened with a click. jsdom does not place the popover: it stays hidden, and hidden elements
 * have no accessible name — the popover and its buttons are found by their labels and text.
 */
async function open() {
	await userEvent.click(field());
	const popover = await screen.findByRole('dialog', { hidden: true });
	expect(popover).toHaveAttribute('aria-label', 'Графический ключ');
	const inside = within(popover);
	return {
		popover,
		press: async (...points: number[]) => {
			for (const n of points) await userEvent.click(inside.getByLabelText(`Точка ${n}`));
		},
		button: (text: string) => inside.getByText(text).closest('button')!
	};
}
const closed = () =>
	waitFor(() => expect(screen.queryByRole('dialog', { hidden: true })).toBeNull());

describe('PatternField', () => {
	it('the field tells whether a pattern is set and how many points it has', () => {
		const { unmount } = render(Harness);
		expect(field()).toHaveTextContent('Не задан');
		unmount();
		render(Harness, { value: '15963' });
		expect(field()).toHaveTextContent('Задан, 5 точек');
		// «Что это?» of the field.
		expect(screen.getByRole('button', { name: 'Что такое «Графический ключ»' })).toBeTruthy();
	});

	it('«Готово» takes a pattern of four points and more; until then it is not available', async () => {
		const onchange = vi.fn();
		render(Harness, { onchange });
		const p = await open();
		await p.press(1, 5, 9);
		expect(p.button('Готово')).toBeDisabled();
		await p.press(6);
		expect(p.button('Готово')).toBeEnabled();
		await userEvent.click(p.button('Готово'));
		await closed();
		expect(onchange).toHaveBeenCalledExactlyOnceWith('1596');
		expect(field()).toHaveTextContent('Задан, 4 точки');
	});

	it('an empty pattern is taken too: the pattern is removed', async () => {
		const onchange = vi.fn();
		render(Harness, { value: '1596', onchange });
		const p = await open();
		expect(p.popover).toHaveTextContent('1 · 5 · 9 · 6');
		await userEvent.click(p.button('Сбросить'));
		expect(p.button('Готово')).toBeEnabled();
		await userEvent.click(p.button('Готово'));
		await closed();
		expect(onchange).toHaveBeenCalledExactlyOnceWith('');
		expect(field()).toHaveTextContent('Не задан');
	});

	it('closing without «Готово» drops what was pressed: the next opening starts from the value', async () => {
		const onchange = vi.fn();
		render(Harness, { value: '1596', onchange });
		let p = await open();
		await p.press(3);
		expect(p.popover).toHaveTextContent('1 · 5 · 9 · 6 · 3');
		await userEvent.keyboard('{Escape}');
		await closed();
		expect(onchange).not.toHaveBeenCalled();
		expect(out()).toBe('1596');
		p = await open();
		expect(p.popover).toHaveTextContent('1 · 5 · 9 · 6');
		expect(p.popover).not.toHaveTextContent('1 · 5 · 9 · 6 · 3');
	});

	it('«Готово» with the pattern as it was closes without a change', async () => {
		const onchange = vi.fn();
		render(Harness, { value: '1596', onchange });
		const p = await open();
		await userEvent.click(p.button('Готово'));
		await closed();
		expect(onchange).not.toHaveBeenCalled();
	});

	it('Ctrl+Enter is «Готово» while the popover is open', async () => {
		const onchange = vi.fn();
		render(Harness, { onchange });
		const p = await open();
		await p.press(7, 5, 3, 6);
		await userEvent.keyboard('{Control>}{Enter}{/Control}');
		await closed();
		expect(onchange).toHaveBeenCalledExactlyOnceWith('7536');
	});

	it('disabled: the field does not open', () => {
		render(Harness, { value: '1596', disabled: true });
		expect(field()).toBeDisabled();
		expect(field()).toHaveTextContent('Задан, 4 точки');
	});
});
