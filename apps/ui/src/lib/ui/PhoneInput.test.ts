import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
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
});
