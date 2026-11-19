import type { Attachment } from '$api/Attachment';
import { onSessionReset } from '#lib/app/session-reset.js';
import { request, requestBlob, upload } from './client.js';

export const listOrderAttachments = (orderId: number) =>
	request<Attachment[]>('GET', `/orders/${orderId}/attachments`);

export const uploadOrderAttachment = (orderId: number, file: File) =>
	upload<Attachment>(`/orders/${orderId}/attachments`, file, { name: file.name });

export const listDraftAttachments = (key: string) =>
	request<Attachment[]>('GET', `/intake-drafts/${encodeURIComponent(key)}/attachments`);

export const uploadDraftAttachment = (key: string, file: File) =>
	upload<Attachment>(`/intake-drafts/${encodeURIComponent(key)}/attachments`, file, {
		name: file.name
	});

export const discardDraft = (key: string) =>
	request<void>('DELETE', `/intake-drafts/${encodeURIComponent(key)}`);

export const updateAttachmentCaption = (id: number, caption: string) =>
	request<Attachment>('PATCH', `/attachments/${id}`, { body: { caption } });

/** `keepalive`: the page is being left, the request still goes. */
export const deleteAttachment = (id: number, keepalive = false) =>
	request<void>('DELETE', `/attachments/${id}`, { keepalive });

export type AttachmentVariant = 'thumb' | 'file';

/** Thumbnails are small: counted. */
const MAX_THUMBS = 200;
/** Originals vary from kilobytes to many megabytes: counted by volume. */
export const MAX_ORIGINAL_BYTES = 100 * 1024 * 1024;

interface Entry {
	url: string;
	size: number;
}
/** One cache per kind. Insertion order is recency order: a hit moves the entry to the end, eviction takes the first. */
const caches: Record<AttachmentVariant, Map<string, Entry>> = {
	thumb: new Map(),
	file: new Map()
};
const pending = new Map<string, Promise<string>>();
/** Counts the clears: a picture fetched for an earlier user must not enter the cache of the next one. */
let generation = 0;

function evict(variant: AttachmentVariant): void {
	const cache = caches[variant];
	const over = () => {
		if (variant === 'thumb') return cache.size > MAX_THUMBS;
		let total = 0;
		for (const e of cache.values()) total += e.size;
		// An original over the whole volume stays alone: the screen showing it still needs the address.
		return total > MAX_ORIGINAL_BYTES && cache.size > 1;
	};
	while (over()) {
		const oldest = cache.keys().next().value as string;
		URL.revokeObjectURL(cache.get(oldest)!.url);
		cache.delete(oldest);
	}
}

/**
 * A `blob:` address of an attachment's picture. The files need the token, so an `<img src>` cannot point
 * at the API itself. Answers are cached: 200 thumbnails and 100 MB of originals, apart from each other;
 * the least recently used go first and their addresses are revoked. Concurrent requests for one picture
 * share a single fetch; failures are not kept.
 */
export function attachmentUrl(id: number, variant: AttachmentVariant): Promise<string> {
	const key = `${id}:${variant}`;
	const cache = caches[variant];
	const hit = cache.get(key);
	if (hit !== undefined) {
		cache.delete(key);
		cache.set(key, hit);
		return Promise.resolve(hit.url);
	}
	const running = pending.get(key);
	if (running) return running;
	const mine = generation;
	const promise = requestBlob(`/attachments/${id}/${variant}`)
		.then((blob) => {
			const url = URL.createObjectURL(blob);
			// Cleared meanwhile (signed out): the caller of that time gets it, nobody after.
			if (mine !== generation) {
				setTimeout(() => URL.revokeObjectURL(url), 0);
				return url;
			}
			cache.set(key, { url, size: blob.size });
			evict(variant);
			return url;
		})
		.finally(() => {
			if (pending.get(key) === promise) pending.delete(key);
		});
	pending.set(key, promise);
	return promise;
}

/** Saves the original file through a temporary `<a download>`. */
export async function downloadAttachment(a: Attachment): Promise<void> {
	const blob = await requestBlob(`/attachments/${a.id}/file`);
	const url = URL.createObjectURL(blob);
	const link = document.createElement('a');
	link.href = url;
	link.download = a.name;
	document.body.append(link);
	link.click();
	link.remove();
	setTimeout(() => URL.revokeObjectURL(url), 0);
}

/** Drops every cached picture of the user who signed out (and in tests). */
export function clearAttachmentCache(): void {
	for (const cache of Object.values(caches)) {
		for (const e of cache.values()) URL.revokeObjectURL(e.url);
		cache.clear();
	}
	pending.clear();
	generation += 1;
}

onSessionReset(clearAttachmentCache);
