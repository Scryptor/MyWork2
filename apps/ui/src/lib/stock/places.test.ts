import { describe, expect, it } from 'vitest';
import type { StockItem } from '$api/StockItem';
import { place, rack } from './places.test-helper.js';
import { cellOptions, exceedText, placeCodes } from './places.js';

describe('placeCodes', () => {
	it('joins the codes of the places', () => {
		expect(placeCodes([place('B-2-4', 3), place('A-1', 1, 1, 1, 1)])).toBe('B-2-4, A-1');
		expect(placeCodes([])).toBe('');
	});
});

describe('cellOptions', () => {
	it('lists every cell in the order of the racks, with its reference', () => {
		const options = cellOptions([rack(1, 'A', 1, 2), rack(2, 'B', 2, 1)]);
		expect(options.map((o) => o.value)).toEqual(['A-1', 'A-2', 'B-1-1', 'B-2-1']);
		expect(options[1]).toEqual({ value: 'A-2', ref: { rack_id: 1, shelf: 1, cell: 2 } });
		expect(options[3]!.ref).toEqual({ rack_id: 2, shelf: 2, cell: 1 });
	});
});

describe('exceedText', () => {
	const item = { unplaced: 1, places: [place('B-2-4', 2)] } as unknown as StockItem;
	it('says how much is unplaced', () => {
		expect(exceedText(item)).toBe('Столько нет — не разложено 1 шт.');
	});
	it('says how much lies in the cell', () => {
		expect(exceedText(item, item.places[0])).toBe('Столько нет — в ячейке B-2-4 лежит 2 шт.');
	});
});
