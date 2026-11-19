import type { Client } from '$api/Client';
import type { ClientListItem } from '$api/ClientListItem';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import type { Order } from '$api/Order';
import { listClients } from '#lib/api/clients.js';
import { createPagedList } from '#lib/app/paged-list.svelte.js';

export { PAGE_SIZE } from '#lib/app/paged-list.svelte.js';

/**
 * The clients of the list, page by page as orders (see `createPagedList`). A client saved in the panel
 * updates its own fields of the row; the counts of orders stay as the list has them until an order is
 * accepted (`orderCreated`).
 */
export function createClientList(
	fetchPage: (q: ClientListQuery, signal?: AbortSignal) => Promise<ClientPage> = listClients
) {
	const list = createPagedList<ClientListItem, ClientListQuery>(fetchPage);
	return Object.assign(list, {
		replace(c: Client): void {
			list.update(c.id, (row) => ({
				...row,
				full_name: c.full_name,
				phones: c.phones,
				source: c.source,
				blacklisted: c.blacklisted
			}));
		},
		/**
		 * An order accepted: its client's row counts it and shows it as the last one. Only that row
		 * changes, no page is asked again; a client not in the list is not added.
		 */
		orderCreated(o: Order): void {
			list.update(o.client_id, (row) => ({
				...row,
				orders_count: row.orders_count + 1,
				last_order_at:
					row.last_order_at !== null && row.last_order_at > o.received_at
						? row.last_order_at
						: o.received_at
			}));
		}
	});
}

export type ClientList = ReturnType<typeof createClientList>;
