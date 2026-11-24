import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

const out = () => screen.getByTestId('out').textContent;

describe('QuantityInput', () => {
	it('«Больше» adds one: from 0 to 1', async () => {
		render(Harness, { kind: 'quantity', quantity: 0 });
		await userEvent.setup().click(screen.getByRole('button', { name: 'Больше' }));
		expect(out()).toBe('1');
		expect(screen.getByLabelText('Количество')).toHaveValue('1');
	});

	it('«Меньше» is off at the minimum and takes one away above it', async () => {
		const ui = userEvent.setup();
		render(Harness, { kind: 'quantity', quantity: 1 });
		await ui.click(screen.getByRole('button', { name: 'Меньше' }));
		expect(out()).toBe('0');
		expect(screen.getByRole('button', { name: 'Меньше' })).toBeDisabled();
	});

	it('takes digits only: «12а» is 12', async () => {
		const ui = userEvent.setup();
		render(Harness, { kind: 'quantity', quantity: 0 });
		const input = screen.getByLabelText('Количество');
		await ui.clear(input);
		await ui.type(input, '12а');
		expect(input).toHaveValue('12');
		expect(out()).toBe('12');
	});

	it('a number over the maximum is brought back on leaving: 2000000 is 1 000 000', async () => {
		const ui = userEvent.setup();
		render(Harness, { kind: 'quantity', quantity: 5 });
		const input = screen.getByLabelText('Количество');
		await ui.clear(input);
		await ui.type(input, '2000000');
		await ui.tab();
		expect(out()).toBe('1000000');
		expect(input).toHaveValue('1 000 000');
	});

	it('an empty field goes back to the minimum on leaving', async () => {
		const ui = userEvent.setup();
		render(Harness, { kind: 'quantity', quantity: 7 });
		const input = screen.getByLabelText('Количество');
		await ui.clear(input);
		await ui.tab();
		expect(out()).toBe('0');
		expect(input).toHaveValue('0');
	});

	it('«Больше» is off at the maximum', async () => {
		render(Harness, { kind: 'quantity', quantity: 5, max: 5 });
		expect(screen.getByRole('button', { name: 'Больше' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Меньше' })).toBeEnabled();
	});
});
