import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import { ApiError } from '#lib/api/client.js';
import { listOrders } from '#lib/api/orders.js';
import { ru } from '#lib/i18n/ru.js';

/** Orders asked for at a time. */
export const PAGE_SIZE = 100;

const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError';
const asApiError = (e: unknown) =>
	e instanceof ApiError ? e : new ApiError(0, 'internal', ru.api.failed, null);

/** The fields of a row that come from the order itself; the client's stay as the list has them. */
function withOrder(row: OrderListItem, o: Order): OrderListItem {
	return {
		...row,
		received_at: o.received_at,
		issued_at: o.issued_at,
		service_point_id: o.service_point_id,
		status_id: o.status_id,
		device_type: o.device_type,
		brand: o.brand,
		model: o.model,
		serial: o.serial,
		malfunction: o.malfunction,
		master: o.master,
		estimate_from: o.estimate_from,
		estimate_to: o.estimate_to,
		prepayment: o.prepayment,
		final_price: o.final_price,
		need_call: o.need_call,
		need_parts: o.need_parts,
		barcode: o.barcode,
		deleted_at: o.deleted_at,
		version: o.version
	};
}

/**
 * The orders of the list, page by page. Each `load` aborts what is on its way; `loadMore` adds a page
 * as a new array (the table asks for more once per array). An order changed here is updated in place
 * and stays even if it no longer fits the view, so the list does not jump under the hand; the next
 * load leaves it out.
 */
export function createOrderList(
	fetchPage: (q: OrderListQuery, signal?: AbortSignal) => Promise<OrderPage> = listOrders
) {
	let rows = $state.raw<OrderListItem[]>([]);
	let total = $state(0);
	let loading = $state(false);
	let error = $state.raw<ApiError | null>(null);
	let query: OrderListQuery = {};
	let controller: AbortController | null = null;

	function request(offset: number, done: (p: OrderPage) => void, failed: () => void) {
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
		get rows(): OrderListItem[] {
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
		/**
		 * The first page of a view. The rows of the last view stay until it comes; when it fails they
		 * go, since they are not what was asked for.
		 */
		load(q: OrderListQuery): void {
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
		replace(o: Order): void {
			const at = rows.findIndex((r) => r.id === o.id);
			const row = rows[at];
			if (row === undefined || o.version < row.version) return;
			rows = rows.with(at, withOrder(row, o));
		}
	};
}

export type OrderList = ReturnType<typeof createOrderList>;
