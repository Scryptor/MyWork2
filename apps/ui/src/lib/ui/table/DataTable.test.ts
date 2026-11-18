import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { prefs } from '#lib/design/prefs.svelte.js';
import DataTable from './DataTableHarness.test-helper.svelte';
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

describe('DataTable row height follows the UI scale', () => {
	const MANY: Item[] = Array.from({ length: 30 }, (_, i) => ({
		id: i + 1,
		name: `Строка ${i + 1}`
	}));
	function renderMany() {
		render(DataTable, {
			props: {
				rows: MANY,
				columns: COLUMNS,
				rowId: (r: unknown) => (r as Item).id,
				storageKey: 'test-many',
				ariaLabel: 'Тест'
			}
		});
	}
	afterEach(() => prefs.set({ scale: 'auto', density: 'comfortable' }));

	it('comfortable at 1.1: 35px rows, the 10th row at 9 rows down', async () => {
		prefs.set({ scale: 1.1, density: 'comfortable' });
		renderMany();
		const rows = await bodyRows();
		expect(rows[0]!.style.height).toBe('35px');
		expect(rows[9]!.style.transform).toBe('translateY(315px)');
	});

	it('compact at 0.9: 25px rows, the 10th row at 9 rows down', async () => {
		prefs.set({ scale: 0.9, density: 'compact' });
		renderMany();
		const rows = await bodyRows();
		expect(rows[0]!.style.height).toBe('25px');
		expect(rows[9]!.style.transform).toBe('translateY(225px)');
	});
});

describe('DataTable columns fill the width', () => {
	const textCell = createRawSnippet<[unknown]>(() => ({ render: () => '<span></span>' }));
	const MIXED: Column<unknown>[] = [
		{ id: 'id', header: '№', width: 80, cell: textCell },
		{ id: 'client', header: 'Клиент', width: 200, minWidth: 120, flex: true, cell: textCell },
		{ id: 'device', header: 'Устройство', width: 180, minWidth: 100, flex: true, cell: textCell },
		{ id: 'status', header: 'Статус', width: 150, cell: textCell }
	];
	function renderMixed(selectable = false) {
		const { container } = render(DataTable, {
			props: {
				rows: ROWS,
				columns: MIXED,
				rowId: (r: unknown) => (r as Item).id,
				storageKey: 'test-fill',
				ariaLabel: 'Тест',
				selectable
			}
		});
		const table = container.querySelector<HTMLElement>('.mw-table')!;
		return {
			cols: () => table.style.getPropertyValue('--cols'),
			min: () => table.style.getPropertyValue('--table-min')
		};
	}
	afterEach(() => prefs.set({ scale: 'auto' }));

	it('text columns share the spare width down to their minimum; the others keep their px', () => {
		const { cols, min } = renderMixed();
		expect(cols()).toBe('80px minmax(120px, 200fr) minmax(100px, 180fr) 150px var(--control-lg)');
		expect(min()).toBe('calc(450px + 1 * var(--control-lg))');
	});

	it('the minimums and fixed widths follow the UI scale', () => {
		prefs.set({ scale: 1.25 });
		const { cols, min } = renderMixed(true);
		expect(cols()).toBe(
			'var(--control-lg) 100px minmax(150px, 200fr) minmax(125px, 180fr) 188px var(--control-lg)'
		);
		expect(min()).toBe('calc(563px + 2 * var(--control-lg))');
	});

	it('a column the user resized is fixed at its width', () => {
		localStorage.setItem(
			'mw.table.test-fill',
			JSON.stringify({ v: 2, order: [], hidden: [], widths: { client: 260 } })
		);
		const { cols, min } = renderMixed();
		expect(cols()).toBe('80px 260px minmax(100px, 180fr) 150px var(--control-lg)');
		expect(min()).toBe('calc(590px + 1 * var(--control-lg))');
	});

	it('without a flexible column the last one takes the rest', () => {
		localStorage.setItem(
			'mw.table.test-fill',
			JSON.stringify({ v: 2, order: [], hidden: [], widths: { client: 260, device: 190 } })
		);
		const { cols } = renderMixed();
		expect(cols()).toBe('80px 260px 190px 150px minmax(var(--control-lg), 1fr)');
	});
});
