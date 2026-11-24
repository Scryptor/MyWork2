import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import FieldDispute from './FieldDispute.svelte';

describe('FieldDispute', () => {
	it('shows the other side and offers the two choices', async () => {
		const onkeep = vi.fn();
		const ontake = vi.fn();
		render(FieldDispute, { label: 'Цена', theirs: '3 000 ₽', onkeep, ontake });
		const group = screen.getByRole('group', { name: 'Цена: изменили на другом компьютере' });
		expect(group).toHaveTextContent('Там: 3 000 ₽');
		await userEvent.click(screen.getByRole('button', { name: 'Взять новое' }));
		expect(ontake).toHaveBeenCalledTimes(1);
		expect(onkeep).not.toHaveBeenCalled();
		await userEvent.click(screen.getByRole('button', { name: 'Оставить моё' }));
		expect(onkeep).toHaveBeenCalledTimes(1);
	});
});
