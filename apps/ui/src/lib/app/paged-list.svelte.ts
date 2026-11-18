// A list the server gives page by page (orders, clients). Each `load` aborts what is on its way;
// `loadMore` adds a page as a new array (the table asks for more once per array); a row changed here is
// updated in place and stays even if it no longer fits the view, so the list does not jump under the
// hand — the next load leaves it out.
import { ApiError } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

/** Rows asked for at a time. */
export const PAGE_SIZE = 100;

/** The paging fields every list query of the API has. */
export interface Paged {
	limit?: number;
	offset?: number;
}

export interface Page<Row> {
	items: Row[];
	total: number;
}

const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError';
const asApiError = (e: unknown) =>
	e instanceof ApiError ? e : new ApiError(0, 'internal', ru.api.failed, null);

export interface PagedList<Row, Q> {
	readonly rows: Row[];
	readonly total: number;
	readonly loading: boolean;
	readonly error: ApiError | null;
	/**
	 * The first page of a view. The rows of the last view stay until it comes; when it fails they go,
	 * since they are not what was asked for.
	 */
	load(q: Q): void;
	/** The next page; after a failure it may be asked again, the rows that came stay. */
	loadMore(): void;
	/**
	 * The row of this id as `change` makes it; `change` gives undefined to leave it as it is (an older
	 * answer). A row not in the list is not added.
	 */
	update(id: number, change: (row: Row) => Row | undefined): void;
}

export function createPagedList<Row extends { id: number }, Q extends Paged>(
	fetchPage: (q: Q, signal?: AbortSignal) => Promise<Page<Row>>
): PagedList<Row, Q> {
	let rows = $state.raw<Row[]>([]);
	let total = $state(0);
	let loading = $state(false);
	let error = $state.raw<ApiError | null>(null);
	let query = {} as Q;
	let controller: AbortController | null = null;

	function request(offset: number, done: (p: Page<Row>) => void, failed: () => void) {
		controller?.abort();
		const mine = new AbortController();
		controller = mine;
		loading = true;
		error = null;
		fetchPage({ ...query, limit: PAGE_SIZE, offset }, mine.signal).then(
			(page) => {
				if (controller !== mine) return;
				controller = null;
				loading = false;
				done(page);
			},
			(e: unknown) => {
				if (controller !== mine || isAbort(e)) return;
				controller = null;
				loading = false;
				error = asApiError(e);
				failed();
			}
		);
	}

	return {
		get rows() {
			return rows;
		},
		get total() {
			return total;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},
		load(q) {
			query = { ...q };
			request(
				0,
				(page) => {
					rows = page.items;
					total = page.total;
				},
				() => {
					rows = [];
					total = 0;
				}
			);
		},
		loadMore() {
			if (loading || rows.length >= total) return;
			request(
				rows.length,
				(page) => {
					// A row that moved between pages is not shown twice.
					const known = rows.map((r) => r.id);
					rows = [...rows, ...page.items.filter((r) => !known.includes(r.id))];
					total = page.total;
				},
				() => {}
			);
		},
		update(id, change) {
			const at = rows.findIndex((r) => r.id === id);
			const row = rows[at];
			if (row === undefined) return;
			const next = change(row);
			if (next !== undefined) rows = rows.with(at, next);
		}
	};
}
