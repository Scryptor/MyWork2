import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEMO_STATUSES } from '#lib/demo/fake.js';
import StatusPicker from './StatusPicker.svelte';

/** jsdom has no matchMedia: pretend to be a computer or a phone. */
function screenWidth(phone: boolean) {
	window.matchMedia = vi.fn((media: string) => ({
		matches: phone && media.includes('max-width'),
		media,
		onchange: null,
		addEventListener() {},
		removeEventListener() {},
		addListener() {},
		removeListener() {},
		dispatchEvent: () => false
	}));
}

afterEach(() => {
	Reflect.deleteProperty(window, 'matchMedia');
});

describe('StatusPicker', () => {
	it('a radio per status as a label; the current one is checked', () => {
		screenWidth(false);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 1 });
		const group = screen.getByRole('radiogroup', { name: 'Статус' });
		expect(
			within(group)
				.getAllByRole('radio')
				.map((r) => r.closest('label')?.textContent?.trim())
		).toEqual(DEMO_STATUSES.map((s) => s.name));
		expect(within(group).getByRole('radio', { name: 'Диагностика' })).toBeChecked();
		expect(within(group).getByRole('radio', { name: 'Готов' }).closest('label')).toHaveClass(
			'st-green'
		);
	});

	it('a pick reports the new and the previous status', async () => {
		screenWidth(false);
		const onchange = vi.fn();
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 1, onchange, label: 'Статус заказа' });
		const group = screen.getByRole('radiogroup', { name: 'Статус заказа' });
		await userEvent.click(within(group).getByRole('radio', { name: 'Готов' }));
		expect(onchange).toHaveBeenCalledWith(7, 1);
		expect(within(group).getByRole('radio', { name: 'Готов' })).toBeChecked();
		await userEvent.click(within(group).getByRole('radio', { name: 'Готов' }));
		expect(onchange).toHaveBeenCalledOnce();
	});

	it('disabled: nothing can be picked', () => {
		screenWidth(false);
		render(StatusPicker, { statuses: DEMO_STATUSES, value: 8, disabled: true });
		for (const radio of screen.getAllByRole('radio')) expect(radio).toBeDisabled();
		expect(screen.getByRole('radio', { name: 'Выдан' })).toBeChecked();
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
