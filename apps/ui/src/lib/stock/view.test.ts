import { describe, expect, it } from 'vitest';
import {
	DEFAULT_STOCK_SORT,
	openStockItemId,
	stockItemHref,
	stockSearch,
	stockViewFromSearch,
	stockViewToQuery,
	stockViewToSearch,
	type StockView
} from './view.js';

const fromSearch = (s: string) => stockViewFromSearch(new URLSearchParams(s));
const plain: StockView = {
	query: '',
	sort: { ...DEFAULT_STOCK_SORT },
	restock: false,
	asText: false
};

describe('stock view in the address', () => {
	it('the default view is an empty address: by name, A to Z, everything', () => {
		expect(DEFAULT_STOCK_SORT).toEqual({ columnId: 'name', descending: false });
		expect(stockViewToSearch(plain)).toBe('');
		expect(fromSearch('')).toEqual(plain);
	});

	it('goes there and back: restock, q, sort, desc', () => {
		const views: StockView[] = [
			{ ...plain, query: 'дисплей', sort: { columnId: 'price', descending: true }, restock: true },
			{ ...plain, sort: { columnId: 'quantity', descending: false } },
			{ ...plain, query: 'a,b', sort: { columnId: 'name', descending: true } },
			{ ...plain, sort: { columnId: 'id', descending: false }, restock: true },
			{ ...plain, query: 'b-2', asText: true }
		];
		for (const v of views) expect(fromSearch(stockViewToSearch(v))).toEqual(v);
		expect(
			stockViewToSearch({
				...plain,
				query: 'дисплей',
				sort: { columnId: 'price', descending: true },
				restock: true
			})
		).toBe('q=%D0%B4%D0%B8%D1%81%D0%BF%D0%BB%D0%B5%D0%B9&sort=price&desc=1&restock=1');
		expect(fromSearch('restock=1&q=дисплей&sort=price&desc=1')).toEqual({
			asText: false,
			query: 'дисплей',
			sort: { columnId: 'price', descending: true },
			restock: true
		});
	});

	it('anything it does not know falls back to the default', () => {
		expect(fromSearch('sort=restock&desc=x&restock=yes')).toEqual(plain);
		expect(fromSearch('sort=price')).toEqual({
			...plain,
			sort: { columnId: 'price', descending: false }
		});
	});

	it('?item= opens a position and stays beside the view', () => {
		expect(openStockItemId(new URLSearchParams('item=12&q=x'))).toBe(12);
		expect(openStockItemId(new URLSearchParams('item=0'))).toBeNull();
		expect(openStockItemId(new URLSearchParams('item=abc'))).toBeNull();
		expect(openStockItemId(new URLSearchParams(''))).toBeNull();
		expect(stockSearch(plain, 12)).toBe('item=12');
		expect(stockSearch(plain, null)).toBe('');
		const v = { ...plain, restock: true };
		expect(stockSearch(v, 12)).toBe('restock=1&item=12');
		expect(stockItemHref(12)).toBe('stock?item=12');
	});

	it('text=1 turns the place code off: it goes there and back and asks the server as text', () => {
		const v = { ...plain, query: 'b-2', asText: true };
		expect(stockViewToSearch(v)).toBe('q=b-2&text=1');
		expect(fromSearch('q=b-2&text=1').asText).toBe(true);
		expect(fromSearch('q=b-2').asText).toBe(false);
		expect(stockViewToSearch(plain)).not.toContain('text');
		expect(stockViewToQuery(v)).toMatchObject({ query: 'b-2', as_text: true });
		expect(stockViewToQuery(plain)).not.toHaveProperty('as_text', true);
	});

	it('asks the server: trimmed text, the sort field, «Докупить»', () => {
		expect(stockViewToQuery(plain)).toEqual({
			query: undefined,
			restock: undefined,
			sort: 'name',
			descending: false
		});
		expect(stockViewToQuery({ ...plain, restock: true })).toEqual({
			restock: true,
			sort: 'name',
			descending: false
		});
		expect(
			stockViewToQuery({
				asText: false,
				query: '  экран ',
				sort: { columnId: 'quantity', descending: true },
				restock: false
			})
		).toEqual({ query: 'экран', sort: 'quantity', descending: true });
	});
});
