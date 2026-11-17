import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError, apiUrl, errorMessage, request, setUnauthenticatedHandler } from './client.js';
import { clearToken, setToken } from './session.js';

function stubFetch(status: number, body: unknown) {
	const fn = vi.fn(
		async () => new Response(body === undefined ? null : JSON.stringify(body), { status })
	);
	vi.stubGlobal('fetch', fn);
	return fn;
}

afterEach(() => {
	vi.unstubAllGlobals();
	clearToken();
	setUnauthenticatedHandler(() => {});
});

describe('apiUrl', () => {
	it('repeats array keys and skips empty values', () => {
		expect(
			apiUrl('/orders', { status_ids: [1, 2], query: '', limit: 50, point: null, none: undefined })
		).toBe('/api/v1/orders?status_ids=1&status_ids=2&query=&limit=50');
	});
	it('has no question mark without parameters', () => {
		expect(apiUrl('/orders', {})).toBe('/api/v1/orders');
	});
});

describe('request', () => {
	it('throws ApiError with the server body', async () => {
		stubFetch(409, {
			code: 'conflict',
			message: 'Заказ изменили',
			fields: [],
			current_version: 7,
			permission: null,
			retry_after_secs: null
		});
		await expect(request('PATCH', '/orders/1')).rejects.toMatchObject({
			status: 409,
			code: 'conflict',
			message: 'Заказ изменили',
			body: { current_version: 7 }
		});
	});
	it('maps a network failure to code network', async () => {
		vi.stubGlobal('fetch', () => Promise.reject(new TypeError('Failed to fetch')));
		const error = await request('GET', '/x').catch((e: unknown) => e);
		expect(error).toBeInstanceOf(ApiError);
		expect(error).toMatchObject({ status: 0, code: 'network', message: 'Нет связи с сервером' });
	});
	it('rethrows an abort instead of calling it a network failure', async () => {
		vi.stubGlobal('fetch', () => Promise.reject(new DOMException('Aborted', 'AbortError')));
		await expect(request('GET', '/x')).rejects.toMatchObject({ name: 'AbortError' });
	});
	it('sends the bearer token and calls the 401 handler once', async () => {
		const handler = vi.fn();
		setUnauthenticatedHandler(handler);
		setToken('tok', true);
		const fn = stubFetch(401, {
			code: 'unauthenticated',
			message: 'Войдите',
			fields: [],
			current_version: null,
			permission: null,
			retry_after_secs: null
		});
		await expect(request('GET', '/orders')).rejects.toMatchObject({ status: 401 });
		expect(handler).toHaveBeenCalledTimes(1);
		const init = (fn.mock.calls[0] as unknown as [string, RequestInit])[1];
		expect(new Headers(init.headers).get('Authorization')).toBe('Bearer tok');
	});
	it('does not call the 401 handler for the login request', async () => {
		const handler = vi.fn();
		setUnauthenticatedHandler(handler);
		stubFetch(401, {
			code: 'invalid_credentials',
			message: 'Неверный пароль',
			fields: [],
			current_version: null,
			permission: null,
			retry_after_secs: null
		});
		await expect(request('POST', '/auth/login', { body: {} })).rejects.toMatchObject({
			code: 'invalid_credentials'
		});
		expect(handler).not.toHaveBeenCalled();
	});
	it('encodes a JSON body with the content type, GET sends none', async () => {
		const fn = stubFetch(200, { ok: 1 });
		await request('POST', '/x', { body: { a: 1 } });
		await request('GET', '/x', { body: { a: 1 } });
		const [, post] = fn.mock.calls[0] as unknown as [string, RequestInit];
		const [, get] = fn.mock.calls[1] as unknown as [string, RequestInit];
		expect(post.body).toBe('{"a":1}');
		expect(new Headers(post.headers).get('Content-Type')).toBe('application/json');
		expect(get.body).toBeUndefined();
	});
	it('a changing request without a body still says JSON: the server refuses it otherwise', async () => {
		const fn = stubFetch(200, { ok: 1 });
		await request('POST', '/orders/1/issue', { query: { version: 3 } });
		await request('GET', '/x');
		const [, post] = fn.mock.calls[0] as unknown as [string, RequestInit];
		const [, get] = fn.mock.calls[1] as unknown as [string, RequestInit];
		expect(new Headers(post.headers).get('Content-Type')).toBe('application/json');
		expect(post.body).toBeUndefined();
		expect(new Headers(get.headers).get('Content-Type')).toBeNull();
	});
	it('returns undefined for 204 and empty bodies', async () => {
		stubFetch(204, undefined);
		expect(await request('DELETE', '/x')).toBeUndefined();
		stubFetch(200, undefined);
		expect(await request('GET', '/x')).toBeUndefined();
	});
	it('falls back to a generic error for a non-JSON error body', async () => {
		vi.stubGlobal('fetch', async () => new Response('<html>', { status: 502 }));
		await expect(request('GET', '/x')).rejects.toMatchObject({ status: 502, body: null });
	});
});

describe('errorMessage', () => {
	it('is the message of an API error, a general text for anything else', () => {
		expect(errorMessage(new ApiError(0, 'network', 'Нет связи с сервером', null))).toBe(
			'Нет связи с сервером'
		);
		expect(errorMessage(new TypeError('x is undefined'))).toBe('Не удалось выполнить запрос');
	});
});
