import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import CellGrid from './CellGrid.svelte';

const cell = (shelf: number, n: number, positions = 0, found = false) => ({
	shelf,
	cell: n,
	code: `B-${shelf}-${n}`,
	positions,
	found
});
// 2 shelves of 3 cells; B-1-2 holds 3, B-2-3 holds 1 and is found.
const cells = [cell(1, 1), cell(1, 2, 3), cell(1, 3), cell(2, 1), cell(2, 2), cell(2, 3, 1, true)];

describe('CellGrid', () => {
	it('draws shelves top down and marks occupied and found cells', () => {
		render(CellGrid, { cells, shelves: 2, perShelf: 3, label: 'Шкаф B' });
		const group = screen.getByRole('group', { name: 'Шкаф B' });
		const buttons = within(group).getAllByRole('button') as [
			HTMLElement,
			HTMLElement,
			HTMLElement,
			HTMLElement,
			HTMLElement,
			HTMLElement
		];
		expect(buttons.map((b) => b.getAttribute('aria-label'))).toEqual([
			'B-1-1, пусто',
			'B-1-2, 3 позиции',
			'B-1-3, пусто',
			'B-2-1, пусто',
			'B-2-2, пусто',
			'B-2-3, 1 позиция'
		]);
		expect(buttons[1]).toHaveTextContent('3');
		expect(buttons[0].textContent?.trim()).toBe('B-1-1');
		expect(buttons[5].querySelector('svg')).not.toBeNull();
		expect(buttons[1].querySelector('svg')).toBeNull();
		expect(buttons[5]).toHaveAttribute('data-found');
		expect(buttons[1]).not.toHaveAttribute('data-found');
	});

	it('selects by click and shows the pressed cell', async () => {
		const onselect = vi.fn();
		render(CellGrid, {
			cells,
			shelves: 2,
			perShelf: 3,
			label: 'Шкаф B',
			selected: 'B-1-2',
			onselect
		});
		expect(screen.getByRole('button', { name: 'B-1-2, 3 позиции' })).toHaveAttribute(
			'aria-pressed',
			'true'
		);
		expect(screen.getByRole('button', { name: 'B-1-1, пусто' })).toHaveAttribute(
			'aria-pressed',
			'false'
		);
		await userEvent.click(screen.getByRole('button', { name: 'B-2-2, пусто' }));
		expect(onselect).toHaveBeenCalledWith('B-2-2');
	});
});
