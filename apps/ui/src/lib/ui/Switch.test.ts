import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

describe('Switch', () => {
	it('has role switch and toggles aria-checked', async () => {
		render(Harness, { kind: 'switch' });
		const sw = screen.getByRole('switch', { name: 'Включено' });
		expect(sw).toHaveAttribute('aria-checked', 'false');
		await userEvent.click(sw);
		expect(sw).toHaveAttribute('aria-checked', 'true');
	});
});
