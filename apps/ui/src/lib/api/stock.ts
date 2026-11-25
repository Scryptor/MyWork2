import type { Attachment } from '$api/Attachment';
import type { AuditPage } from '$api/AuditPage';
import type { HistoryQuery } from '$api/HistoryQuery';
import type { NewOrderPart } from '$api/NewOrderPart';
import type { NewStockItem } from '$api/NewStockItem';
import type { OrderParts } from '$api/OrderParts';
import type { StockCellRef } from '$api/StockCellRef';
import type { StockItem } from '$api/StockItem';
import type { StockItemPatch } from '$api/StockItemPatch';
import type { StockListQuery } from '$api/StockListQuery';
import type { StockMove } from '$api/StockMove';
import type { StockPage } from '$api/StockPage';
import type { StockPut } from '$api/StockPut';
import type { StockRack } from '$api/StockRack';
import type { StockRackQuery } from '$api/StockRackQuery';
import type { StockSummary } from '$api/StockSummary';
import { request, upload } from './client.js';

export const listStock = (q: StockListQuery, signal?: AbortSignal) =>
	request<StockPage>('GET', '/stock', { query: q, signal });

/** The racks with their cells; the cells of a matching position are marked `found`. */
export const listStockRacks = (q: StockRackQuery) =>
	request<StockRack[]>('GET', '/stock/racks', { query: q });

/** Lays units from «Не разложено» into a cell; more than is unplaced is 422 `places_exceed`. */
export const putStock = (id: number, body: StockPut) =>
	request<StockItem>('POST', `/stock/${id}/places`, { body });

/** Moves units from one cell to another. */
export const moveStock = (id: number, body: StockMove) =>
	request<StockItem>('POST', `/stock/${id}/places/move`, { body });

/** Takes everything out of a cell back into «Не разложено». */
export const unplaceStock = (id: number, at: StockCellRef) =>
	request<StockItem>('DELETE', `/stock/${id}/places/${at.rack_id}/${at.shelf}/${at.cell}`);

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

/** The parts written off into an order and their sum (whoever may see the order). */
export const listOrderParts = (orderId: number) =>
	request<OrderParts>('GET', `/orders/${orderId}/parts`);

/** Writes a position off into an order; the same `request_key` writes off once (a repeat after a lost answer). */
export const addOrderPart = (orderId: number, n: NewOrderPart) =>
	request<OrderParts>('POST', `/orders/${orderId}/parts`, { body: n });

/** Gives a part back: the stock gets it back when the position is alive; 404 — the row is already gone. */
export const returnOrderPart = (orderId: number, partId: number) =>
	request<OrderParts>('DELETE', `/orders/${orderId}/parts/${partId}`);
