import type { StockItem } from '$api/StockItem';
import type { StockListQuery } from '$api/StockListQuery';
import type { StockPage } from '$api/StockPage';
import { listStock } from '#lib/api/stock.js';
import { createPagedList } from '#lib/app/paged-list.svelte.js';

export { PAGE_SIZE } from '#lib/app/paged-list.svelte.js';

/** The positions of the list, page by page as orders and clients (see `createPagedList`). */
export function createStockList(
	fetchPage: (q: StockListQuery, signal?: AbortSignal) => Promise<StockPage> = listStock
) {
	return createPagedList<StockItem, StockListQuery>(fetchPage);
}

export type StockList = ReturnType<typeof createStockList>;
