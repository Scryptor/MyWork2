import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import DataTable from './DataTable.svelte';
import type { Column } from './types.js';

interface Item {
	id: number;
	name: string;
}

const ROWS: Item[] = Array.from({ length: 5 }, (_, i) => ({ id: i + 1, name: `Строка ${i + 1}` }));
const nameCell = createRawSnippet<[unknown]>((item) => ({
	render: () => `<span>${(item() as Item).name}</span>`
}));
const COLUMNS: Column<unknown>[] = [{ id: 'name', header: 'Имя', width: 160, cell: nameCell }];

// jsdom has no layout: the virtualizer reads the scroller's client size, give it one.
beforeEach(() => {
	vi.spyOn(Element.prototype, 'clientHeight', 'get').mockReturnValue(400);
	vi.spyOn(Element.prototype, 'clientWidth', 'get').mockReturnValue(800);
});
afterEach(() => {
	vi.restoreAllMocks();
	localStorage.clear();
});

function setup(selectable: boolean) {
	render(DataTable, {
		props: {
			rows: ROWS,
			columns: COLUMNS,
			rowId: (r: unknown) => (r as Item).id,
			storageKey: 'test',
			ariaLabel: 'Тест',
			selectable,
			activeId: 2
		}
	});
	return screen.getByRole('grid', { name: 'Тест' });
}

async function bodyRows(): Promise<HTMLElement[]> {
	const rows = await screen.findAllByRole('row');
	return rows.slice(1); // the first one is the header row
}

describe('DataTable rows for assistive technology', () => {
	it('not selectable: the current row is aria-current, no row carries aria-selected', async () => {
		const grid = setup(false);
		const rows = await bodyRows();
		expect(rows).toHaveLength(5);
		expect(grid).not.toHaveAttribute('aria-multiselectable');
		for (const row of rows) expect(row).not.toHaveAttribute('aria-selected');
		expect(rows[1]).toHaveAttribute('aria-current', 'true');
		expect(rows[0]).not.toHaveAttribute('aria-current');
		expect(grid).toHaveAttribute('aria-activedescendant', rows[1]!.id);
	});

	it('selectable: multiselectable grid, aria-selected follows the checkbox, not the cursor', async () => {
		const grid = setup(true);
		const rows = await bodyRows();
		expect(grid).toHaveAttribute('aria-multiselectable', 'true');
		for (const row of rows) expect(row).toHaveAttribute('aria-selected', 'false');

		await userEvent.click(within(rows[3]!).getByRole('checkbox', { name: 'Выбрать строку' }));
		expect(rows[3]).toHaveAttribute('aria-selected', 'true');
		expect(rows[3]).not.toHaveAttribute('aria-current');
		expect(rows[1]).toHaveAttribute('aria-current', 'true');
		expect(rows[1]).toHaveAttribute('aria-selected', 'false');
	});
});
