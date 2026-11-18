import type { Snippet } from 'svelte';

export type RowId = string | number;

export interface Column<Row> {
	/** Stable key: column preferences are stored under it. */
	id: string;
	header: string;
	/**
	 * Default width, px at the normal text size; the table scales widths with the UI scale. Of a `flex`
	 * column, its share of the spare width (as `fr`).
	 */
	width: number;
	/** Smallest width the user can drag to, px (60 when not set); a `flex` column shrinks to it. */
	minWidth?: number;
	/**
	 * Text (a name, a device, a malfunction): the column takes the spare width and shrinks down to its
	 * `minWidth` before the table scrolls sideways. Resized by the user, it keeps the width it was given.
	 */
	flex?: boolean;
	align?: 'start' | 'end' | 'center';
	/** The header click asks for sorting (`onsortchange`); the rows come sorted from outside. */
	sortable?: boolean;
	hiddenByDefault?: boolean;
	cell: Snippet<[Row]>;
}

export interface SortState {
	columnId: string;
	descending: boolean;
}

/** What the user changed in a table on this device. */
export interface ColumnPrefs {
	order: string[];
	hidden: string[];
	/** px by column id: only the columns the user resized. */
	widths: Record<string, number>;
}
