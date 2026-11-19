import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEMO_STATUSES } from '#lib/demo/fake.js';
import { prefs } from '#lib/design/prefs.svelte.js';
import StatusPicker from './StatusPicker.svelte';
import Harness from './StatusPickerHarness.test-helper.svelte';

let stop: (() => void) | null = null;

/** The width band comes from the window: pretend to be a computer or a phone. */
function screenWidth(phone: boolean) {
	stop?.();
	vi.stubGlobal('innerWidth', phone ? 500 : 1280);
	vi.stubGlobal('screen', { width: 1920, height: 1080 });
	stop = prefs.init();
}

afterEach(() => {
	stop?.();
	stop = null;
	vi.unstubAllGlobals();
});

describe('StatusPicker', () => {
	it('a radio per status as a label; the current one is checked', () => {
		screenWidth(false);
		render(StatusPicker, { statuses: DEMO_STATUSES.slice(0, 7), value: 1 });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		expect(
			within(group)
				.getAllByRole('radio')
				.map((r) => r.closest('label')?.textContent?.trim())
		).toEqual(DEMO_STATUSES.slice(0, 7).map((s) => s.name));
		expect(screen.queryByRole('button', { name: /^Ещё/ })).toBeNull();
		expect(within(group).getByRole('radio', { name: 'Диагностика' })).toBeChecked();
		expect(within(group).getByRole('radio', { name: 'Готов' }).closest('label')).toHaveClass(
			'st-green'
		);
	});

	it('with onchange the parent decides: a status it takes is shown', async () => {
		screenWidth(false);
		render(Harness, { mode: 'take' });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		await userEvent.click(within(group).getByRole('radio', { name: 'Готов' }));
		expect(screen.getByTestId('changes')).toHaveTextContent('1→7');
		expect(screen.getByTestId('value')).toHaveTextContent('7');
		expect(within(group).getByRole('radio', { name: 'Готов' })).toBeChecked();
		await userEvent.click(within(group).getByRole('radio', { name: 'Готов' }));
		expect(screen.getByTestId('changes')).toHaveTextContent(/^1→7$/);
	});

	it('a parent that keeps the old value refuses the change: the old radio is checked again', async () => {
		screenWidth(false);
		render(Harness, { mode: 'keep' });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		const ready = within(group).getByRole('radio', { name: 'Готов' });
		await userEvent.click(ready);
		expect(screen.getByTestId('changes')).toHaveTextContent('1→7');
		expect(screen.getByTestId('value')).toHaveTextContent('1');
		expect(ready).not.toBeChecked();
		expect(within(group).getByRole('radio', { name: 'Диагностика' })).toBeChecked();
		expect(ready.closest('label')).not.toHaveClass('on');

		// Refused again: the picker asks every time, it does not remember the refused choice.
		await userEvent.click(ready);
		expect(screen.getByTestId('changes')).toHaveTextContent('1→7 1→7');
		expect(ready).not.toBeChecked();
	});

	it('without onchange a bound value takes the choice', async () => {
		screenWidth(false);
		render(Harness, { mode: 'bind' });
		await userEvent.click(screen.getByRole('radio', { name: 'Ждёт запчасть' }));
		expect(screen.getByTestId('value')).toHaveTextContent('5');
		expect(screen.getByRole('radio', { name: 'Ждёт запчасть' })).toBeChecked();
	});

	it('disabled: nothing can be picked', () => {
		screenWidth(false);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 7, disabled: true });
		for (const radio of screen.getAllByRole('radio')) expect(radio).toBeDisabled();
		expect(screen.getByRole('radio', { name: 'Готов' })).toBeChecked();
		expect(screen.getByRole('button', { name: 'Ещё' })).toBeDisabled();
	});

	it('puts statuses beyond max into «Ещё»', () => {
		screenWidth(false);
		expect(DEMO_STATUSES).toHaveLength(9);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 1 });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		expect(
			within(group)
				.getAllByRole('radio')
				.map((r) => r.closest('label')?.textContent?.trim())
		).toEqual(DEMO_STATUSES.slice(0, 7).map((s) => s.name));
		const more = screen.getByRole('button', { name: 'Ещё' });
		expect(more).toHaveAttribute('aria-haspopup', 'menu');
		expect(screen.queryByRole('radio', { name: 'Выдан' })).toBeNull();
	});

	it('a chosen status inside «Ещё» is shown on the button; no segment is checked', () => {
		screenWidth(false);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 8, max: 5 });
		expect(screen.getAllByRole('radio')).toHaveLength(5);
		for (const radio of screen.getAllByRole('radio')) expect(radio).not.toBeChecked();
		const more = screen.getByRole('button', { name: 'Ещё: Выдан' });
		expect(more).toHaveTextContent('Выдан');
		expect(more).not.toHaveTextContent('Ещё');
	});

	it('arrow keys move focus without choosing; space chooses', async () => {
		screenWidth(false);
		const user = userEvent.setup();
		render(Harness, { mode: 'take' });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		const radios = within(group).getAllByRole('radio');
		radios[0]!.focus();
		await user.keyboard('{ArrowRight}{ArrowRight}');
		expect(radios[2]).toHaveFocus();
		expect(screen.getByTestId('changes')).toHaveTextContent(/^$/);
		expect(radios[0]).toBeChecked();
		await user.keyboard(' ');
		expect(screen.getByTestId('changes')).toHaveTextContent(/^1→3$/);
		expect(radios[2]).toBeChecked();
	});

	it('Shift+arrow does not choose through the native radio: it only moves focus', async () => {
		screenWidth(false);
		render(Harness, { mode: 'take' });
		const radios = within(screen.getByRole('radiogroup', { name: 'Статус' })).getAllByRole('radio');
		radios[0]!.focus();
		// A browser would check the radio it reaches by this key; the picker must cancel it.
		const notCancelled = await fireEvent.keyDown(radios[0]!, { key: 'ArrowRight', shiftKey: true });
		expect(notCancelled).toBe(false);
		expect(radios[1]).toHaveFocus();
		await fireEvent.keyDown(radios[1]!, { key: 'ArrowDown', shiftKey: true });
		expect(radios[2]).toHaveFocus();
		expect(screen.getByTestId('changes')).toHaveTextContent(/^$/);
		expect(radios[0]).toBeChecked();
	});

	it('Enter chooses the focused status; arrows wrap around', async () => {
		screenWidth(false);
		const user = userEvent.setup();
		render(Harness, { mode: 'take' });
		const radios = within(screen.getByRole('radiogroup', { name: 'Статус' })).getAllByRole('radio');
		radios[0]!.focus();
		await user.keyboard('{ArrowLeft}');
		expect(radios[6]).toHaveFocus();
		await user.keyboard('{ArrowDown}{ArrowUp}{ArrowUp}');
		expect(radios[5]).toHaveFocus();
		expect(screen.getByTestId('changes')).toHaveTextContent(/^$/);
		await user.keyboard('{Enter}');
		expect(screen.getByTestId('changes')).toHaveTextContent(/^1→6$/);
	});

	it('phone: a button with the current status instead of the labels', () => {
		screenWidth(true);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 5 });
		expect(screen.queryByRole('radiogroup')).toBeNull();
		const button = screen.getByRole('button', { name: 'Статус: Ждёт запчасть' });
		expect(button).toHaveAttribute('aria-haspopup', 'dialog');
		expect(button).toHaveTextContent('Ждёт запчасть');
	});
});
