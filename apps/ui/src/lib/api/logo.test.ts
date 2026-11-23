import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from './client.js';
import { getLogoInfo, logoUrl, releaseLogoUrl, removeLogo, uploadLogo } from './logo.js';

let seq = 0;
const revoked: string[] = [];

function stubFetch(handler: (url: string, init: RequestInit) => Response | Promise<Response>) {
	const fn = vi.fn(async (url: string, init: RequestInit) => handler(url, init));
	vi.stubGlobal('fetch', fn);
	return fn;
}
const path = (fn: ReturnType<typeof stubFetch>, i = 0) =>
	new URL(fn.mock.calls[i]![0], 'http://127.0.0.1').pathname;
const notFound = () =>
	new Response(JSON.stringify({ code: 'not_found', message: 'Нет логотипа' }), {
		status: 404,
		headers: { 'Content-Type': 'application/json' }
	});

beforeEach(() => {
	seq = 0;
	revoked.length = 0;
	URL.createObjectURL = () => `blob:logo/${++seq}`;
	URL.revokeObjectURL = (u: string) => void revoked.push(u);
});
afterEach(() => {
	releaseLogoUrl();
	vi.unstubAllGlobals();
});

describe('logo API', () => {
	it('reads the info, and a missing logo is null', async () => {
		const fn = stubFetch(() => new Response(JSON.stringify({ width: 4, height: 2, size: 70 })));
		expect(await getLogoInfo()).toEqual({ width: 4, height: 2, size: 70 });
		expect(path(fn)).toBe('/api/v1/settings/logo/info');
		stubFetch(notFound);
		expect(await getLogoInfo()).toBeNull();
	});

	it('any other failure of the info is thrown', async () => {
		stubFetch(() => new Response('{"code":"internal","message":"x"}', { status: 500 }));
		await expect(getLogoInfo()).rejects.toBeInstanceOf(ApiError);
	});

	it('gives a blob: address of the picture; no logo is null', async () => {
		const fn = stubFetch(() => new Response('png'));
		expect(await logoUrl()).toBe('blob:logo/1');
		expect(path(fn)).toBe('/api/v1/settings/logo');
		stubFetch(notFound);
		expect(await logoUrl()).toBeNull();
	});

	it('the address of an earlier picture is revoked when a new one is made', async () => {
		stubFetch(() => new Response('png'));
		const first = await logoUrl();
		const second = await logoUrl();
		expect(second).not.toBe(first);
		expect(revoked).toEqual([first]);
	});

	it('releaseLogoUrl revokes the current address, once', async () => {
		stubFetch(() => new Response('png'));
		const url = await logoUrl();
		releaseLogoUrl();
		releaseLogoUrl();
		expect(revoked).toEqual([url]);
	});

	it('uploads the file as raw bytes with PUT', async () => {
		const fn = stubFetch(() => new Response(JSON.stringify({ width: 1, height: 1, size: 3 })));
		const info = await uploadLogo(new Blob(['abc'], { type: 'image/png' }));
		expect(info.size).toBe(3);
		expect(path(fn)).toBe('/api/v1/settings/logo');
		const init = fn.mock.calls[0]![1];
		expect(init.method).toBe('PUT');
		expect(new Headers(init.headers).get('Content-Type')).toBe('application/octet-stream');
	});

	it('removes the logo with DELETE', async () => {
		const fn = stubFetch(() => new Response(null, { status: 204 }));
		await removeLogo();
		expect(fn.mock.calls[0]![1].method).toBe('DELETE');
		expect(path(fn)).toBe('/api/v1/settings/logo');
	});
});
