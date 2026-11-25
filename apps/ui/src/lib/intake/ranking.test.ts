import type { CatalogUsage } from '$api/CatalogUsage';
import { describe, expect, it, vi } from 'vitest';
import { loadDictionaries, rankByUsage } from './ranking.js';

const items = [
	{ id: 1, value: 'Чехол' },
	{ id: 2, value: 'Коробка' },
	{ id: 3, value: 'Шнур' }
];

describe('rankByUsage', () => {
	it('the most used first, the unused after them in the order of the dictionary', () => {
		expect(
			rankByUsage(items, [
				{ id: 3, count: 5 },
				{ id: 2, count: 9 }
			])
		).toEqual(['Коробка', 'Шнур', 'Чехол']);
	});

	it('without counts the order of the dictionary', () => {
		expect(rankByUsage(items, [])).toEqual(['Чехол', 'Коробка', 'Шнур']);
	});

	it('equal counts keep the order of the dictionary', () => {
		expect(
			rankByUsage(items, [
				{ id: 3, count: 2 },
				{ id: 1, count: 2 }
			])
		).toEqual(['Чехол', 'Шнур', 'Коробка']);
	});
});

describe('loadDictionaries', () => {
	type Kind = 'brand' | 'condition';
	const lists: Record<Kind, typeof items> = {
		brand: [{ id: 9, value: 'Samsung' }],
		condition: items
	};
	const flush = () => new Promise((r) => setTimeout(r, 0));

	/** Usage counts that come when the test says so. */
	function slowUsage() {
		const waiting: ((u: CatalogUsage[]) => void)[] = [];
		const usage = vi.fn(() => new Promise<CatalogUsage[]>((r) => waiting.push(r)));
		return { usage, answer: (u: CatalogUsage[]) => waiting.forEach((r) => r(u)) };
	}

	it('gives the lists at once in the order of the dictionary, the ranked one again when its counts come', async () => {
		const { usage, answer } = slowUsage();
		const show = vi.fn();
		const done = loadDictionaries(
			['brand', 'condition'],
			['condition'],
			{ list: async (k: Kind) => lists[k], usage },
			show
		);
		await flush();
		expect(usage).toHaveBeenCalledWith('condition');
		expect(show.mock.calls).toEqual([
			[{ brand: ['Samsung'], condition: ['Чехол', 'Коробка', 'Шнур'] }]
		]);
		answer([{ id: 3, count: 4 }]);
		await done;
		expect(show.mock.calls.slice(1)).toEqual([[{ condition: ['Шнур', 'Чехол', 'Коробка'] }]]);
	});

	it('counts that come before the lists still rank after them', async () => {
		const show = vi.fn();
		let giveLists: () => void = () => {};
		const slowLists = new Promise<void>((r) => (giveLists = r));
		const done = loadDictionaries(
			['brand', 'condition'],
			['condition'],
			{
				list: async (k: Kind) => (await slowLists, lists[k]),
				usage: async () => [{ id: 2, count: 1 }]
			},
			show
		);
		await flush();
		expect(show).not.toHaveBeenCalled();
		giveLists();
		await done;
		expect(show.mock.calls.map(([l]) => l.condition)).toEqual([
			['Чехол', 'Коробка', 'Шнур'],
			['Коробка', 'Чехол', 'Шнур']
		]);
	});

	it('counts refused or empty, a list failed: the order of the dictionary, the failed list empty', async () => {
		const show = vi.fn();
		await loadDictionaries(
			['brand', 'condition'],
			['brand', 'condition'],
			{
				list: async (k: Kind) => (k === 'brand' ? Promise.reject(new Error('нет')) : lists[k]),
				usage: async (k: Kind) => (k === 'brand' ? [] : Promise.reject(new Error('нет прав')))
			},
			show
		);
		expect(show.mock.calls).toEqual([[{ brand: [], condition: ['Чехол', 'Коробка', 'Шнур'] }]]);
	});
});
