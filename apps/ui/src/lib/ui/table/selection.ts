import type { RowId } from './types.js';

/**
 * Ids from `anchor` to `target` inclusive, in list order (Shift+click).
 * Without the anchor in the list only the target is selected; a target outside the list selects nothing.
 */
export function rangeSelect(ids: RowId[], anchor: RowId, target: RowId): RowId[] {
	const to = ids.indexOf(target);
	if (to < 0) return [];
	const from = ids.indexOf(anchor);
	if (from < 0) return [target];
	return ids.slice(Math.min(from, to), Math.max(from, to) + 1);
}
