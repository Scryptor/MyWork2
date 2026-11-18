import type { Client } from '$api/Client';
import type { ClientListItem } from '$api/ClientListItem';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import { listClients } from '#lib/api/clients.js';
import { createPagedList } from '#lib/app/paged-list.svelte.js';

export { PAGE_SIZE } from '#lib/app/paged-list.svelte.js';

/**
 * The clients of the list, page by page as orders (see `createPagedList`). A client saved in the panel
 * updates its own fields of the row; the counts of orders stay as the list has them.
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
		}
	});
}

export type ClientList = ReturnType<typeof createClientList>;
