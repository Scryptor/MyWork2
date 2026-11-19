// The photos and files of one order or one intake draft: the list, uploads one after another with a
// «Загружаю…» tile each, captions and deletion. One instance per open order (the panel and the card each
// make their own) or per draft of the intake sheet.
import type { Attachment } from '$api/Attachment';
import {
	deleteAttachment,
	listDraftAttachments,
	listOrderAttachments,
	updateAttachmentCaption,
	uploadDraftAttachment,
	uploadOrderAttachment
} from '#lib/api/attachments.js';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { undo } from '#lib/app/undo.svelte.js';
import { countLabel } from '#lib/format/plural.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

/** Pictures taken or chosen in one go. */
export const MAX_PHOTOS_PER_ADD = 10;

/** The attachments of an owner: an order by its number, a draft by its key. */
export interface MediaApi<K = number> {
	list: (owner: K) => Promise<Attachment[]>;
	upload: (owner: K, file: File) => Promise<Attachment>;
	caption: (id: number, caption: string) => Promise<Attachment>;
	/** `keepalive`: sent as the page is left. */
	remove: (id: number, keepalive?: boolean) => Promise<void>;
}

const orderApi: MediaApi<number> = {
	list: listOrderAttachments,
	upload: uploadOrderAttachment,
	caption: updateAttachmentCaption,
	remove: deleteAttachment
};

const draftApi: MediaApi<string> = {
	list: listDraftAttachments,
	upload: uploadDraftAttachment,
	caption: updateAttachmentCaption,
	remove: deleteAttachment
};

export interface PendingUpload {
	id: string;
	name: string;
	/** A picture: its tile goes among the photos, any other file among the files. */
	photo: boolean;
}

/**
 * The first files of an adding that there is room for (ten by default); the rest are skipped with a
 * toast, `message` by default the limit of one adding.
 */
export function limitPhotos(
	files: File[],
	room = MAX_PHOTOS_PER_ADD,
	message: string = ru.media.tooManyPhotos
): File[] {
	const fit = Math.max(0, room);
	if (files.length <= fit) return files;
	toast.show({ message });
	return files.slice(0, fit);
}

/**
 * What may be added to a draft: the room (`max` photos in all) and the moments when nothing may be
 * (`locked` says why). «Повторить» of a failed upload asks it again, as the file was only picked then.
 */
export interface DraftGuard {
	max: number;
	/** Shown when there is no room. */
	limitMessage: string;
	/** Why nothing may be added now; null when it may. */
	locked: () => string | null;
}

/** Deletions waiting for their toast to go: the page being left sends them at once. */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, nothing renders it
const waiting = new Set<(keepalive: boolean) => Promise<void>>();
if (typeof window !== 'undefined')
	window.addEventListener('pagehide', () => {
		for (const commit of [...waiting]) void commit(true);
	});

/** What an instance that was left says about itself: whose uploads these are. */
interface Detached {
	/** A message about the owner the user has left. */
	about: (text: string) => string;
	/** What was added, once the queue of a left instance is empty. */
	added: (what: string) => string;
}

function createMedia<K>(owner: K, api: MediaApi<K>, left?: Detached, guard?: DraftGuard) {
	let attachments = $state.raw<Attachment[]>([]);
	let pending = $state.raw<PendingUpload[]>([]);
	let loading = $state(false);
	let error = $state<string | null>(null);
	/** Uploads wait for each other here. */
	let tail: Promise<unknown> = Promise.resolve();
	/** The list on its way: until it comes, the owner may have more than is shown. */
	let listing: Promise<void> = Promise.resolve();
	let seq = 0;
	/** The owner is no longer shown: uploads go on and tell about themselves by toasts. */
	let detached = false;
	/** The owner is gone for good (another draft, cancelled): nothing goes to it any more. */
	let retired = false;
	let added = { photo: 0, file: 0 };
	/** Uploaded while the list was on its way: the list does not know them yet. */
	let arrived: Attachment[] = [];

	const photos = $derived(attachments.filter((a) => a.kind === 'photo'));
	const files = $derived(attachments.filter((a) => a.kind === 'file'));

	let listSeq = 0;
	function reload(): Promise<void> {
		const mine = ++listSeq;
		loading = true;
		arrived = [];
		const run = (async () => {
			try {
				const list = await api.list(owner);
				attachments = [...list, ...arrived.filter((a) => !list.some((l) => l.id === a.id))];
				error = null;
			} catch (e) {
				error = errorMessage(e);
			} finally {
				if (mine === listSeq) loading = false;
			}
		})();
		listing = run;
		return run;
	}

	const about = (text: string) => (detached && left ? left.about(text) : text);

	function fail(file: File, e: unknown): void {
		if (e instanceof ApiError && e.code === 'network') {
			toast.show({
				message: about(ru.media.uploadOffline(file.name)),
				tone: 'danger',
				action: { label: ru.common.retry, run: () => retry(file) }
			});
			return;
		}
		const why = e instanceof ApiError ? (e.body?.fields[0]?.message ?? e.message) : '';
		toast.show({
			message: about(ru.media.uploadFailed(file.name, why || ru.api.failed)),
			tone: 'danger'
		});
	}

	/** «Повторить»: the file goes again if the owner still takes it, as when it was first picked. */
	function retry(file: File): Promise<void> {
		if (retired) return Promise.resolve();
		const why = guard?.locked();
		if (why) {
			toast.show({ message: why });
			return Promise.resolve();
		}
		if (guard) {
			const here = attachments.filter((a) => a.kind === 'photo').length;
			const coming = pending.filter((p) => p.photo).length;
			if (file.type.startsWith('image/') && here + coming >= guard.max) {
				toast.show({ message: guard.limitMessage });
				return Promise.resolve();
			}
		}
		return add([file]);
	}

	async function send(file: File, tile: PendingUpload): Promise<void> {
		try {
			const saved = await api.upload(owner, file);
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
		if (parts.length > 0 && left) toast.show({ message: left.added(parts.join(', ')) });
	}

	/** The order is left (another one opened, the panel or card closed). */
	function detach(): void {
		detached = true;
		if (pending.length === 0) added = { photo: 0, file: 0 };
	}

	/** The owner is left for good: late retries do nothing. */
	function retire(): void {
		retired = true;
	}

	/** Resolves once the list and the uploads queued so far are done or have failed. */
	function idle(): Promise<void> {
		return Promise.all([listing, tail]).then(() => {});
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

	/**
	 * The tile goes at once; «Фото удалено · Отменить» for 6 s. The request goes when the toast is gone
	 * (or the page is left, with `keepalive`); «Отменить» puts the tile back without any request.
	 */
	function remove(id: number): Promise<void> {
		const index = attachments.findIndex((a) => a.id === id);
		const before = attachments[index];
		if (!before) return Promise.resolve();
		attachments = attachments.filter((a) => a.id !== id);
		let settled = false;
		const restore = () => {
			if (attachments.some((a) => a.id === id)) return;
			const at = Math.min(index, attachments.length);
			attachments = [...attachments.slice(0, at), before, ...attachments.slice(at)];
		};
		const commit = async (keepalive: boolean): Promise<void> => {
			if (settled) return;
			settled = true;
			waiting.delete(commit);
			try {
				await (keepalive ? api.remove(id, true) : api.remove(id));
			} catch (e) {
				// Already gone is what was wanted.
				if (e instanceof ApiError && e.status === 404) return;
				restore();
				toast.show({ message: about(errorMessage(e)), tone: 'danger' });
			}
		};
		waiting.add(commit);
		undo.push(
			about(before.kind === 'file' ? ru.media.fileDeleted : ru.media.photoDeleted),
			async () => {
				if (settled) return;
				settled = true;
				waiting.delete(commit);
				restore();
			},
			// The button and Ctrl/⌘+Z dismiss the toast first and undo right after: the undo wins.
			() => queueMicrotask(() => void commit(false))
		);
		return Promise.resolve();
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
		idle,
		detach,
		retire,
		caption,
		remove,
		reload
	};
}

export type Media = ReturnType<typeof createMedia>;
export type OrderMedia = Media;

export function createOrderMedia(orderId: number, api: MediaApi<number> = orderApi): Media {
	return createMedia(orderId, api, {
		about: (text) => ru.media.ofOrder(orderId, text),
		added: (what) => ru.media.addedToOrder(orderId, what)
	});
}

/**
 * The photos of an intake draft, kept on the server under its key until the order is accepted. The sheet
 * keeps the instance while closed: uploads go on and show again at the next opening.
 */
export function createDraftMedia(
	key: string,
	api: MediaApi<string> = draftApi,
	guard?: DraftGuard
): Media {
	return createMedia(key, api, undefined, guard);
}
