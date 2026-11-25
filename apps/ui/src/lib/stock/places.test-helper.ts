// Racks and positions for the unit tests of the places («Где лежит»).
import type { StockItem } from '$api/StockItem';
import type { StockPlace } from '$api/StockPlace';
import type { StockRack } from '$api/StockRack';

export const rack = (id: number, code: string, shelves: number, perShelf: number): StockRack => ({
	id,
	code,
	label: '',
	shelves,
	cells_per_shelf: perShelf,
	service_point_id: null,
	sort_order: id,
	cells: Array.from({ length: shelves * perShelf }, (_, i) => {
		const shelf = Math.floor(i / perShelf) + 1;
		const cell = (i % perShelf) + 1;
		const single = shelves === 1;
		return {
			shelf,
			cell,
			code: single ? `${code}-${cell}` : `${code}-${shelf}-${cell}`,
			positions: 0,
			found: false
		};
	})
});

export const place = (
	code: string,
	quantity: number,
	rack_id = 2,
	shelf = 2,
	cell = 4
): StockPlace => ({
	rack_id,
	shelf,
	cell,
	code,
	quantity
});

export const stockItem = (over: Partial<StockItem> = {}): StockItem => ({
	id: 7,
	name: '',
	display_name: 'Дисплей iPhone',
	device_type: 'Телефон',
	part_type: 'Дисплей',
	brand: 'Apple',
	model: 'iPhone 12',
	color: '',
	note: '',
	quantity: 5,
	price: 250000,
	restock_threshold: 2,
	needs_restock: false,
	created_at: '2026-11-24T10:00:00',
	updated_at: '2026-11-24T10:00:00',
	version: 1,
	places: [],
	unplaced: 5,
	...over
});
