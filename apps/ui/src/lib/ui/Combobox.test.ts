import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Combobox from './Combobox.svelte';

describe('Combobox: Enter and leaving the field', () => {
	it('a free text: Enter and leaving the field tell the owner, a key does not', async () => {
		const onenter = vi.fn();
		const onblur = vi.fn();
		render(Combobox, {
			value: '',
			items: [{ value: 'Europe/Moscow' }],
			label: 'Зона',
			onenter,
			onblur
		});
		const field = screen.getByRole('combobox', { name: 'Зона' });
		await userEvent.type(field, 'Mars/Olympus');
		expect(onenter).not.toHaveBeenCalled();
		await userEvent.keyboard('{Enter}');
		expect(onenter).toHaveBeenCalledTimes(1);
		await userEvent.tab();
		expect(onblur).toHaveBeenCalledTimes(1);
	});
});
