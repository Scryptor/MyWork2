import type { Attachment } from '$api/Attachment';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	createOrderMedia,
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

	it('remove takes the tile away at once and the toast says «Фото удалено»', async () => {
		const gate = deferred<undefined>();
		const m = createOrderMedia(7, fakeApi({ remove: vi.fn(() => gate.promise) }));
		await m.reload();
		const p = m.remove(1);
		expect(m.photos).toEqual([]);
		gate.resolve(undefined);
		await p;
		expect(m.photos).toEqual([]);
		expect(messages()).toEqual(['Фото удалено']);
	});

	it('a failed remove puts the tile back and tells the error', async () => {
		const m = createOrderMedia(
			7,
			fakeApi({ remove: vi.fn().mockRejectedValue(new ApiError(500, 'internal', 'Сбой', null)) })
		);
		await m.reload();
		await m.remove(1);
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
});
