import { render, screen, within } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import Harness from './CatalogRowsHarness.test-helper.svelte';

// The rows of statuses, points and dictionaries: one grid with the same columns for every row, and a row of
// column titles over them instead of a label over every control.
const rows = [
	{ id: 1, label: 'Принят', archived: false, count: 3 },
	{ id: 2, label: 'Готов', archived: false, count: 1234 },
	{ id: 3, label: 'Старый', archived: true, count: 0 }
];

const titles = () =>
	[...screen.getByTestId('catalog-head').children]
		.map((c) => c.textContent?.replace(/\s+/g, ' ').trim())
		.filter(Boolean);

describe('CatalogRows', () => {
	it('titles the columns once, in their order; the help of a column is next to its title', async () => {
		render(Harness, { rows });
		expect(titles()).toEqual(['Название', 'Заказов', 'Цвет', 'Вид']);
		const head = screen.getByTestId('catalog-head');
		expect(within(head).getByRole('button', { name: 'Что такое «Вид»' })).toBeInTheDocument();
		// No label over the controls of a row.
		expect(screen.getAllByText('Вид')).toHaveLength(1);
	});

	it('the count is a number under «Заказов», said in words to a screen reader', () => {
		render(Harness, { rows });
		const row = screen.getAllByRole('listitem').find((li) => li.textContent?.includes('Готов'))!;
		const count = within(row).getByText('1 234');
		expect(count).toHaveAttribute('aria-hidden', 'true');
		expect(row).toHaveTextContent('в 1234 заказах');
	});

	it('the title of the count follows what is counted', () => {
		render(Harness, { rows, withColumns: false, countTitle: 'Клиентов' });
		expect(titles()).toEqual(['Название', 'Клиентов']);
	});

	it('«В архив» and «Вернуть» are buttons with their words, not only an icon', () => {
		render(Harness, { rows });
		const archive = screen.getAllByRole('button', { name: 'В архив' });
		expect(archive).toHaveLength(2);
		for (const b of archive) expect(b).toHaveTextContent('В архив');
		expect(screen.getByRole('button', { name: 'Вернуть' })).toHaveTextContent('Вернуть');
	});

	it('read only: the titles stay, no buttons to change anything', () => {
		render(Harness, { rows, readonly: true });
		expect(titles()).toEqual(['Название', 'Заказов', 'Цвет', 'Вид']);
		expect(screen.queryByRole('button', { name: 'В архив' })).not.toBeInTheDocument();
	});
});
