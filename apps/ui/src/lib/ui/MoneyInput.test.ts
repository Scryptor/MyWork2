import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

const NNBSP = '\u202f';

describe('MoneyInput', () => {
	it('shows a formatted value without focus and the currency suffix', () => {
		render(Harness, { kind: 'money', money: 650000 });
		expect(screen.getByLabelText('Сумма')).toHaveValue(`6${NNBSP}500`);
		expect(screen.getByText('₽')).toBeInTheDocument();
	});
	it('shows the plain number on focus', async () => {
		render(Harness, { kind: 'money', money: 650000 });
		await userEvent.click(screen.getByLabelText('Сумма'));
		expect(screen.getByLabelText('Сумма')).toHaveValue('6500');
	});
	it('parses on blur', async () => {
		render(Harness, { kind: 'money', money: null });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await userEvent.type(input, '7 000,5');
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent('700050');
	});
	it('invalid input keeps the value and shows an error', async () => {
		render(Harness, { kind: 'money', money: 650000 });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await userEvent.clear(input);
		await userEvent.type(input, 'abc');
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent('650000');
		expect(screen.getByText('Введите сумму, например 6 500')).toBeInTheDocument();
		expect(input).toHaveAttribute('aria-invalid', 'true');
	});
	it('empty input is null when nullable', async () => {
		render(Harness, { kind: 'money', money: 650000, nullable: true });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await userEvent.clear(input);
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent('null');
	});
	it('empty input is 0 when not nullable', async () => {
		render(Harness, { kind: 'money', money: 650000, nullable: false });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await userEvent.clear(input);
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent(/^0$/);
	});
});
