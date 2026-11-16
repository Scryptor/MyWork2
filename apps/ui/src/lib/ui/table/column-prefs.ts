import type { Column, ColumnPrefs } from './types.js';

export const COLUMN_MIN_WIDTH = 60;
export const COLUMN_MAX_WIDTH = 800;

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
		const raw = localStorage.getItem(key);
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
 * ones go to the end with their default visibility, widths are clamped, garbage gives the defaults.
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

	const storedWidths = isRecord(s.widths) ? s.widths : {};
	const widths: Record<string, number> = {};
	for (const c of columns) {
		const w = storedWidths[c.id];
		widths[c.id] = clampColumnWidth(
			typeof w === 'number' && Number.isFinite(w) ? w : c.width,
			c.minWidth
		);
	}
	return { order, hidden, widths };
}

export function saveColumnPrefs(storageKey: string, prefs: ColumnPrefs): void {
	try {
		localStorage.setItem(columnPrefsKey(storageKey), JSON.stringify(prefs));
	} catch {
		// storage unavailable: the table keeps working with what it has in memory
	}
}
