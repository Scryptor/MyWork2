import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

const NNBSP = '\u202f';
const MONEY_ERROR = 'Введите сумму, например 6 500';

/** Selects the whole text of the focused field, as a user does before typing over it. */
function selectAll(input: HTMLElement) {
	if (input instanceof HTMLInputElement) input.setSelectionRange(0, input.value.length);
}

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
	it('takes the amount as it is typed, without leaving the field', async () => {
		render(Harness, { kind: 'money', money: null });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await userEvent.type(input, '7 000,5');
		expect(input).toHaveFocus();
		expect(input).toHaveValue('7 000,5');
		expect(screen.getByTestId('out')).toHaveTextContent('700050');
		await userEvent.tab();
		expect(input).toHaveValue(`7${NNBSP}000,50`);
	});
	it.each([650000, null])(
		'focus and blur without typing write nothing (value %s)',
		async (money) => {
			const onwrite = vi.fn();
			render(Harness, { kind: 'money', money, onwrite });
			await userEvent.click(screen.getByLabelText('Сумма'));
			await userEvent.tab();
			expect(onwrite).not.toHaveBeenCalled();
			expect(screen.getByTestId('out')).toHaveTextContent(JSON.stringify(money));
		}
	);
	it('keeps typed text while focused when value changes from outside; leaving shows the value', async () => {
		const onwrite = vi.fn();
		const { rerender } = render(Harness, { kind: 'money', money: 50_00, onwrite });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		selectAll(input);
		await userEvent.keyboard('4 5');
		expect(screen.getByTestId('out')).toHaveTextContent('4500');
		onwrite.mockClear();
		await rerender({ money: 100_00 }); // a late server answer
		expect(input).toHaveFocus();
		expect(input).toHaveValue('4 5');
		await userEvent.tab();
		expect(input).toHaveValue('100');
		expect(screen.getByTestId('out')).toHaveTextContent(/^10000$/);
		expect(onwrite).not.toHaveBeenCalled();
	});
	it('an untouched focused field follows a value from outside (another record); leaving writes nothing', async () => {
		const onwrite = vi.fn();
		const { rerender } = render(Harness, { kind: 'money', money: null, onwrite });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		await rerender({ money: 990000 });
		expect(input).toHaveValue('9900');
		await userEvent.tab();
		expect(input).toHaveValue(`9${NNBSP}900`);
		expect(screen.getByTestId('out')).toHaveTextContent('990000');
		expect(onwrite).not.toHaveBeenCalled();
	});
	it('invalid input keeps the value and shows an error', async () => {
		render(Harness, { kind: 'money', money: 650000 });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		selectAll(input);
		await userEvent.keyboard('abc');
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent('650000');
		expect(screen.getByText(MONEY_ERROR)).toBeInTheDocument();
		expect(input).toHaveAttribute('aria-invalid', 'true');
	});
	it('`invalid` follows the text at once; the error waits for leaving the field or Enter and goes when fixed', async () => {
		render(Harness, { kind: 'money', money: 650000 });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		selectAll(input);
		await userEvent.keyboard('4 O5O');
		expect(screen.getByTestId('invalid')).toHaveTextContent('true');
		expect(screen.queryByText(MONEY_ERROR)).toBeNull();
		// «4» was an amount: the value keeps the last amount typed.
		expect(screen.getByTestId('out')).toHaveTextContent(/^400$/);
		await userEvent.keyboard('{Enter}');
		expect(screen.getByText(MONEY_ERROR)).toBeInTheDocument();
		expect(input).toHaveFocus();
		expect(input).toHaveValue('4 O5O');

		selectAll(input);
		await userEvent.keyboard('4050');
		expect(screen.queryByText(MONEY_ERROR)).toBeNull();
		expect(screen.getByTestId('invalid')).toHaveTextContent('false');
		expect(screen.getByTestId('out')).toHaveTextContent('405000');
	});
	it('a value from outside replaces a wrong text and its error', async () => {
		const { rerender } = render(Harness, { kind: 'money', money: 650000 });
		const input = screen.getByLabelText('Сумма');
		await userEvent.click(input);
		selectAll(input);
		await userEvent.keyboard('abc');
		await userEvent.tab();
		expect(screen.getByText(MONEY_ERROR)).toBeInTheDocument();
		expect(screen.getByTestId('invalid')).toHaveTextContent('true');
		await rerender({ money: 990000 });
		expect(screen.queryByText(MONEY_ERROR)).toBeNull();
		expect(screen.getByTestId('invalid')).toHaveTextContent('false');
		expect(input).toHaveValue(`9${NNBSP}900`);
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
