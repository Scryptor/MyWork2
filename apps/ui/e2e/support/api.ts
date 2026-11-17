import type { APIRequestContext } from '@playwright/test';
import type { DictionaryKind } from '$api/DictionaryKind';
import type { NewOrder } from '$api/NewOrder';
import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { Status } from '$api/Status';

/** Test password of the administrator created by `server.mjs --admin` (fictional). */
export const E2E_ADMIN_PASSWORD = 'e2e-test-password';
export const E2E_ADMIN_LOGIN = 'e2e-admin';

let counter = 0;

/** A unique marker so that tests do not see each other's data on a shared server. */
export function uniq(prefix = 'e2e'): string {
	counter += 1;
	return `${prefix}-${Date.now().toString(36)}${counter.toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

/** A fictional mobile number nobody else uses: «+7 911 …», createOrder's own are «+7 900 …». */
export function uniqPhone(): string {
	counter += 1;
	const digits = (Date.now() * 37 + counter * 7919 + Math.floor(Math.random() * 1e6)) % 1e7;
	return `+7911${String(digits).padStart(7, '0')}`;
}

async function json<T>(response: Awaited<ReturnType<APIRequestContext['get']>>): Promise<T> {
	if (!response.ok()) {
		throw new Error(`${response.status()} ${response.url()}: ${await response.text()}`);
	}
	return (await response.json()) as T;
}

/** Creates an order through the API; a new client with a unique name and phone unless given. */
export async function createOrder(
	request: APIRequestContext,
	over: Partial<NewOrder> & { clientName?: string; phone?: string } = {}
): Promise<Order> {
	const { clientName, phone, ...rest } = over;
	const digits = String(Date.now() + ++counter).slice(-7);
	const body: NewOrder = {
		client: {
			new: {
				full_name: clientName ?? `Тестов ${uniq('К')}`,
				phones: [{ phone: phone ?? `+7900${digits}`, note: '' }],
				address: '',
				note: '',
				source: '',
				blacklisted: false
			}
		},
		service_point_id: null,
		status_id: null,
		device_type: 'Смартфон',
		brand: 'Тест',
		model: 'Модель',
		serial: '',
		color: '',
		device_password: '',
		completeness: '',
		device_condition: '',
		malfunction: 'Не включается',
		internal_comment: '',
		master: '',
		estimate_from: null,
		estimate_to: null,
		prepayment: 0,
		warranty: '',
		need_call: false,
		need_parts: false,
		attachment_draft: null,
		request_key: null,
		...rest
	};
	return json<Order>(await request.post('/api/v1/orders', { data: body }));
}

export async function statusId(request: APIRequestContext, name: string): Promise<number> {
	const statuses = await json<Status[]>(await request.get('/api/v1/statuses'));
	const found = statuses.find((s) => s.name === name);
	if (!found) throw new Error(`no status named ${name}`);
	return found.id;
}

/** Edits an order with its current version. */
export async function patchOrder(
	request: APIRequestContext,
	id: number,
	patch: OrderPatch
): Promise<Order> {
	const current = await json<Order>(await request.get(`/api/v1/orders/${id}`));
	return json<Order>(
		await request.patch(`/api/v1/orders/${id}?version=${current.version}`, { data: patch })
	);
}

/** Adds a value to a dictionary (`master`, `warranty`, …) through the API. */
export async function addDictionaryItem(
	request: APIRequestContext,
	kind: DictionaryKind,
	value: string
): Promise<void> {
	await json<unknown>(await request.post(`/api/v1/dictionaries/${kind}`, { data: { value } }));
}

/** Gets an order as the server has it now. */
export async function fetchOrder(request: APIRequestContext, id: number): Promise<Order> {
	return json<Order>(await request.get(`/api/v1/orders/${id}`));
}

/** Sets a status through the API with the order's current version. */
export async function setStatus(
	request: APIRequestContext,
	id: number,
	status_id: number
): Promise<Order> {
	const current = await fetchOrder(request, id);
	return json<Order>(
		await request.post(`/api/v1/orders/${id}/status?version=${current.version}`, {
			data: { status_id }
		})
	);
}

/** Deletes an order through the API (soft delete) with its current version. */
export async function deleteOrder(request: APIRequestContext, id: number): Promise<void> {
	const current = await fetchOrder(request, id);
	const response = await request.delete(`/api/v1/orders/${id}?version=${current.version}`, {
		headers: { 'Content-Type': 'application/json' }
	});
	if (!response.ok())
		throw new Error(`${response.status()} ${response.url()}: ${await response.text()}`);
}
