import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Harness from './FieldHarness.test-helper.svelte';

const options = ['Чехол', 'Зарядка'];

describe('ChipGroup', () => {
	it('click toggles a chip', async () => {
		render(Harness, { kind: 'chips', options });
		const chip = screen.getByRole('button', { name: 'Чехол' });
		await userEvent.click(chip);
		expect(chip).toHaveAttribute('aria-pressed', 'true');
		expect(screen.getByTestId('out')).toHaveTextContent('["Чехол"]');
		await userEvent.click(chip);
		expect(chip).toHaveAttribute('aria-pressed', 'false');
		expect(screen.getByTestId('out')).toHaveTextContent('[]');
	});
	it('custom duplicate selects the existing option without oncreate', async () => {
		const oncreate = vi.fn();
		render(Harness, { kind: 'chips', options, oncreate });
		await userEvent.click(screen.getByRole('button', { name: '+ своё' }));
		await userEvent.type(screen.getByRole('textbox'), ' чехол {Enter}');
		expect(oncreate).not.toHaveBeenCalled();
		expect(screen.getByRole('button', { name: 'Чехол' })).toHaveAttribute('aria-pressed', 'true');
		expect(screen.getAllByRole('button', { name: /чехол/i })).toHaveLength(1);
	});
	it('custom new value is added, selected and reported', async () => {
		const oncreate = vi.fn();
		render(Harness, { kind: 'chips', options, oncreate });
		await userEvent.click(screen.getByRole('button', { name: '+ своё' }));
		await userEvent.type(screen.getByRole('textbox'), 'Стилус{Enter}');
		expect(oncreate).toHaveBeenCalledWith('Стилус');
		expect(screen.getByRole('button', { name: 'Стилус' })).toHaveAttribute('aria-pressed', 'true');
		expect(screen.getByTestId('out')).toHaveTextContent('["Стилус"]');
	});
});
