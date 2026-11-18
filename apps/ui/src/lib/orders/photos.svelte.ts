// The photos and files of one order: the list, uploads one after another with a «Загружаю…» tile each,
// captions and deletion. One instance per open order (the panel and the card each make their own).
import type { Attachment } from '$api/Attachment';
import {
	deleteAttachment,
	listOrderAttachments,
	updateAttachmentCaption,
	uploadOrderAttachment
} from '#lib/api/attachments.js';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { countLabel } from '#lib/format/plural.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

/** Pictures taken or chosen in one go. */
export const MAX_PHOTOS_PER_ADD = 10;

export interface MediaApi {
	list: (orderId: number) => Promise<Attachment[]>;
	upload: (orderId: number, file: File) => Promise<Attachment>;
	caption: (id: number, caption: string) => Promise<Attachment>;
	remove: (id: number) => Promise<void>;
}

const defaultApi: MediaApi = {
	list: listOrderAttachments,
	upload: uploadOrderAttachment,
	caption: updateAttachmentCaption,
	remove: deleteAttachment
};

export interface PendingUpload {
	id: string;
	name: string;
	/** A picture: its tile goes among the photos, any other file among the files. */
	photo: boolean;
}

/** The first ten files of an adding; the rest are skipped with a toast. */
export function limitPhotos(files: File[]): File[] {
	if (files.length <= MAX_PHOTOS_PER_ADD) return files;
	toast.show({ message: ru.media.tooManyPhotos });
	return files.slice(0, MAX_PHOTOS_PER_ADD);
}

export function createOrderMedia(orderId: number, api: MediaApi = defaultApi) {
	let attachments = $state.raw<Attachment[]>([]);
	let pending = $state.raw<PendingUpload[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	/** Uploads wait for each other here. */
	let tail: Promise<unknown> = Promise.resolve();
	let seq = 0;
	/** The order is no longer shown: uploads go on and tell about themselves by toasts. */
	let detached = false;
	let added = { photo: 0, file: 0 };
	/** Uploaded while the list was on its way: the list does not know them yet. */
	let arrived: Attachment[] = [];

	const photos = $derived(attachments.filter((a) => a.kind === 'photo'));
	const files = $derived(attachments.filter((a) => a.kind === 'file'));

	async function reload(): Promise<void> {
		loading = true;
		arrived = [];
		try {
			const list = await api.list(orderId);
			attachments = [...list, ...arrived.filter((a) => !list.some((l) => l.id === a.id))];
			error = null;
		} catch (e) {
			error = errorMessage(e);
		} finally {
			loading = false;
		}
	}

	const about = (text: string) => (detached ? ru.media.ofOrder(orderId, text) : text);

	function fail(file: File, e: unknown): void {
		if (e instanceof ApiError && e.code === 'network') {
			toast.show({
				message: about(ru.media.uploadOffline(file.name)),
				tone: 'danger',
				action: { label: ru.common.retry, run: () => add([file]) }
			});
			return;
		}
		const why = e instanceof ApiError ? (e.body?.fields[0]?.message ?? e.message) : '';
		toast.show({
			message: about(ru.media.uploadFailed(file.name, why || ru.api.failed)),
			tone: 'danger'
		});
	}

	async function send(file: File, tile: PendingUpload): Promise<void> {
		try {
			const saved = await api.upload(orderId, file);
			if (loading) arrived.push(saved);
			attachments = attachments.some((a) => a.id === saved.id)
				? attachments
				: [...attachments, saved];
			added[saved.kind] += 1;
		} catch (e) {
			fail(file, e);
		} finally {
			pending = pending.filter((p) => p.id !== tile.id);
			if (detached && pending.length === 0) report();
		}
	}

	/** Once the queue of a detached instance is empty: what was added, in one toast. */
	function report(): void {
		const parts = [
			added.photo > 0 ? countLabel(added.photo, ru.media.photoForms) : '',
			added.file > 0 ? countLabel(added.file, ru.media.fileForms) : ''
		].filter((x) => x !== '');
		added = { photo: 0, file: 0 };
		if (parts.length > 0) toast.show({ message: ru.media.addedToOrder(orderId, parts.join(', ')) });
	}

	/** The order is left (another one opened, the panel or card closed). */
	function detach(): void {
		detached = true;
		if (pending.length === 0) added = { photo: 0, file: 0 };
	}

	/** Uploads the files one by one; resolves when all of them are done or have failed. */
	function add(list: File[]): Promise<void> {
		if (list.length === 0) return Promise.resolve();
		const batch = list.map((file) => ({
			file,
			tile: { id: `up-${++seq}`, name: file.name, photo: file.type.startsWith('image/') }
		}));
		pending = [...pending, ...batch.map((b) => b.tile)];
		const run = tail.then(async () => {
			for (const { file, tile } of batch) await send(file, tile);
		});
		tail = run;
		return run;
	}

	async function caption(id: number, text: string): Promise<void> {
		const before = attachments.find((a) => a.id === id);
		if (!before) return;
		attachments = attachments.map((a) => (a.id === id ? { ...a, caption: text } : a));
		try {
			const saved = await api.caption(id, text);
			attachments = attachments.map((a) => (a.id === id ? saved : a));
		} catch (e) {
			attachments = attachments.map((a) => (a.id === id ? before : a));
			toast.show({ message: errorMessage(e), tone: 'danger' });
		}
	}

	async function remove(id: number): Promise<void> {
		const index = attachments.findIndex((a) => a.id === id);
		const before = attachments[index];
		if (!before) return;
		attachments = attachments.filter((a) => a.id !== id);
		try {
			await api.remove(id);
			toast.show({
				message: about(before.kind === 'file' ? ru.media.fileDeleted : ru.media.photoDeleted)
			});
		} catch (e) {
			const at = Math.min(index, attachments.length);
			attachments = [...attachments.slice(0, at), before, ...attachments.slice(at)];
			toast.show({ message: about(errorMessage(e)), tone: 'danger' });
		}
	}

	void reload();

	return {
		get photos() {
			return photos;
		},
		get files() {
			return files;
		},
		get pending() {
			return pending;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},
		add,
		detach,
		caption,
		remove,
		reload
	};
}

export type OrderMedia = ReturnType<typeof createOrderMedia>;
