import type { Client } from '$api/Client';
import type { ClientListItem } from '$api/ClientListItem';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import { ApiError } from '#lib/api/client.js';
import { listClients } from '#lib/api/clients.js';
import { ru } from '#lib/i18n/ru.js';

/** Clients asked for at a time, as orders. */
export const CLIENTS_PAGE_SIZE = 100;

const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError';
const asApiError = (e: unknown) =>
	e instanceof ApiError ? e : new ApiError(0, 'internal', ru.api.failed, null);

/**
 * The clients of the list, page by page, as the orders list does it: each `load` aborts what is on its
 * way, `loadMore` adds a page as a new array, a client saved in the panel is updated in place.
 */
export function createClientList(
	fetchPage: (q: ClientListQuery, signal?: AbortSignal) => Promise<ClientPage> = listClients
) {
	let rows = $state.raw<ClientListItem[]>([]);
	let total = $state(0);
	let loading = $state(false);
	let error = $state.raw<ApiError | null>(null);
	let query: ClientListQuery = {};
	let controller: AbortController | null = null;

	function request(offset: number, done: (p: ClientPage) => void, failed: () => void) {
		controller?.abort();
		const mine = new AbortController();
		controller = mine;
		loading = true;
		error = null;
		fetchPage({ ...query, limit: CLIENTS_PAGE_SIZE, offset }, mine.signal).then(
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
		get rows(): ClientListItem[] {
			return rows;
		},
		get total(): number {
			return total;
		},
		get loading(): boolean {
			return loading;
		},
		get error(): ApiError | null {
			return error;
		},
		/** The first page of a view; the rows of the last one stay until it comes, and go if it fails. */
		load(q: ClientListQuery): void {
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
		/** The next page; after a failure it may be asked again, the rows that came stay. */
		loadMore(): void {
			if (loading || rows.length >= total) return;
			request(
				rows.length,
				(page) => {
					const known = rows.map((r) => r.id);
					rows = [...rows, ...page.items.filter((r) => !known.includes(r.id))];
					total = page.total;
				},
				() => {}
			);
		},
		/** The client as saved: its own fields; the counts of orders stay as the list has them. */
		replace(c: Client): void {
			const at = rows.findIndex((r) => r.id === c.id);
			const row = rows[at];
			if (row === undefined) return;
			rows = rows.with(at, {
				...row,
				full_name: c.full_name,
				phones: c.phones,
				source: c.source,
				blacklisted: c.blacklisted
			});
		}
	};
}

export type ClientList = ReturnType<typeof createClientList>;
