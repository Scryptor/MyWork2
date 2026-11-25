// The map of the stock (plan 8.6): what the racks screen decides without the server — whether the codes of
// the cells change, which neighbour a rack swaps with, which points to offer, the cell in the address.
import type { ServicePoint } from '$api/ServicePoint';
import type { StockCellRef } from '$api/StockCellRef';
import type { StockRack } from '$api/StockRack';
import { ru } from '#lib/i18n/ru.js';

/**
 * The rule `codes_change` of the core: the code of the rack changes, or a side of the grid crosses one. The
 * stickers of the rack are printed anew then.
 */
export function codesWillChange(
	before: StockRack,
	after: { code: string; shelves: number; cells_per_shelf: number }
): boolean {
	return (
		after.code.trim().toUpperCase() !== before.code ||
		before.shelves > 1 !== after.shelves > 1 ||
		before.cells_per_shelf > 1 !== after.cells_per_shelf > 1
	);
}

/**
 * The order of all racks after `id` swaps with its neighbour of the same point (`dir` −1 — up, 1 — down);
 * `null` at the edge of the point: there is nothing to swap with.
 */
export function moveRack(racks: readonly StockRack[], id: number, dir: -1 | 1): number[] | null {
	const rack = racks.find((r) => r.id === id);
	if (!rack) return null;
	const mates = racks.filter((r) => r.service_point_id === rack.service_point_id);
	const neighbour = mates[mates.indexOf(rack) + dir];
	if (!neighbour) return null;
	const order = racks.map((r) => r.id);
	const a = order.indexOf(rack.id);
	const b = order.indexOf(neighbour.id);
	[order[a], order[b]] = [neighbour.id, rack.id];
	return order;
}

export const NO_POINT = 'none';

/** The points that have racks, in the order of the points; «Без точки» when a rack has none. */
export function pointOptions(
	racks: readonly StockRack[],
	points: readonly ServicePoint[]
): { value: string; label: string }[] {
	const used = new Set(racks.map((r) => r.service_point_id));
	const out = points
		.filter((p) => used.has(p.id))
		.map((p) => ({ value: String(p.id), label: p.name }));
	if (used.has(null)) out.push({ value: NO_POINT, label: ru.stock.map.noPoint });
	return out;
}

/** The racks of the chosen point (`'none'` — those without one). */
export const racksOfPoint = (racks: readonly StockRack[], value: string): StockRack[] =>
	racks.filter(
		(r) => (r.service_point_id === null ? NO_POINT : String(r.service_point_id)) === value
	);

/** The cell in the address (`?cell=3.2.4`): rack, shelf, cell. */
export const cellParam = (rackId: number, shelf: number, cell: number): string =>
	`${rackId}.${shelf}.${cell}`;

export function parseCellParam(s: string | null): StockCellRef | null {
	const m = s === null ? null : /^([1-9]\d*)\.([1-9]\d*)\.([1-9]\d*)$/.exec(s);
	return m ? { rack_id: Number(m[1]), shelf: Number(m[2]), cell: Number(m[3]) } : null;
}
