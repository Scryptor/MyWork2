// Editing one stock position on the server (decision 18): its fields over the shared editor — one queue
// of writes per position, text a moment after the last key, a 409 (someone wrote a part off meanwhile)
// merged with the newer version and the disputed fields asked about, as in a client.
import type { StockItem } from '$api/StockItem';
import type { StockItemPatch } from '$api/StockItemPatch';
import { ApiError } from '#lib/api/client.js';
import { getStockItem, updateStockItem } from '#lib/api/stock.js';
import {
	createEntityEditor,
	type Conflict,
	type EntityEditor
} from '#lib/app/entity-editor.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

const t = ru.stock;

/** The fields of the position panel. */
export const STOCK_EDIT_FIELDS = [
	'name',
	'part_type',
	'device_type',
	'brand',
	'model',
	'color',
	'note',
	'quantity',
	'price',
	'restock_threshold'
] as const satisfies readonly (keyof StockItem & keyof StockItemPatch)[];

export type StockEditField = (typeof STOCK_EDIT_FIELDS)[number];
export type StockValues = Pick<StockItem, StockEditField>;
export type StockConflict = Conflict<StockValues[StockEditField]>;

export interface StockEditorApi {
	getStockItem: (id: number) => Promise<StockItem>;
	updateStockItem: (id: number, version: number, patch: StockItemPatch) => Promise<StockItem>;
}

export interface StockEditorOptions {
	api?: StockEditorApi;
	/** Every answer of the server goes here: the list and the counter catch up. */
	emit?: (item: StockItem) => void;
	delayMs?: number;
}

/** The shared editor of one position; `item` is its record. */
export interface StockEditor extends EntityEditor<StockItem, StockEditField> {
	/** The position as the server last gave it. */
	readonly item: StockItem;
}

const DEFAULT_API: StockEditorApi = { getStockItem, updateStockItem };

/** A save no retry can help: the position is deleted, or editing is not allowed. */
function lostText(e: ApiError | null): string | null {
	if (e?.code === 'not_found') return t.lostDeleted;
	if (e?.code === 'forbidden') return e.message;
	return null;
}

export function createStockEditor(initial: StockItem, opts: StockEditorOptions = {}): StockEditor {
	const api = opts.api ?? DEFAULT_API;
	const ed = createEntityEditor<StockItem, StockEditField>({
		initial,
		fields: STOCK_EDIT_FIELDS,
		load: () => api.getStockItem(initial.id),
		save: (patch, version) => api.updateStockItem(initial.id, version, patch),
		emit: opts.emit,
		lostText,
		// The server trims the texts: the field keeps what was typed until it is left.
		keeps: (_field, typed, server) =>
			typeof typed === 'string' && typeof server === 'string' && typed.trim() === server,
		delayMs: opts.delayMs
	});
	return Object.defineProperty(ed, 'item', {
		get: () => ed.record,
		enumerable: true
	}) as StockEditor;
}

/** The position's name for texts: as the screen shows it. */
export const stockTitle = (i: Pick<StockItem, 'display_name'>): string => i.display_name;

/** Why the position's own changes are not saved: the failed save, or a conflict waiting for a choice. */
export function stockUnsavedText(ed: StockEditor): string {
	const reason =
		ed.conflicts.size > 0
			? t.conflictTitle
			: (ed.lost ?? ed.autosave.error?.message ?? ru.api.failed);
	return t.unsaved(stockTitle(ed.item), reason);
}

/**
 * An editor that is no longer shown (another position, the panel closed): what is left is saved; a
 * failure or an undecided conflict is told; then it stops.
 */
export function closeStockEditor(ed: StockEditor): void {
	void ed
		.flush()
		.then((ok) => {
			if (!ok) toast.show({ message: stockUnsavedText(ed), tone: 'danger' });
		})
		.finally(() => ed.dispose());
}
