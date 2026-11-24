import type { Attachment } from '$api/Attachment';
import type { AuditPage } from '$api/AuditPage';
import type { HistoryQuery } from '$api/HistoryQuery';
import type { NewStockItem } from '$api/NewStockItem';
import type { StockItem } from '$api/StockItem';
import type { StockItemPatch } from '$api/StockItemPatch';
import type { StockListQuery } from '$api/StockListQuery';
import type { StockPage } from '$api/StockPage';
import type { StockSummary } from '$api/StockSummary';
import { request, upload } from './client.js';

export const listStock = (q: StockListQuery, signal?: AbortSignal) =>
	request<StockPage>('GET', '/stock', { query: q, signal });

/** How many positions are to be bought («Докупить»): the counter of the sidebar. */
export const stockSummary = () => request<StockSummary>('GET', '/stock/summary');

export const getStockItem = (id: number) => request<StockItem>('GET', `/stock/${id}`);

export const createStockItem = (n: NewStockItem) =>
	request<StockItem>('POST', '/stock', { body: n });

export const updateStockItem = (id: number, version: number, p: StockItemPatch) =>
	request<StockItem>('PATCH', `/stock/${id}`, { query: { version }, body: p });

/** Soft delete: the position leaves the list; the parts written off from it stay in their orders. */
export const deleteStockItem = (id: number, version: number) =>
	request<void>('DELETE', `/stock/${id}`, { query: { version } });

export const stockItemHistory = (id: number, page?: HistoryQuery) =>
	request<AuditPage>('GET', `/stock/${id}/history`, { query: { ...page } });

export const listStockPhotos = (id: number) => request<Attachment[]>('GET', `/stock/${id}/photos`);

export const uploadStockPhoto = (id: number, file: File) =>
	upload<Attachment>(`/stock/${id}/photos`, file, { name: file.name });
