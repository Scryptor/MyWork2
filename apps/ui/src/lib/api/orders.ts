import type { AuditPage } from '$api/AuditPage';
import type { NewOrder } from '$api/NewOrder';
import type { NewWarrantyCase } from '$api/NewWarrantyCase';
import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import type { OrderPatch } from '$api/OrderPatch';
import type { OrderSummary } from '$api/OrderSummary';
import type { OrderValueField } from '$api/OrderValueField';
import type { PageQuery } from '$api/PageQuery';
import type { StatusChange } from '$api/StatusChange';
import { request } from './client.js';

export const listOrders = (q: OrderListQuery, signal?: AbortSignal) =>
	request<OrderPage>('GET', '/orders', { query: q, signal });

export const getOrder = (id: number) => request<Order>('GET', `/orders/${id}`);

export const updateOrder = (id: number, version: number, patch: OrderPatch) =>
	request<Order>('PATCH', `/orders/${id}`, { query: { version }, body: patch });

export const changeStatus = (id: number, version: number, change: StatusChange) =>
	request<Order>('POST', `/orders/${id}/status`, { query: { version }, body: change });

export const issueOrder = (id: number, version: number) =>
	request<Order>('POST', `/orders/${id}/issue`, { query: { version } });

export const createOrder = (n: NewOrder) => request<Order>('POST', '/orders', { body: n });

export const ordersByBarcode = (code: string) =>
	request<OrderListItem[]>('GET', '/orders/by-barcode', { query: { code } });

export const orderSummary = (pointId: number | null) =>
	request<OrderSummary>('GET', '/orders/summary', { query: { service_point_id: pointId } });

export const orderValues = (field: OrderValueField, query: string, brand?: string) =>
	request<string[]>('GET', '/orders/values', { query: { field, query, brand } });

export const orderHistory = (id: number, page?: PageQuery) =>
	request<AuditPage>('GET', `/orders/${id}/history`, { query: { ...page } });

export const acceptWarrantyCase = (id: number, version: number, c: NewWarrantyCase) =>
	request<Order>('POST', `/orders/${id}/warranty-case`, { query: { version }, body: c });
