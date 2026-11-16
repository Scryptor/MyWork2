import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

const DATE_ERROR = 'Введите дату, например 15.11.2026';
const RANGE_ERROR = 'Дата вне допустимого диапазона';

describe('DateInput', () => {
	it('`invalid` follows the typed text; the error shows on leaving the field, the date is taken then', async () => {
		render(Harness, { kind: 'date', date: null });
		const input = screen.getByLabelText('Дата');
		await userEvent.click(input);
		await userEvent.type(input, '31.02.2026');
		expect(screen.getByTestId('invalid')).toHaveTextContent('true');
		expect(screen.queryByText(DATE_ERROR)).toBeNull();
		await userEvent.tab();
		expect(screen.getByText(DATE_ERROR)).toBeInTheDocument();
		expect(input).toHaveAttribute('aria-invalid', 'true');
		expect(screen.getByTestId('out')).toHaveTextContent('null');

		await userEvent.clear(input);
		await userEvent.type(input, '15.11.2026');
		expect(screen.getByTestId('invalid')).toHaveTextContent('false');
		await userEvent.tab();
		expect(screen.queryByText(DATE_ERROR)).toBeNull();
		expect(screen.getByTestId('out')).toHaveTextContent('"2026-11-15"');
	});
	it('a date out of range is invalid too and keeps the value', async () => {
		render(Harness, { kind: 'date', date: '2026-03-01', min: '2026-01-01' });
		const input = screen.getByLabelText('Дата');
		await userEvent.clear(input);
		await userEvent.type(input, '31.12.2025');
		expect(screen.getByTestId('invalid')).toHaveTextContent('true');
		await userEvent.keyboard('{Enter}');
		expect(screen.getByText(RANGE_ERROR)).toBeInTheDocument();
		expect(screen.getByTestId('out')).toHaveTextContent('"2026-03-01"');
	});
	it('a date from outside clears a wrong text', async () => {
		const { rerender } = render(Harness, { kind: 'date', date: '2026-03-01' });
		const input = screen.getByLabelText('Дата');
		await userEvent.clear(input);
		await userEvent.type(input, '99.99.2026');
		await userEvent.tab();
		expect(screen.getByTestId('invalid')).toHaveTextContent('true');
		await rerender({ date: '2026-11-15' });
		expect(screen.getByTestId('invalid')).toHaveTextContent('false');
		expect(input).toHaveValue('15.11.2026');
		expect(screen.queryByText(DATE_ERROR)).toBeNull();
	});
});
