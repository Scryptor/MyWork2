import type { Attachment } from '$api/Attachment';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { resetSession } from '#lib/app/session-reset.js';
import { settleQueues } from '#lib/app/session-reset.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	createDraftMedia,
	createOrderMedia,
	type DraftGuard,
	limitPhotos,
	MAX_PHOTOS_PER_ADD,
	type MediaApi
} from './photos.svelte.js';

function attachment(id: number, over: Partial<Attachment> = {}): Attachment {
	return {
		id,
		order_id: 7,
		kind: 'photo',
		name: `p${id}.png`,
		caption: '',
		content_type: 'image/png',
		size: 10,
		width: 2,
		height: 2,
		legacy: false,
		missing: false,
		created_at: '2026-11-17T10:00:00',
		...over
	};
}

const file = (name: string) => new File(['x'], name, { type: 'image/png' });
const messages = () => toast.items.map((t) => t.message);

function deferred<T>() {
	let resolve!: (v: T) => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function fakeApi(over: Partial<MediaApi> = {}): MediaApi {
	let next = 100;
	return {
		list: vi.fn(async () => [attachment(1), attachment(2, { kind: 'file', name: 'a.pdf' })]),
		upload: vi.fn(async (_id: number, f: File) => attachment(next++, { name: f.name })),
		caption: vi.fn(async (id: number, caption: string) => attachment(id, { caption })),
		remove: vi.fn(async () => undefined),
		...over
	};
}

const tooLarge = (message: string) =>
	new ApiError(422, 'validation', message, {
		code: 'validation',
		message,
		fields: [{ field: 'file', code: 'too_large', message }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

afterEach(() => {
	for (const t of [...toast.items]) toast.dismiss(t.id);
	vi.useRealTimers();
});

describe('createOrderMedia', () => {
	it('loads and splits photos from files by kind', async () => {
		const m = createOrderMedia(7, fakeApi());
		await m.reload();
		expect(m.photos.map((a) => a.id)).toEqual([1]);
		expect(m.files.map((a) => a.id)).toEqual([2]);
		expect(m.loading).toBe(false);
		expect(m.error).toBeNull();
	});

	it('says why the list did not load', async () => {
		const m = createOrderMedia(7, fakeApi({ list: vi.fn().mockRejectedValue(new Error('x')) }));
		await m.reload();
		expect(m.error).not.toBeNull();
	});

	it('uploads one at a time, each with a «Загружаю…» tile that the server photo replaces', async () => {
		const first = deferred<Attachment>();
		const second = deferred<Attachment>();
		const upload = vi.fn().mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise);
		const m = createOrderMedia(7, fakeApi({ upload, list: async () => [] }));
		await m.reload();
		const done = m.add([file('a.png'), file('b.png')]);
		await vi.waitFor(() => expect(upload).toHaveBeenCalledTimes(1));
		expect(m.pending.map((p) => p.name)).toEqual(['a.png', 'b.png']);
		first.resolve(attachment(10, { name: 'a.png' }));
		await vi.waitFor(() => expect(upload).toHaveBeenCalledTimes(2));
		expect(m.pending.map((p) => p.name)).toEqual(['b.png']);
		expect(m.photos.map((a) => a.id)).toEqual([10]);
		second.resolve(attachment(11, { name: 'b.png' }));
		await done;
		expect(m.pending).toEqual([]);
		expect(m.photos.map((a) => a.id)).toEqual([10, 11]);
	});

	it('a too_large file gets a toast with its name; the rest still upload', async () => {
		const upload = vi
			.fn()
			.mockRejectedValueOnce(tooLarge('Файл больше 25 МБ'))
			.mockImplementation(async (_id: number, f: File) => attachment(50, { name: f.name }));
		const m = createOrderMedia(7, fakeApi({ upload, list: async () => [] }));
		await m.reload();
		await m.add([file('big.png'), file('ok.png')]);
		expect(messages()).toEqual(['big.png: Файл больше 25 МБ']);
		expect(m.photos.map((a) => a.name)).toEqual(['ok.png']);
		expect(m.pending).toEqual([]);
	});

	it('a lost connection offers «Повторить» that uploads that file again', async () => {
		const upload = vi
			.fn()
			.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи с сервером', null))
			.mockImplementation(async (_id: number, f: File) => attachment(60, { name: f.name }));
		const m = createOrderMedia(7, fakeApi({ upload, list: async () => [] }));
		await m.reload();
		await m.add([file('a.png')]);
		const [t] = toast.items;
		expect(t?.message).toBe('Не удалось загрузить a.png: нет связи с сервером');
		expect(t?.action?.label).toBe('Повторить');
		await t?.action?.run();
		expect(m.photos.map((a) => a.name)).toEqual(['a.png']);
	});

	it('remove takes the tile away at once; «Фото удалено» with «Отменить», the request goes after 6 s', async () => {
		vi.useFakeTimers();
		const api = fakeApi();
		const m = createOrderMedia(7, api);
		await m.reload();
		await m.remove(1);
		expect(m.photos).toEqual([]);
		expect(messages()).toEqual(['Фото удалено']);
		expect(toast.items[0]?.action?.label).toBe('Отменить');
		expect(api.remove).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(5900);
		expect(api.remove).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(200);
		expect(api.remove).toHaveBeenCalledOnce();
		expect(api.remove).toHaveBeenCalledWith(1);
		expect(m.photos).toEqual([]);
	});

	it('«Отменить» puts the photo back where it was, and no request is ever sent', async () => {
		vi.useFakeTimers();
		const api = fakeApi({
			list: async () => [attachment(1), attachment(3), attachment(4)]
		});
		const m = createOrderMedia(7, api);
		await m.reload();
		await m.remove(3);
		expect(m.photos.map((a) => a.id)).toEqual([1, 4]);
		await toast.items[0]?.action?.run();
		expect(m.photos.map((a) => a.id)).toEqual([1, 3, 4]);
		await vi.advanceTimersByTimeAsync(10_000);
		expect(api.remove).not.toHaveBeenCalled();
	});

	it('leaving the page sends the waiting requests at once with keepalive, and only once', async () => {
		vi.useFakeTimers();
		const api = fakeApi();
		const m = createOrderMedia(7, api);
		await m.reload();
		await m.remove(1);
		window.dispatchEvent(new Event('pagehide'));
		expect(api.remove).toHaveBeenCalledOnce();
		expect(api.remove).toHaveBeenCalledWith(1, true);
		await vi.advanceTimersByTimeAsync(10_000);
		expect(api.remove).toHaveBeenCalledOnce();
	});

	it('signing out sends the waiting deletions while the token is there, once', async () => {
		vi.useFakeTimers();
		const api = fakeApi();
		const m = createOrderMedia(7, api);
		await m.reload();
		await m.remove(1);
		expect(api.remove).not.toHaveBeenCalled();
		await expect(settleQueues(1000)).resolves.toBe(true);
		expect(api.remove).toHaveBeenCalledOnce();
		expect(api.remove).toHaveBeenCalledWith(1);
		// The session is reset after: the cleared toast does not send it again nor restore the photo.
		resetSession();
		await vi.advanceTimersByTimeAsync(10_000);
		expect(api.remove).toHaveBeenCalledOnce();
		expect(m.photos).toEqual([]);
		expect(messages()).toEqual([]);
	});

	it('a toast that is gone (pushed out by others) sends the request: nothing is left to undo', async () => {
		vi.useFakeTimers();
		const api = fakeApi();
		const m = createOrderMedia(7, api);
		await m.reload();
		await m.remove(1);
		toast.dismiss(toast.items[0]!.id);
		await vi.advanceTimersByTimeAsync(0);
		expect(api.remove).toHaveBeenCalledOnce();
	});

	it('a photo the server no longer has is deleted all the same: no error', async () => {
		vi.useFakeTimers();
		const m = createOrderMedia(
			7,
			fakeApi({ remove: vi.fn().mockRejectedValue(new ApiError(404, 'not_found', 'Нет', null)) })
		);
		await m.reload();
		await m.remove(1);
		await vi.advanceTimersByTimeAsync(7000);
		expect(m.photos).toEqual([]);
		expect(messages()).toEqual([]);
	});

	it('a failed remove puts the tile back and tells the error', async () => {
		vi.useFakeTimers();
		const m = createOrderMedia(
			7,
			fakeApi({ remove: vi.fn().mockRejectedValue(new ApiError(500, 'internal', 'Сбой', null)) })
		);
		await m.reload();
		await m.remove(1);
		await vi.advanceTimersByTimeAsync(7000);
		expect(m.photos.map((a) => a.id)).toEqual([1]);
		expect(messages()).toEqual(['Сбой']);
	});

	it('caption saves through the API and shows at once', async () => {
		const api = fakeApi();
		const m = createOrderMedia(7, api);
		await m.reload();
		const p = m.caption(1, 'царапина');
		expect(m.photos[0]?.caption).toBe('царапина');
		await p;
		expect(api.caption).toHaveBeenCalledWith(1, 'царапина');
	});
});

describe('a detached instance', () => {
	it('reports what it added once, naming the order', async () => {
		const gate = deferred<Attachment>();
		const upload = vi
			.fn()
			.mockReturnValueOnce(gate.promise)
			.mockImplementation(async (_id: number, f: File) =>
				attachment(70, { kind: 'file', name: f.name })
			);
		const m = createOrderMedia(7, fakeApi({ upload, list: async () => [] }));
		await m.reload();
		const done = m.add([file('a.png'), new File(['x'], 'b.pdf', { type: 'application/pdf' })]);
		m.detach();
		gate.resolve(attachment(69, { name: 'a.png' }));
		await done;
		expect(messages()).toEqual(['Добавлено к заказу № 7: 1 фото, 1 файл']);
	});

	it('names the order in a failure toast', async () => {
		const upload = vi.fn().mockRejectedValue(tooLarge('Файл больше 25 МБ'));
		const m = createOrderMedia(7, fakeApi({ upload, list: async () => [] }));
		await m.reload();
		const done = m.add([file('big.png')]);
		m.detach();
		await done;
		expect(messages()).toEqual(['№ 7: big.png: Файл больше 25 МБ']);
	});

	it('an attached instance does not report', async () => {
		const m = createOrderMedia(7, fakeApi({ list: async () => [] }));
		await m.reload();
		await m.add([file('a.png')]);
		expect(messages()).toEqual([]);
	});
});

describe('remove of a file', () => {
	it('says «Файл удалён»', async () => {
		const m = createOrderMedia(7, fakeApi());
		await m.reload();
		await m.remove(2);
		expect(messages()).toEqual(['Файл удалён']);
		expect(toast.items[0]?.action?.label).toBe('Отменить');
	});
});

describe('limitPhotos', () => {
	it('keeps ten and says about the rest', () => {
		const files = Array.from({ length: MAX_PHOTOS_PER_ADD + 2 }, (_, i) => file(`${i}.png`));
		expect(limitPhotos(files)).toHaveLength(10);
		expect(messages()).toEqual(['Можно добавить не больше 10 фото за раз']);
	});
	it('stays quiet within the limit', () => {
		expect(limitPhotos([file('a.png')])).toHaveLength(1);
		expect(messages()).toEqual([]);
	});
	it('keeps as many as there is room for, with the text of that place', () => {
		const files = [file('a.png'), file('b.png'), file('c.png')];
		expect(limitPhotos(files, 2, 'Не больше 10').map((f) => f.name)).toEqual(['a.png', 'b.png']);
		expect(limitPhotos(files, 0, 'Не больше 10')).toEqual([]);
		expect(messages()).toEqual(['Не больше 10', 'Не больше 10']);
	});
});

describe('createDraftMedia', () => {
	function draftApi(over: Partial<MediaApi<string>> = {}): MediaApi<string> {
		let next = 200;
		return {
			list: vi.fn(async () => [attachment(1, { order_id: null })]),
			upload: vi.fn(async (_key: string, f: File) =>
				attachment(next++, { order_id: null, name: f.name })
			),
			caption: vi.fn(async (id: number, caption: string) => attachment(id, { caption })),
			remove: vi.fn(async () => undefined),
			...over
		};
	}

	it('lists and uploads by the key of the draft, through the same queue as an order', async () => {
		const api = draftApi();
		const m = createDraftMedia('k-1', api);
		await m.reload();
		expect(api.list).toHaveBeenCalledWith('k-1');
		expect(m.photos.map((a) => a.id)).toEqual([1]);
		await m.add([file('a.png')]);
		expect(api.upload).toHaveBeenCalledWith('k-1', expect.any(File));
		expect(m.photos.map((a) => a.name)).toEqual(['p1.png', 'a.png']);
		await m.remove(1);
		expect(messages()).toEqual(['Фото удалено']);
	});

	it('idle waits for the list too: until it comes, the draft may have photos', async () => {
		const list = deferred<Attachment[]>();
		const m = createDraftMedia('k-1', draftApi({ list: vi.fn(() => list.promise) }));
		let idle = false;
		const waiting = m.idle().then(() => (idle = true));
		await Promise.resolve();
		expect(m.loading).toBe(true);
		expect(idle).toBe(false);
		list.resolve([attachment(3, { order_id: null })]);
		await waiting;
		expect(m.photos.map((a) => a.id)).toEqual([3]);
	});

	it('idle waits for the uploads already queued', async () => {
		const gate = deferred<Attachment>();
		const m = createDraftMedia('k-1', draftApi({ upload: vi.fn(() => gate.promise) }));
		await m.reload();
		void m.add([file('a.png')]);
		let idle = false;
		const waiting = m.idle().then(() => (idle = true));
		await Promise.resolve();
		expect(idle).toBe(false);
		gate.resolve(attachment(5, { order_id: null }));
		await waiting;
		expect(m.photos.map((a) => a.id)).toEqual([1, 5]);
		expect(m.pending).toEqual([]);
	});
});

describe('«Повторить» of a failed upload to a draft', () => {
	const offline = () => new ApiError(0, 'network', 'Нет связи с сервером', null);
	const guard = (over: Partial<DraftGuard> = {}): DraftGuard => ({
		max: 10,
		limitMessage: 'При приёме можно добавить не больше 10 фото',
		locked: () => null,
		...over
	});
	function draftApi(over: Partial<MediaApi<string>> = {}): MediaApi<string> {
		let next = 300;
		return {
			list: vi.fn(async () => []),
			upload: vi.fn(async (_key: string, f: File) =>
				attachment(next++, { order_id: null, name: f.name })
			),
			caption: vi.fn(),
			remove: vi.fn(async () => undefined),
			...over
		};
	}

	it('keeps the limit of ten: with the draft full again nothing is uploaded', async () => {
		const upload = vi.fn().mockRejectedValueOnce(offline());
		const ten = Array.from({ length: 10 }, (_, i) => attachment(i + 1, { order_id: null }));
		let have: Attachment[] = [];
		const api = draftApi({ upload, list: vi.fn(async () => have) });
		const m = createDraftMedia('k', api, guard());
		await m.reload();
		await m.add([file('late.png')]);
		const retry = toast.items[0]?.action;
		expect(retry?.label).toBe('Повторить');
		// Meanwhile the ten places were filled (another tab, the camera).
		have = ten;
		await m.reload();
		await retry?.run();
		expect(upload).toHaveBeenCalledTimes(1);
		expect(messages()).toContain('При приёме можно добавить не больше 10 фото');
	});

	it('counts the photos on their way too', async () => {
		const gate = deferred<Attachment>();
		const upload = vi.fn().mockRejectedValueOnce(offline()).mockReturnValue(gate.promise);
		const nine = Array.from({ length: 9 }, (_, i) => attachment(i + 1, { order_id: null }));
		const m = createDraftMedia('k', draftApi({ upload, list: async () => nine }), guard());
		await m.reload();
		await m.add([file('late.png')]);
		const retry = toast.items[0]?.action;
		void m.add([file('other.png')]);
		await retry?.run();
		// 9 there + 1 on its way = ten: the retry has no room.
		expect(upload).toHaveBeenCalledTimes(2);
		expect(upload.mock.calls[1]![1].name).toBe('other.png');
		gate.resolve(attachment(99));
	});

	it('uploads again when there is room', async () => {
		const upload = vi
			.fn()
			.mockRejectedValueOnce(offline())
			.mockImplementation(async (_k: string, f: File) => attachment(77, { name: f.name }));
		const m = createDraftMedia('k', draftApi({ upload }), guard());
		await m.reload();
		await m.add([file('late.png')]);
		await toast.items[0]?.action?.run();
		expect(m.photos.map((a) => a.name)).toEqual(['late.png']);
	});

	it('is blocked while «Принять» is on its way, and says why', async () => {
		let reason: string | null = null;
		const upload = vi.fn().mockRejectedValueOnce(offline());
		const m = createDraftMedia('k', draftApi({ upload }), guard({ locked: () => reason }));
		await m.reload();
		await m.add([file('late.png')]);
		const retry = toast.items[0]?.action;
		reason = 'Заказ принимается: фото добавить нельзя';
		await retry?.run();
		expect(upload).toHaveBeenCalledTimes(1);
		expect(messages()).toContain('Заказ принимается: фото добавить нельзя');
	});

	it('does nothing for a draft that was left (another key, cancelled)', async () => {
		const upload = vi.fn().mockRejectedValueOnce(offline());
		const m = createDraftMedia('k', draftApi({ upload }), guard());
		await m.reload();
		await m.add([file('late.png')]);
		const retry = toast.items[0]?.action;
		m.retire();
		await retry?.run();
		expect(upload).toHaveBeenCalledTimes(1);
	});
});
