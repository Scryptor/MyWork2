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

export const deleteAttachment = (id: number) => request<void>('DELETE', `/attachments/${id}`);

export type AttachmentVariant = 'thumb' | 'file';

const MAX_CACHED = 200;
/** Insertion order is recency order: a hit moves the entry to the end, eviction takes the first. */
const cache = new Map<string, string>();
const pending = new Map<string, Promise<string>>();

/**
 * A `blob:` address of an attachment's picture. The files need the token, so an `<img src>` cannot point
 * at the API itself. Answers are cached (200 at most, the least recently used go first and their
 * addresses are revoked); concurrent requests for one picture share a single fetch; failures are not kept.
 */
export function attachmentUrl(id: number, variant: AttachmentVariant): Promise<string> {
	const key = `${id}:${variant}`;
	const hit = cache.get(key);
	if (hit !== undefined) {
		cache.delete(key);
		cache.set(key, hit);
		return Promise.resolve(hit);
	}
	const running = pending.get(key);
	if (running) return running;
	const promise = requestBlob(`/attachments/${id}/${variant}`)
		.then((blob) => {
			const url = URL.createObjectURL(blob);
			cache.set(key, url);
			while (cache.size > MAX_CACHED) {
				const oldest = cache.keys().next().value as string;
				URL.revokeObjectURL(cache.get(oldest)!);
				cache.delete(oldest);
			}
			return url;
		})
		.finally(() => pending.delete(key));
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
	for (const url of cache.values()) URL.revokeObjectURL(url);
	cache.clear();
	pending.clear();
}

onSessionReset(clearAttachmentCache);
