import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Harness from './TagInputHarness.test-helper.svelte';

const out = () => JSON.parse(screen.getByTestId('out').textContent ?? '[]') as string[];

describe('TagInput', () => {
	it('Enter adds the text as a chip and empties the field; the form is not sent', async () => {
		const onsubmit = vi.fn();
		render(Harness, { onsubmit });
		const field = screen.getByRole('textbox', { name: 'Мастера' });
		await userEvent.type(field, '  Петров   Пётр {Enter}');
		expect(out()).toEqual(['Петров Пётр']);
		expect(field).toHaveValue('');
		expect(
			within(screen.getByRole('list', { name: 'Мастера' })).getByText('Петров Пётр')
		).toBeTruthy();
		expect(onsubmit).not.toHaveBeenCalled();
	});

	it('Enter in an empty field sends the form', async () => {
		const onsubmit = vi.fn();
		render(Harness, { values: ['Петров'], onsubmit });
		await userEvent.type(screen.getByRole('textbox', { name: 'Мастера' }), '{Enter}');
		expect(onsubmit).toHaveBeenCalledOnce();
		expect(out()).toEqual(['Петров']);
	});

	it('the same name again (any case) is not added twice', async () => {
		render(Harness, { values: ['Петров'] });
		await userEvent.type(screen.getByRole('textbox', { name: 'Мастера' }), 'петров{Enter}');
		expect(out()).toEqual(['Петров']);
	});

	it('× removes its chip; Backspace in an empty field removes the last one', async () => {
		render(Harness, { values: ['Петров', 'Сидоров', 'Иванов'] });
		await userEvent.click(screen.getByRole('button', { name: 'Убрать «Сидоров»' }));
		expect(out()).toEqual(['Петров', 'Иванов']);
		await userEvent.type(screen.getByRole('textbox', { name: 'Мастера' }), '{Backspace}');
		expect(out()).toEqual(['Петров']);
	});

	it('leaving the field keeps what was typed as a chip', async () => {
		render(Harness);
		await userEvent.type(screen.getByRole('textbox', { name: 'Мастера' }), 'Сидоров');
		await userEvent.click(screen.getByRole('button', { name: 'Отправить' }));
		expect(out()).toEqual(['Сидоров']);
	});
});
