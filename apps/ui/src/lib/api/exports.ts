import type { ClientExportQuery } from '$api/ClientExportQuery';
import type { ContactsFormat } from '$api/ContactsFormat';
import type { OrderExportQuery } from '$api/OrderExportQuery';
import { downloadBlob } from '#lib/platform/download.js';
import { requestFile } from './client.js';

/** The names of the files when the server gave none. */
const UNNAMED_ORDERS = 'orders.xlsx';
const UNNAMED_CLIENTS = 'clients.xlsx';
const UNNAMED_CONTACTS = { csv: 'contacts.csv', vcard: 'contacts.vcf' } as const;

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

/**
 * The phone contacts of the clients' list as CSV (Google) or vCard: the query is the list's, `createdFrom`
 * (a day «2026-11-20») keeps only the clients added since that day.
 */
export async function downloadContacts(
	query: ClientExportQuery,
	format: ContactsFormat,
	createdFrom: string | null
): Promise<void> {
	const file = await requestFile('/exports/contacts', {
		query: { ...query, format, ...(createdFrom ? { created_from: createdFrom } : {}) }
	});
	downloadBlob(file.filename ?? UNNAMED_CONTACTS[format], file.blob);
}
