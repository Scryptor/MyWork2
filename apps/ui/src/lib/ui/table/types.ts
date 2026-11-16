import type { Snippet } from 'svelte';

export type RowId = string | number;

export interface Column<Row> {
	/** Stable key: column preferences are stored under it. */
	id: string;
	header: string;
	/** Default width, px at the normal text size; the table scales widths with the UI scale. */
	width: number;
	/** Smallest width the user can drag to, px (60 when not set). */
	minWidth?: number;
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
	/** px by column id. */
	widths: Record<string, number>;
}
