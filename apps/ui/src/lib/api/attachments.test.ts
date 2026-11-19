import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clearAttachmentCache, attachmentUrl, uploadOrderAttachment } from './attachments.js';
import { clearToken, setToken } from './session.js';

let seq = 0;
const created: string[] = [];
const revoked: string[] = [];

function stubFetch(handler: (url: string, init: RequestInit) => Response | Promise<Response>) {
	const fn = vi.fn(async (url: string, init: RequestInit) => handler(url, init));
	vi.stubGlobal('fetch', fn);
	return fn;
}

beforeEach(() => {
	seq = 0;
	created.length = 0;
	clearAttachmentCache();
	revoked.length = 0;
	URL.createObjectURL = () => {
		const u = `blob:test/${++seq}`;
		created.push(u);
		return u;
	};
	URL.revokeObjectURL = (u: string) => void revoked.push(u);
});
afterEach(() => {
	vi.unstubAllGlobals();
	clearToken();
});

describe('attachmentUrl', () => {
	it('fetches once and serves the second request from the cache', async () => {
		const fetchFn = stubFetch(() => new Response('img'));
		const a = await attachmentUrl(1, 'thumb');
		const b = await attachmentUrl(1, 'thumb');
		expect(a).toBe(b);
		expect(a.startsWith('blob:')).toBe(true);
		expect(fetchFn).toHaveBeenCalledOnce();
		expect(fetchFn.mock.calls[0]![0]).toBe('/api/v1/attachments/1/thumb');
	});
	it('shares one request between concurrent callers', async () => {
		const fetchFn = stubFetch(() => new Response('img'));
		const [a, b] = await Promise.all([attachmentUrl(2, 'file'), attachmentUrl(2, 'file')]);
		expect(a).toBe(b);
		expect(fetchFn).toHaveBeenCalledOnce();
	});
	it('keeps variants apart and sends the token', async () => {
		setToken('tok', false);
		const fetchFn = stubFetch(() => new Response('img'));
		await attachmentUrl(3, 'thumb');
		await attachmentUrl(3, 'file');
		expect(fetchFn).toHaveBeenCalledTimes(2);
		const headers = fetchFn.mock.calls[0]![1].headers as Headers;
		expect(headers.get('Authorization')).toBe('Bearer tok');
	});
	it('does not cache a failure', async () => {
		let ok = false;
		const fetchFn = stubFetch(() =>
			ok
				? new Response('img')
				: new Response(JSON.stringify({ code: 'not_found', message: 'нет' }), { status: 404 })
		);
		await expect(attachmentUrl(4, 'thumb')).rejects.toBeTruthy();
		ok = true;
		await expect(attachmentUrl(4, 'thumb')).resolves.toMatch(/^blob:/);
		expect(fetchFn).toHaveBeenCalledTimes(2);
	});
	it('evicts the least recently used entry and revokes its URL after 200', async () => {
		stubFetch(() => new Response('img'));
		const first = await attachmentUrl(1, 'thumb');
		for (let id = 2; id <= 200; id++) await attachmentUrl(id, 'thumb');
		expect(revoked).toEqual([]);
		await attachmentUrl(201, 'thumb');
		expect(revoked).toEqual([first]);
	});
	it('a hit makes the entry recent', async () => {
		stubFetch(() => new Response('img'));
		const first = await attachmentUrl(1, 'thumb');
		const second = await attachmentUrl(2, 'thumb');
		for (let id = 3; id <= 200; id++) await attachmentUrl(id, 'thumb');
		await attachmentUrl(1, 'thumb');
		await attachmentUrl(201, 'thumb');
		expect(revoked).toEqual([second]);
		expect(revoked).not.toContain(first);
	});
});

describe('uploadOrderAttachment', () => {
	it('posts the bytes as octet-stream with the name in the query', async () => {
		const fetchFn = stubFetch(() => new Response(JSON.stringify({ id: 9 }), { status: 201 }));
		const file = new File(['abc'], 'a b.jpg', { type: 'image/jpeg' });
		await expect(uploadOrderAttachment(5, file)).resolves.toEqual({ id: 9 });
		const [url, init] = fetchFn.mock.calls[0]!;
		expect(url).toBe('/api/v1/orders/5/attachments?name=a+b.jpg');
		expect(init.method).toBe('POST');
		expect((init.headers as Headers).get('Content-Type')).toBe('application/octet-stream');
		expect(init.body).toBe(file);
	});
});
