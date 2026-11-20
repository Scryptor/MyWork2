import { deflateSync } from 'node:zlib';
import type { APIRequestContext, Page } from '@playwright/test';
import type { Attachment } from '$api/Attachment';
import type { Client } from '$api/Client';
import type { ClientPatch } from '$api/ClientPatch';
import type { DictionaryKind } from '$api/DictionaryKind';
import type { MasterUser } from '$api/MasterUser';
import type { NewClient } from '$api/NewClient';
import type { NewOrder } from '$api/NewOrder';
import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { Role } from '$api/Role';
import type { Status } from '$api/Status';
import type { User } from '$api/User';

/** Test password of the administrator created by `server.mjs --admin` (fictional). */
export const E2E_ADMIN_PASSWORD = 'e2e-test-password';
export const E2E_ADMIN_LOGIN = 'e2e-admin';

/**
 * The token of the page's session as the program stores it (`mw.session`: `{ token, user_id }`, in
 * the tab's own storage first); null when nobody is signed in.
 */
export async function sessionToken(page: Page): Promise<string | null> {
	return page.evaluate(() => {
		const raw = sessionStorage.getItem('mw.session') ?? localStorage.getItem('mw.session');
		if (raw === null) return null;
		try {
			return (JSON.parse(raw) as { token: string }).token;
		} catch {
			return raw;
		}
	});
}

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

/**
 * The active master named `name` (a user marked «Мастер»); made without sign-in, in the role «Мастер»,
 * when there is none.
 */
export async function masterId(request: APIRequestContext, name: string): Promise<number> {
	const masters = await json<MasterUser[]>(await request.get('/api/v1/masters'));
	const found = masters.find((m) => m.active && m.display_name === name);
	if (found) return found.id;
	const roles = await json<Role[]>(await request.get('/api/v1/roles'));
	const role = roles.find((r) => r.name === 'Мастер');
	if (!role) throw new Error('no role «Мастер»');
	const user = await json<User>(
		await request.post('/api/v1/users', {
			data: {
				login: '',
				display_name: name,
				role_id: role.id,
				password: '',
				can_sign_in: false,
				is_master: true
			}
		})
	);
	return user.id;
}

/**
 * Creates an order through the API; a new client with a unique name and phone unless given. `master` —
 * the name of a master user (made when there is none).
 */
export async function createOrder(
	request: APIRequestContext,
	over: Partial<NewOrder> & { clientName?: string; phone?: string; master?: string } = {}
): Promise<Order> {
	const { clientName, phone, master, ...rest } = over;
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
		master_user_id: master === undefined ? null : await masterId(request, master),
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

/** Creates a client through the API; a unique name and phone unless given. */
export async function createClient(
	request: APIRequestContext,
	over: Partial<NewClient> = {}
): Promise<Client> {
	const body: NewClient = {
		full_name: `Тестов ${uniq('К')}`,
		phones: [{ phone: uniqPhone(), note: '' }],
		address: '',
		note: '',
		source: '',
		blacklisted: false,
		...over
	};
	return json<Client>(await request.post('/api/v1/clients', { data: body }));
}

/** Gets a client as the server has it now. */
export async function fetchClient(request: APIRequestContext, id: number): Promise<Client> {
	return json<Client>(await request.get(`/api/v1/clients/${id}`));
}

/** Edits a client with its current version. */
export async function patchClient(
	request: APIRequestContext,
	id: number,
	patch: ClientPatch
): Promise<Client> {
	const current = await fetchClient(request, id);
	return json<Client>(
		await request.patch(`/api/v1/clients/${id}?version=${current.version}`, { data: patch })
	);
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

/** Adds a value to a dictionary (`warranty`, `brand`, …) through the API. */
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

/** A valid 2x2 PNG (fictional); `seed` changes the colour, so every picture has its own bytes. */
export function tinyPng(seed = 0): Buffer {
	const crcTable = Array.from({ length: 256 }, (_, n) => {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		return c >>> 0;
	});
	const crc = (buf: Buffer) => {
		let c = 0xffffffff;
		for (const b of buf) c = crcTable[(c ^ b) & 0xff]! ^ (c >>> 8);
		return (c ^ 0xffffffff) >>> 0;
	};
	const chunk = (type: string, data: Buffer) => {
		const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
		const out = Buffer.alloc(body.length + 8);
		out.writeUInt32BE(data.length, 0);
		body.copy(out, 4);
		out.writeUInt32BE(crc(body), body.length + 4);
		return out;
	};
	const header = Buffer.alloc(13);
	header.writeUInt32BE(2, 0);
	header.writeUInt32BE(2, 4);
	header[8] = 8; // bit depth
	header[9] = 2; // RGB
	const row = Buffer.from([0, seed & 255, 80, 160, 200, (seed >> 8) & 255, 40]);
	const raw = Buffer.concat([row, row]);
	return Buffer.concat([
		Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
		chunk('IHDR', header),
		chunk('IDAT', deflateSync(raw)),
		chunk('IEND', Buffer.alloc(0))
	]);
}

/** A minimal PDF (fictional) for file uploads. */
export const TINY_PDF = Buffer.from('%PDF-1.4\n%%EOF');

/** Uploads a picture or a file to an order the way the program does (an octet-stream body). */
export async function uploadPhoto(
	request: APIRequestContext,
	orderId: number,
	bytes: Buffer,
	name = 'photo.png'
): Promise<Attachment> {
	return json<Attachment>(
		await request.post(`/api/v1/orders/${orderId}/attachments?name=${encodeURIComponent(name)}`, {
			data: bytes,
			headers: { 'Content-Type': 'application/octet-stream' }
		})
	);
}
