// «Где лежит» (plan 8.6): the words and lists the place blocks share — codes of places, the cells to choose
// from, and what a refused quantity says.
import type { StockCellRef } from '$api/StockCellRef';
import type { StockItem } from '$api/StockItem';
import type { StockPlace } from '$api/StockPlace';
import type { StockRack } from '$api/StockRack';
import { ru } from '#lib/i18n/ru.js';

/** The codes of the places, as the list and the pick dialog show them: «B-2-4, A-1». */
export const placeCodes = (places: readonly Pick<StockPlace, 'code'>[]): string =>
	places.map((p) => p.code).join(', ');

export interface CellOption {
	/** The code of the cell: what the field shows and the user types. */
	value: string;
	ref: StockCellRef;
}

/** Every cell of every rack, in the order of the racks: what «Ячейка» offers. */
export function cellOptions(racks: readonly StockRack[]): CellOption[] {
	return racks.flatMap((r) =>
		r.cells.map((c) => ({ value: c.code, ref: { rack_id: r.id, shelf: c.shelf, cell: c.cell } }))
	);
}

export const sameCell = (a: StockCellRef, b: StockCellRef): boolean =>
	a.rack_id === b.rack_id && a.shelf === b.shelf && a.cell === b.cell;

/** The cell a place lies in. */
export const cellOf = (p: StockPlace): StockCellRef => ({
	rack_id: p.rack_id,
	shelf: p.shelf,
	cell: p.cell
});

/**
 * Why a quantity was refused (`places_exceed`), by the position as the server has it now: the server only
 * says that there is not so much, the screen says how much there is.
 */
export function exceedText(item: Pick<StockItem, 'unplaced'>, from?: StockPlace): string {
	const t = ru.stock.places;
	return from ? t.exceedCell(from.code, from.quantity) : t.exceedUnplaced(item.unplaced);
}
