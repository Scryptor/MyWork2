import { afterEach, describe, expect, it, vi } from 'vitest';
import { createRawSnippet } from 'svelte';
import {
	clampColumnWidth,
	columnPrefsKey,
	loadColumnPrefs,
	saveColumnPrefs
} from './column-prefs.js';
import type { Column } from './types.js';

const cell = createRawSnippet<[never]>(() => ({ render: () => '<span></span>' }));

const COLUMNS: Column<never>[] = [
	{ id: 'id', header: '№', width: 80, cell },
	{ id: 'client', header: 'Клиент', width: 200, minWidth: 120, cell },
	{ id: 'phone', header: 'Телефон', width: 150, hiddenByDefault: true, cell },
	{ id: 'master', header: 'Мастер', width: 110, cell }
];

const DEFAULTS = {
	order: ['id', 'client', 'phone', 'master'],
	hidden: ['phone'],
	widths: { id: 80, client: 200, phone: 150, master: 110 }
};

function store(value: unknown): void {
	localStorage.setItem(columnPrefsKey('orders'), JSON.stringify(value));
}

afterEach(() => {
	vi.restoreAllMocks();
	localStorage.clear();
});

describe('columnPrefsKey', () => {
	it('prefixes the table key', () => {
		expect(columnPrefsKey('orders')).toBe('mw.table.orders');
	});
});

describe('loadColumnPrefs', () => {
	it('nothing stored: declared order, hiddenByDefault hidden, default widths', () => {
		expect(loadColumnPrefs('orders', COLUMNS)).toEqual(DEFAULTS);
	});

	it('keeps a stored order, hidden list and widths', () => {
		store({
			order: ['master', 'id', 'client', 'phone'],
			hidden: ['id'],
			widths: { client: 260 }
		});
		expect(loadColumnPrefs('orders', COLUMNS)).toEqual({
			order: ['master', 'id', 'client', 'phone'],
			hidden: ['id'],
			widths: { id: 80, client: 260, phone: 150, master: 110 }
		});
	});

	it('drops a stored column the table no longer has', () => {
		store({
			order: ['gone', 'master', 'id', 'client', 'phone'],
			hidden: ['gone', 'phone'],
			widths: { gone: 300, master: 90 }
		});
		const prefs = loadColumnPrefs('orders', COLUMNS);
		expect(prefs.order).toEqual(['master', 'id', 'client', 'phone']);
		expect(prefs.hidden).toEqual(['phone']);
		expect(prefs.widths).not.toHaveProperty('gone');
		expect(prefs.widths.master).toBe(90);
	});

	it('appends a new column at the end, hidden if it is hidden by default', () => {
		store({ order: ['master', 'client', 'id'], hidden: [], widths: {} });
		const prefs = loadColumnPrefs('orders', COLUMNS);
		expect(prefs.order).toEqual(['master', 'client', 'id', 'phone']);
		expect(prefs.hidden).toEqual(['phone']);

		store({ order: ['master', 'client', 'phone'], hidden: ['phone'], widths: {} });
		const withNewVisible = loadColumnPrefs('orders', COLUMNS);
		expect(withNewVisible.order).toEqual(['master', 'client', 'phone', 'id']);
		expect(withNewVisible.hidden).toEqual(['phone']);
	});

	it('raises a width of 10 to minWidth and caps 5000 at 800', () => {
		store({ order: DEFAULTS.order, hidden: [], widths: { id: 10, client: 10, master: 5000 } });
		const { widths } = loadColumnPrefs('orders', COLUMNS);
		expect(widths.id).toBe(60);
		expect(widths.client).toBe(120);
		expect(widths.master).toBe(800);
	});

	it('ignores widths that are not finite numbers', () => {
		// JSON reads 1e999 as Infinity.
		localStorage.setItem(
			columnPrefsKey('orders'),
			'{"order":["id","client","phone","master"],"hidden":[],' +
				'"widths":{"id":"300","client":null,"master":1e999}}'
		);
		expect(loadColumnPrefs('orders', COLUMNS).widths).toEqual(DEFAULTS.widths);
	});

	it('garbage in storage gives the defaults', () => {
		for (const raw of ['{bad', '[]', 'null', '42', '"x"', '{"order":5,"hidden":"x","widths":[]}']) {
			localStorage.setItem(columnPrefsKey('orders'), raw);
			expect(loadColumnPrefs('orders', COLUMNS), raw).toEqual(DEFAULTS);
		}
	});

	it('every column hidden: visibility falls back to the defaults', () => {
		store({ order: DEFAULTS.order, hidden: ['id', 'client', 'phone', 'master'], widths: {} });
		expect(loadColumnPrefs('orders', COLUMNS).hidden).toEqual(['phone']);
	});

	it('a throwing storage gives the defaults', () => {
		vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
			throw new Error('denied');
		});
		expect(loadColumnPrefs('orders', COLUMNS)).toEqual(DEFAULTS);
	});
});

describe('saveColumnPrefs', () => {
	it('round-trips through loadColumnPrefs', () => {
		const prefs = {
			order: ['client', 'id', 'master', 'phone'],
			hidden: ['master'],
			widths: { id: 90, client: 240, phone: 150, master: 110 }
		};
		saveColumnPrefs('orders', prefs);
		expect(JSON.parse(localStorage.getItem('mw.table.orders')!)).toEqual(prefs);
		expect(loadColumnPrefs('orders', COLUMNS)).toEqual(prefs);
	});

	it('does not throw when storage refuses', () => {
		vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
			throw new Error('quota');
		});
		expect(() => saveColumnPrefs('orders', DEFAULTS)).not.toThrow();
	});
});

describe('clampColumnWidth', () => {
	it('keeps widths between minWidth (60 by default) and 800, in whole pixels', () => {
		expect(clampColumnWidth(10)).toBe(60);
		expect(clampColumnWidth(10, 120)).toBe(120);
		expect(clampColumnWidth(5000)).toBe(800);
		expect(clampColumnWidth(200.4)).toBe(200);
	});
});
