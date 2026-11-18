import { describe, expect, it } from 'vitest';
import {
	clientHref,
	clientsSearch,
	clientsViewFromSearch,
	clientsViewToQuery,
	clientsViewToSearch,
	defaultClientsView,
	openClientId,
	shownChips,
	withChips,
	type ClientsView
} from './view.js';

const fromSearch = (s: string) => clientsViewFromSearch(new URLSearchParams(s));

describe('clients view in the address', () => {
	it('the default view is an empty address: last order, newest first, everyone', () => {
		expect(defaultClientsView()).toEqual({
			query: '',
			sort: { columnId: 'last', descending: true },
			duplicates: false
		});
		expect(clientsViewToSearch(defaultClientsView())).toBe('');
		expect(fromSearch('')).toEqual(defaultClientsView());
	});

	it('goes there and back: q, sort, desc, dup', () => {
		const views: ClientsView[] = [
			{ query: 'иванов 921', sort: { columnId: 'orders', descending: true }, duplicates: false },
			{ query: '', sort: { columnId: 'name', descending: false }, duplicates: true },
			{ query: 'a,b', sort: { columnId: 'last', descending: false }, duplicates: false }
		];
		for (const v of views) expect(fromSearch(clientsViewToSearch(v))).toEqual(v);
		expect(
			clientsViewToSearch({
				query: 'Петров',
				sort: { columnId: 'orders', descending: true },
				duplicates: true
			})
		).toBe('q=%D0%9F%D0%B5%D1%82%D1%80%D0%BE%D0%B2&sort=orders&desc=1&dup=1');
	});

	it('anything it does not know falls back to the default', () => {
		expect(fromSearch('sort=phones&desc=x&dup=yes')).toEqual(defaultClientsView());
		expect(fromSearch('sort=name')).toEqual({
			...defaultClientsView(),
			sort: { columnId: 'name', descending: true }
		});
	});

	it('?client= opens a client and stays beside the view', () => {
		expect(openClientId(new URLSearchParams('client=42&q=x'))).toBe(42);
		expect(openClientId(new URLSearchParams('client=0'))).toBeNull();
		expect(openClientId(new URLSearchParams('client=abc'))).toBeNull();
		expect(openClientId(new URLSearchParams(''))).toBeNull();
		const v = { ...defaultClientsView(), query: 'иван' };
		expect(clientsSearch(v, 42)).toBe('q=%D0%B8%D0%B2%D0%B0%D0%BD&client=42');
		expect(clientsSearch(v, null)).toBe('q=%D0%B8%D0%B2%D0%B0%D0%BD');
		expect(clientsSearch(defaultClientsView(), 7)).toBe('client=7');
		// The panel does not change the view.
		expect(fromSearch(clientsSearch(v, 42))).toEqual(v);
	});

	it('a client of Ctrl K or of an order opens on «Клиенты»', () => {
		expect(clientHref(15)).toBe('clients?client=15');
	});
});

describe('clientsViewToQuery', () => {
	it('sends the sort of the column and the text without spaces around', () => {
		expect(clientsViewToQuery(defaultClientsView())).toEqual({
			query: undefined,
			sort: 'last_order',
			descending: true,
			duplicates: undefined
		});
		expect(
			clientsViewToQuery({
				query: '  921 ',
				sort: { columnId: 'orders', descending: false },
				duplicates: false
			})
		).toEqual({ query: '921', sort: 'orders_count', descending: false, duplicates: undefined });
		expect(
			clientsViewToQuery({
				query: '',
				sort: { columnId: 'name', descending: true },
				duplicates: false
			})
		).toMatchObject({ sort: 'name', descending: true });
	});

	it('«Похожие» are in the server order (by name): no sort is sent', () => {
		expect(
			clientsViewToQuery({
				query: 'иван',
				sort: { columnId: 'orders', descending: true },
				duplicates: true
			})
		).toEqual({ query: 'иван', duplicates: true });
	});
});

describe('chips «Все» / «Похожие»', () => {
	it('shows one of them pressed', () => {
		expect(shownChips(defaultClientsView())).toEqual(['all']);
		expect(shownChips({ ...defaultClientsView(), duplicates: true })).toEqual(['similar']);
	});

	it('a click picks one; Ctrl+click on the other one picks that one too', () => {
		const all = defaultClientsView();
		expect(withChips(all, ['similar']).duplicates).toBe(true);
		expect(withChips(all, ['all', 'similar']).duplicates).toBe(true);
		const similar = { ...all, duplicates: true };
		expect(withChips(similar, ['all']).duplicates).toBe(false);
		expect(withChips(similar, ['similar', 'all']).duplicates).toBe(false);
		// Ctrl+click on the pressed one leaves nothing pressed: nothing changes.
		expect(withChips(similar, [])).toEqual(similar);
		expect(withChips(all, ['all'])).toEqual(all);
	});
});
