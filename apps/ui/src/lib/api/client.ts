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

export async function request<T>(
	method: Method,
	path: string,
	opts: { query?: Query; body?: unknown; signal?: AbortSignal } = {}
): Promise<T> {
	const headers = new Headers();
	const token = getToken();
	if (token) headers.set('Authorization', `Bearer ${token}`);
	let body: string | undefined;
	// The server takes changing requests only as JSON, an empty one too (a page of another site cannot
	// send that without CORS).
	if (method !== 'GET') headers.set('Content-Type', 'application/json');
	if (opts.body !== undefined && method !== 'GET') body = JSON.stringify(opts.body);
	let response: Response;
	try {
		response = await fetch(apiUrl(path, opts.query), {
			method,
			headers,
			body,
			signal: opts.signal
		});
	} catch (error) {
		if (error instanceof DOMException && error.name === 'AbortError') throw error;
		throw new ApiError(0, 'network', ru.api.offline, null);
	}
	const data = await readBody(response);
	if (!response.ok) {
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
	return data as T;
}
