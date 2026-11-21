import type { AuditPage } from '$api/AuditPage';
import type { NewOrderComment } from '$api/NewOrderComment';
import type { OrderComment } from '$api/OrderComment';
import type { OrderCommentPage } from '$api/OrderCommentPage';
import type { NewOrder } from '$api/NewOrder';
import type { NewWarrantyCase } from '$api/NewWarrantyCase';
import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import type { OrderPatch } from '$api/OrderPatch';
import type { OrderSummary } from '$api/OrderSummary';
import type { OrderValueField } from '$api/OrderValueField';
import type { HistoryQuery } from '$api/HistoryQuery';
import type { StatusChange } from '$api/StatusChange';
import { request, requestWithHeaders } from './client.js';

export const listOrders = (q: OrderListQuery, signal?: AbortSignal) =>
	request<OrderPage>('GET', '/orders', { query: q, signal });

export const getOrder = (id: number, signal?: AbortSignal) =>
	request<Order>('GET', `/orders/${id}`, { signal });

export const updateOrder = (id: number, version: number, patch: OrderPatch) =>
	request<Order>('PATCH', `/orders/${id}`, { query: { version }, body: patch });

export const changeStatus = (id: number, version: number, change: StatusChange) =>
	request<Order>('POST', `/orders/${id}/status`, { query: { version }, body: change });

export const issueOrder = (id: number, version: number) =>
	request<Order>('POST', `/orders/${id}/issue`, { query: { version } });

/** `replayed`: the server answered an attempt that had already made this order (`X-MW-Replayed: 1`). */
export async function createOrder(
	n: NewOrder,
	signal?: AbortSignal
): Promise<{ order: Order; replayed: boolean }> {
	const { data, headers } = await requestWithHeaders<Order>('POST', '/orders', { body: n, signal });
	return { order: data, replayed: headers.get('X-MW-Replayed') === '1' };
}

export const ordersByBarcode = (code: string) =>
	request<OrderListItem[]>('GET', '/orders/by-barcode', { query: { code } });

export const orderSummary = (pointId: number | null) =>
	request<OrderSummary>('GET', '/orders/summary', { query: { service_point_id: pointId } });

export const orderValues = (
	field: OrderValueField,
	query: string,
	brand?: string,
	signal?: AbortSignal
) => request<string[]>('GET', '/orders/values', { query: { field, query, brand }, signal });

export const orderHistory = (id: number, page?: HistoryQuery) =>
	request<AuditPage>('GET', `/orders/${id}/history`, { query: { ...page } });

export const acceptWarrantyCase = (id: number, version: number, c: NewWarrantyCase) =>
	request<Order>('POST', `/orders/${id}/warranty-case`, { query: { version }, body: c });

export const orderComments = (id: number, limit: number, beforeId?: number, signal?: AbortSignal) =>
	request<OrderCommentPage>('GET', `/orders/${id}/comments`, {
		query: { limit, before_id: beforeId },
		signal
	});

export const addOrderComment = (id: number, body: NewOrderComment) =>
	request<OrderComment>('POST', `/orders/${id}/comments`, { body });
