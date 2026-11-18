import { readStorage, writeStorage } from '#lib/design/storage.js';
import type { Column, ColumnPrefs } from './types.js';

export const COLUMN_MIN_WIDTH = 60;
export const COLUMN_MAX_WIDTH = 800;
/** Without `v: 2` the stored widths are ignored: before 5.3 every width was saved, resized or not. */
export const COLUMN_PREFS_VERSION = 2;

export function columnPrefsKey(storageKey: string): string {
	return 'mw.table.' + storageKey;
}

/** Whole pixels between the column's minimum (60 by default) and 800. */
export function clampColumnWidth(width: number, minWidth = COLUMN_MIN_WIDTH): number {
	return Math.round(Math.min(COLUMN_MAX_WIDTH, Math.max(minWidth, width)));
}

function isRecord(v: unknown): v is Record<string, unknown> {
	return typeof v === 'object' && v !== null && !Array.isArray(v);
}

function readStored(key: string): unknown {
	try {
		const raw = readStorage(key);
		return raw === null ? null : JSON.parse(raw);
	} catch {
		return null;
	}
}

function knownIds(value: unknown, known: (id: string) => boolean): string[] {
	if (!Array.isArray(value)) return [];
	const ids = value.filter((x): x is string => typeof x === 'string' && known(x));
	return [...new Set(ids)];
}

/**
 * Stored preferences merged with the current columns. Never throws: unknown columns are dropped, new
 * ones go to the end with their default visibility, widths (of the resized columns only) are clamped,
 * garbage gives the defaults.
 */
export function loadColumnPrefs(storageKey: string, columns: Column<never>[]): ColumnPrefs {
	const byId = new Map(columns.map((c) => [c.id, c]));
	const stored = readStored(columnPrefsKey(storageKey));
	const s = isRecord(stored) ? stored : {};

	const storedOrder = knownIds(s.order, (id) => byId.has(id));
	const seen = new Set(storedOrder);
	const added = columns.filter((c) => !seen.has(c.id));
	const order = [...storedOrder, ...added.map((c) => c.id)];

	const defaultHidden = columns.filter((c) => c.hiddenByDefault).map((c) => c.id);
	let hidden = [
		...knownIds(s.hidden, (id) => seen.has(id)),
		...added.filter((c) => c.hiddenByDefault).map((c) => c.id)
	];
	if (order.every((id) => hidden.includes(id))) hidden = defaultHidden;

	const storedWidths = s.v === COLUMN_PREFS_VERSION && isRecord(s.widths) ? s.widths : {};
	const widths: Record<string, number> = {};
	for (const c of columns) {
		const w = storedWidths[c.id];
		if (typeof w === 'number' && Number.isFinite(w)) widths[c.id] = clampColumnWidth(w, c.minWidth);
	}
	return { order, hidden, widths };
}

export function saveColumnPrefs(storageKey: string, prefs: ColumnPrefs): void {
	// storage unavailable: the table keeps working with what it has in memory
	writeStorage(columnPrefsKey(storageKey), JSON.stringify({ v: COLUMN_PREFS_VERSION, ...prefs }));
}
