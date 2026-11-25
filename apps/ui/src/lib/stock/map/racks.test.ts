import { describe, expect, it } from 'vitest';
import type { ServicePoint } from '$api/ServicePoint';
import type { StockRack } from '$api/StockRack';
import { rack } from '../places.test-helper.js';
import { cellParam, codesWillChange, moveRack, parseCellParam, pointOptions } from './racks.js';

const at = (r: StockRack, point: number | null): StockRack => ({ ...r, service_point_id: point });
const point = (id: number, name: string): ServicePoint => ({
	id,
	name,
	sort_order: id,
	archived: false
});

describe('codesWillChange', () => {
	const before = rack(1, 'B', 3, 5);
	it('is true when a side drops to one cell', () => {
		expect(codesWillChange(before, { code: 'B', shelves: 3, cells_per_shelf: 1 })).toBe(true);
	});
	it('is false while both sides stay over one', () => {
		expect(codesWillChange(before, { code: 'B', shelves: 4, cells_per_shelf: 6 })).toBe(false);
	});
	it('is true when the code changes', () => {
		expect(codesWillChange(before, { code: 'C', shelves: 3, cells_per_shelf: 5 })).toBe(true);
		expect(codesWillChange(before, { code: ' b ', shelves: 3, cells_per_shelf: 5 })).toBe(false);
	});
});

describe('moveRack', () => {
	const racks = [at(rack(1, 'A', 1, 1), 1), at(rack(2, 'B', 1, 1), 2), at(rack(3, 'C', 1, 1), 1)];
	it('swaps with the neighbour of the same point and gives the whole order', () => {
		expect(moveRack(racks, 3, -1)).toEqual([3, 2, 1]);
		expect(moveRack(racks, 1, 1)).toEqual([3, 2, 1]);
	});
	it('stops at the edges of the point', () => {
		expect(moveRack(racks, 1, -1)).toBeNull();
		expect(moveRack(racks, 3, 1)).toBeNull();
		expect(moveRack(racks, 2, 1)).toBeNull();
		expect(moveRack(racks, 99, 1)).toBeNull();
	});
});

describe('pointOptions', () => {
	const points = [point(1, 'Центр'), point(2, 'Север'), point(3, 'Юг')];
	it('lists the points that have racks, in the order of the points', () => {
		const racks = [at(rack(1, 'A', 1, 1), 3), at(rack(2, 'B', 1, 1), 1)];
		expect(pointOptions(racks, points)).toEqual([
			{ value: '1', label: 'Центр' },
			{ value: '3', label: 'Юг' }
		]);
	});
	it('adds «Без точки» only when a rack has none', () => {
		const racks = [at(rack(1, 'A', 1, 1), 1), at(rack(2, 'B', 1, 1), null)];
		expect(pointOptions(racks, points)).toEqual([
			{ value: '1', label: 'Центр' },
			{ value: 'none', label: 'Без точки' }
		]);
		expect(pointOptions([at(rack(1, 'A', 1, 1), 1)], points)).toHaveLength(1);
	});
});

describe('the cell in the address', () => {
	it('goes there and back', () => {
		expect(cellParam(3, 2, 4)).toBe('3.2.4');
		expect(parseCellParam('3.2.4')).toEqual({ rack_id: 3, shelf: 2, cell: 4 });
	});
	it('takes rubbish for no cell', () => {
		for (const s of [null, '', '3.2', '3.2.4.1', 'a.b.c', '0.1.1', '1.-1.1', '1.1.01'])
			expect(parseCellParam(s)).toBeNull();
	});
});
