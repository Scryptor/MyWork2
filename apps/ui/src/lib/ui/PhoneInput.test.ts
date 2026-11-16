import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

describe('PhoneInput', () => {
	it('emits digits and shows the pretty form without focus', async () => {
		render(Harness, { kind: 'phone' });
		const input = screen.getByLabelText('Телефон');
		await userEvent.click(input);
		await userEvent.type(input, '+7 (921) 123-45-67');
		expect(screen.getByTestId('out')).toHaveTextContent('79211234567');
		await userEvent.tab();
		expect(input).toHaveValue('+7 921 123-45-67');
	});
	it('a number changed from outside while focused replaces the text; leaving writes nothing', async () => {
		const onwrite = vi.fn();
		const { rerender } = render(Harness, { kind: 'phone', phone: '70000000001', onwrite });
		const input = screen.getByLabelText('Телефон');
		await userEvent.click(input);
		expect(input).toHaveValue('+7 000 000-00-01');
		await rerender({ phone: '70001234567' });
		expect(input).toHaveFocus();
		expect(input).toHaveValue('+7 000 123-45-67');
		await userEvent.tab();
		expect(screen.getByTestId('out')).toHaveTextContent('70001234567');
		expect(onwrite).not.toHaveBeenCalled();
	});
});
