import { SvelteMap } from 'svelte/reactivity';
import type { NewOrderComment } from '$api/NewOrderComment';
import { errorMessage } from '#lib/api/client.js';
import { addOrderComment } from '#lib/api/orders.js';
import { onSessionReset, trackQueue } from '#lib/app/session-reset.js';
import { newDraftKey } from '#lib/intake/draft.js';

class CommentDraft {
	body = $state('');
	error = $state('');
	pending = $state.raw<Promise<void> | null>(null);
	attempt: NewOrderComment | null = null;
}

// Черновик принадлежит заказу и текущему сеансу. Панель и полная карточка видят одну отправку.
const drafts = new SvelteMap<number, CommentDraft>();
const latest = new SvelteMap<number, number>();
export const commentDraft = (id: number) => drafts.get(id);
export const latestCommentId = (id: number) => latest.get(id);
export const hasCommentDrafts = () => drafts.size > 0;

export function editComment(id: number, text: string) {
	const draft = drafts.get(id) ?? new CommentDraft();
	if (draft.pending) return;
	if (!text) {
		drafts.delete(id);
		return;
	}
	draft.body = text;
	draft.error = '';
	drafts.set(id, draft);
}

export function sendComment(id: number) {
	const draft = drafts.get(id);
	const body = draft?.body.trim();
	if (!draft || !body || draft.pending) return;
	draft.error = '';
	if (draft.attempt?.body !== body) draft.attempt = { body, request_key: newDraftKey() };
	draft.pending = addOrderComment(id, draft.attempt)
		.then((note) => {
			if (drafts.get(id) !== draft) return;
			drafts.delete(id);
			latest.set(id, note.id);
		})
		.catch((e: unknown) => {
			draft.error = errorMessage(e);
		})
		.finally(() => {
			draft.pending = null;
		});
}

trackQueue({
	unsaved: hasCommentDrafts,
	async flush() {
		await Promise.all(Array.from(drafts.values(), (draft) => draft.pending));
		return !hasCommentDrafts();
	},
	discard: () => drafts.clear()
});
onSessionReset(() => latest.clear());
