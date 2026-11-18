import type { ErrorBody } from '$api/ErrorBody';
import type { ErrorCode } from '$api/ErrorCode';
import { ru } from '#lib/i18n/ru.js';
import { getToken } from './session.js';

type Scalar = string | number | boolean;
export type Query = Record<string, Scalar | null | undefined | (string | number)[]>;
export type Method = 'GET' | 'POST' | 'PATCH' | 'PUT' | 'DELETE';

const BASE = '/api/v1';

export class ApiError extends Error {
	status: number;
	code: ErrorCode | 'network';
	body: ErrorBody | null;
	constructor(
		status: number,
		code: ErrorCode | 'network',
		message: string,
		body: ErrorBody | null
	) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.code = code;
		this.body = body;
	}
}

/** Text for a toast or a form: the server's message, or a general one for anything else. */
export function errorMessage(e: unknown): string {
	return e instanceof ApiError && e.message ? e.message : ru.api.failed;
}

let onUnauthenticated: () => void = () => {};

export function setUnauthenticatedHandler(fn: () => void): void {
	onUnauthenticated = fn;
}

export function apiUrl(path: string, query?: Query): string {
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query ?? {})) {
		if (value === null || value === undefined) continue;
		if (Array.isArray(value)) for (const item of value) params.append(key, String(item));
		else params.append(key, String(value));
	}
	const qs = params.toString();
	return `${BASE}${path}${qs ? `?${qs}` : ''}`;
}

async function readBody(response: Response): Promise<unknown> {
	const text = await response.text();
	if (!text) return undefined;
	try {
		return JSON.parse(text);
	} catch {
		return undefined;
	}
}

function authHeaders(): Headers {
	const headers = new Headers();
	const token = getToken();
	if (token) headers.set('Authorization', `Bearer ${token}`);
	return headers;
}

async function send(
	method: Method,
	path: string,
	query: Query | undefined,
	init: { headers: Headers; body?: BodyInit; signal?: AbortSignal }
): Promise<Response> {
	try {
		return await fetch(apiUrl(path, query), { method, ...init });
	} catch (error) {
		if (error instanceof DOMException && error.name === 'AbortError') throw error;
		throw new ApiError(0, 'network', ru.api.offline, null);
	}
}

/** Turns a failed response into an ApiError (and tells the session about a 401). */
async function fail(response: Response, method: Method, path: string): Promise<never> {
	const data = await readBody(response);
	const isBody = typeof data === 'object' && data !== null && 'code' in data;
	const errorBody = isBody ? (data as ErrorBody) : null;
	if (response.status === 401 && !(method === 'POST' && path === '/auth/login')) {
		onUnauthenticated();
	}
	throw new ApiError(
		response.status,
		errorBody?.code ?? 'internal',
		errorBody?.message || ru.api.failed,
		errorBody
	);
}

export async function request<T>(
	method: Method,
	path: string,
	opts: { query?: Query; body?: unknown; signal?: AbortSignal } = {}
): Promise<T> {
	const headers = authHeaders();
	let body: string | undefined;
	// The server takes changing requests only as JSON, an empty one too (a page of another site cannot
	// send that without CORS).
	if (method !== 'GET') headers.set('Content-Type', 'application/json');
	if (opts.body !== undefined && method !== 'GET') body = JSON.stringify(opts.body);
	const response = await send(method, path, opts.query, { headers, body, signal: opts.signal });
	if (!response.ok) return fail(response, method, path);
	return (await readBody(response)) as T;
}

/** Sends a file as raw bytes (`application/octet-stream`: the one non-JSON type the server takes). */
export async function upload<T>(path: string, body: Blob, query?: Query): Promise<T> {
	const headers = authHeaders();
	headers.set('Content-Type', 'application/octet-stream');
	const response = await send('POST', path, query, { headers, body });
	if (!response.ok) return fail(response, 'POST', path);
	return (await readBody(response)) as T;
}

/** A binary response (an attachment's file or thumbnail), fetched with the token. */
export async function requestBlob(path: string, signal?: AbortSignal): Promise<Blob> {
	const response = await send('GET', path, undefined, { headers: authHeaders(), signal });
	if (!response.ok) return fail(response, 'GET', path);
	return response.blob();
}
