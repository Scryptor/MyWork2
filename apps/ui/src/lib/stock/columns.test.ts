import type { StockItem } from '$api/StockItem';
import type { Snippet } from 'svelte';
import { describe, expect, it } from 'vitest';
import { STOCK_COLUMN_IDS, STOCK_SORT_FIELD, stockColumns, type StockColumnId } from './columns.js';

const cell = (() => {}) as unknown as Snippet<[StockItem]>;
const cells = Object.fromEntries(STOCK_COLUMN_IDS.map((id) => [id, cell])) as Record<
	StockColumnId,
	Snippet<[StockItem]>
>;

describe('stockColumns', () => {
	it('has the columns of decision 18 in Russian, each with its cell', () => {
		const columns = stockColumns({ cells });
		expect(columns.map((c) => c.id)).toEqual([
			'id',
			'name',
			'quantity',
			'place',
			'price',
			'restock',
			'part_type',
			'device_type',
			'brand',
			'model',
			'color'
		]);
		expect(columns.map((c) => c.header)).toEqual([
			'Арт.',
			'Наименование',
			'Кол-во',
			'Место',
			'Цена',
			'Докупить при',
			'Тип запчасти',
			'Тип устройства',
			'Бренд',
			'Модель',
			'Цвет'
		]);
		for (const c of columns) expect(c.cell).toBe(cell);
	});

	it('shows six columns by default, the other five are for the columns menu', () => {
		const columns = stockColumns({ cells });
		expect(columns.filter((c) => !c.hiddenByDefault).map((c) => c.id)).toEqual([
			'id',
			'name',
			'quantity',
			'place',
			'price',
			'restock'
		]);
	});

	it('sorts by number, name, quantity and price, as the server does', () => {
		expect(STOCK_SORT_FIELD).toEqual({
			id: 'id',
			name: 'name',
			quantity: 'quantity',
			price: 'price'
		});
		const sortable = stockColumns({ cells }).filter((c) => c.sortable);
		expect(stockColumns({ cells }).find((c) => c.id === 'place')).toMatchObject({
			width: 140,
			sortable: false
		});
		expect(sortable.map((c) => c.id)).toEqual(['id', 'name', 'quantity', 'price']);
	});
});
