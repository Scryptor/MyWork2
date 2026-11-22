import type { ErrorBody } from '$api/ErrorBody';
import type { ErrorCode } from '$api/ErrorCode';
import { ru } from '#lib/i18n/ru.js';
import { followStorage, getToken, loseSession } from './session.js';

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

/**
 * A 401 of a `quietUnauthenticated` request that its caller has made sense of (a notice for the login page
 * queued first): the session ends after all, as on any other 401.
 */
export function signInAgain(): void {
	onUnauthenticated();
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

/**
 * The headers of a request of this page's session, with what another tab did to it: a new token of the
 * same user is taken up; when that tab ended the session (signed out, or in as another user) nothing is
 * sent and the session ends here too.
 */
function authHeaders(explicit?: string): Headers {
	// A request with a token of its own (revoking a lost session) is not this page's session.
	if (explicit !== undefined) return new Headers({ Authorization: `Bearer ${explicit}` });
	if (followStorage()) {
		loseSession();
		throw new ApiError(401, 'unauthenticated', ru.auth.sessionChanged, null);
	}
	const headers = new Headers();
	const token = getToken();
	if (token) headers.set('Authorization', `Bearer ${token}`);
	return headers;
}

async function send(
	method: Method,
	path: string,
	query: Query | undefined,
	init: { headers: Headers; body?: BodyInit; signal?: AbortSignal; keepalive?: boolean }
): Promise<Response> {
	try {
		return await fetch(apiUrl(path, query), { method, ...init });
	} catch (error) {
		if (error instanceof DOMException && error.name === 'AbortError') throw error;
		throw new ApiError(0, 'network', ru.api.offline, null);
	}
}

/**
 * Turns a failed response into an ApiError (and tells the session about a 401, unless the request is
 * `quiet`: its caller handles that itself).
 */
async function fail(
	response: Response,
	method: Method,
	path: string,
	quiet = false
): Promise<never> {
	const data = await readBody(response);
	const isBody = typeof data === 'object' && data !== null && 'code' in data;
	const errorBody = isBody ? (data as ErrorBody) : null;
	if (response.status === 401 && !quiet && !(method === 'POST' && path === '/auth/login')) {
		onUnauthenticated();
	}
	throw new ApiError(
		response.status,
		errorBody?.code ?? 'internal',
		errorBody?.message || ru.api.failed,
		errorBody
	);
}

export interface RequestOptions {
	query?: Query;
	body?: unknown;
	signal?: AbortSignal;
	/** The request outlives the page (a deletion sent as the page is left). */
	keepalive?: boolean;
	/**
	 * A 401 does not end the session of the program: the caller makes sense of it. The progress of an
	 * import: the base it has just written has users of its own, and the screen asks one of them to sign in.
	 */
	quietUnauthenticated?: boolean;
	/** Sent with this token instead of the page's session (the token of a session the page lost). */
	token?: string;
}

/** Like `request`, with the answer's headers. */
export async function requestWithHeaders<T>(
	method: Method,
	path: string,
	opts: RequestOptions = {}
): Promise<{ data: T; headers: Headers }> {
	const headers = authHeaders(opts.token);
	let body: string | undefined;
	// The server takes changing requests only as JSON, an empty one too (a page of another site cannot
	// send that without CORS).
	if (method !== 'GET') headers.set('Content-Type', 'application/json');
	if (opts.body !== undefined && method !== 'GET') body = JSON.stringify(opts.body);
	const response = await send(method, path, opts.query, {
		headers,
		body,
		signal: opts.signal,
		keepalive: opts.keepalive
	});
	if (!response.ok) return fail(response, method, path, opts.quietUnauthenticated);
	return { data: (await readBody(response)) as T, headers: response.headers };
}

export async function request<T>(
	method: Method,
	path: string,
	opts: RequestOptions = {}
): Promise<T> {
	return (await requestWithHeaders<T>(method, path, opts)).data;
}

/** Sends a file as raw bytes (`application/octet-stream`: the one non-JSON type the server takes). */
export async function upload<T>(path: string, body: Blob, query?: Query): Promise<T> {
	const headers = authHeaders();
	headers.set('Content-Type', 'application/octet-stream');
	const response = await send('POST', path, query, { headers, body });
	if (!response.ok) return fail(response, 'POST', path);
	return (await readBody(response)) as T;
}

/**
 * Only the name of a file, never a path: what follows the last `/` or `\`. Nothing, `.` or `..` is no
 * name. A download named «../../x.pdf» stays in the folder it is saved to.
 */
function baseName(name: string): string | null {
	const base = name.split(/[/\\]/).pop()?.trim() ?? '';
	return base === '' || base === '.' || base === '..' ? null : base;
}

/**
 * The name of a file in `Content-Disposition`: `filename*=UTF-8''…` (RFC 5987, an attachment's own name)
 * first, then `filename="…"` or `filename=…`; only its last part (`baseName`). Null when there is none,
 * it does not decode, or it is only a path.
 */
export function fileNameOf(header: string | null): string | null {
	if (header === null) return null;
	const encoded = /filename\*\s*=\s*UTF-8''([^;]+)/i.exec(header)?.[1];
	if (encoded !== undefined) {
		try {
			return baseName(decodeURIComponent(encoded.trim()));
		} catch {
			return null;
		}
	}
	const plain = /filename\s*=\s*(?:"([^"]*)"|([^;]+))/i.exec(header);
	return baseName(plain?.[1] ?? plain?.[2] ?? '');
}

/** A binary response with the name the server gave it (`Content-Disposition`), fetched with the token. */
export async function requestFile(
	path: string,
	opts: { query?: Query; signal?: AbortSignal } = {}
): Promise<{ blob: Blob; filename: string | null }> {
	const response = await send('GET', path, opts.query, {
		headers: authHeaders(),
		signal: opts.signal
	});
	if (!response.ok) return fail(response, 'GET', path);
	return {
		blob: await response.blob(),
		filename: fileNameOf(response.headers.get('Content-Disposition'))
	};
}

/** A binary response (an attachment's file or thumbnail), fetched with the token. */
export async function requestBlob(
	path: string,
	opts: { query?: Query; signal?: AbortSignal } = {}
): Promise<Blob> {
	return (await requestFile(path, opts)).blob;
}
