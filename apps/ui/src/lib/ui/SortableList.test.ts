import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Harness from './SortableListHarness.test-helper.svelte';

describe('SortableList', () => {
	it('ArrowDown on a handle moves the row, keeps focus and announces the place', async () => {
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Второй“' }).focus();
		await userEvent.keyboard('{ArrowDown}');
		expect(onorder).toHaveBeenCalledTimes(1);
		expect(onorder).toHaveBeenCalledWith([1, 3, 2]);
		expect(screen.getByRole('button', { name: 'Переместить „Второй“' })).toHaveFocus();
		expect(screen.getByText('„Второй“ — место 3 из 3')).toBeInTheDocument();
		expect(screen.getByText('„Второй“ — место 3 из 3').closest('[aria-live]')).toHaveAttribute(
			'aria-live',
			'polite'
		);
	});

	it('ArrowUp on the first row does nothing', async () => {
		const onorder = vi.fn();
		render(Harness, { onorder });
		screen.getByRole('button', { name: 'Переместить „Первый“' }).focus();
		await userEvent.keyboard('{ArrowUp}');
		expect(onorder).not.toHaveBeenCalled();
	});

	it('disabled has no handles', () => {
		render(Harness, { disabled: true });
		expect(screen.queryAllByRole('button')).toHaveLength(0);
		expect(screen.getByText('Второй')).toBeInTheDocument();
	});
});
