import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import { listOrders } from '#lib/api/orders.js';
import { createPagedList } from '#lib/app/paged-list.svelte.js';

export { PAGE_SIZE } from '#lib/app/paged-list.svelte.js';

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
		discount: o.discount,
		need_call: o.need_call,
		need_parts: o.need_parts,
		barcode: o.barcode,
		deleted_at: o.deleted_at,
		version: o.version
	};
}

/**
 * The orders of the list, page by page (see `createPagedList`). An order changed here updates the order
 * fields of its row; an older answer changes nothing.
 */
export function createOrderList(
	fetchPage: (q: OrderListQuery, signal?: AbortSignal) => Promise<OrderPage> = listOrders
) {
	const list = createPagedList<OrderListItem, OrderListQuery>(fetchPage);
	return Object.assign(list, {
		replace(o: Order): void {
			list.update(o.id, (row) => (o.version < row.version ? undefined : withOrder(row, o)));
		}
	});
}

export type OrderList = ReturnType<typeof createOrderList>;
