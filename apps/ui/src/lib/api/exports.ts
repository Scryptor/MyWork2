import type { ClientExportQuery } from '$api/ClientExportQuery';
import type { OrderExportQuery } from '$api/OrderExportQuery';
import { downloadBlob } from '#lib/platform/download.js';
import { requestFile } from './client.js';

/** The names of the files when the server gave none. */
const UNNAMED_ORDERS = 'orders.xlsx';
const UNNAMED_CLIENTS = 'clients.xlsx';

/**
 * The orders of the list as a file of Excel. The query is the one the list sends (`viewToQuery`): the
 * same filter and order, no paging.
 */
export async function downloadOrders(query: OrderExportQuery): Promise<void> {
	const file = await requestFile('/exports/orders', { query });
	downloadBlob(file.filename ?? UNNAMED_ORDERS, file.blob);
}

/** The clients of the list as a file of Excel; the query is the one the list sends (`clientsViewToQuery`). */
export async function downloadClients(query: ClientExportQuery): Promise<void> {
	const file = await requestFile('/exports/clients', { query });
	downloadBlob(file.filename ?? UNNAMED_CLIENTS, file.blob);
}
