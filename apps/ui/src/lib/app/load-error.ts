import { error } from '@sveltejs/kit';
import { ApiError } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

/**
 * A request of a load failed: the error page (`routes/+error.svelte`) shows what happened. No answer
 * at all is 503 marked `offline`; another answer of the server keeps its status and message.
 */
export function failLoad(e: unknown): never {
	if (e instanceof ApiError) {
		if (e.code === 'network') error(503, ru.api.offline, { offline: true });
		error(e.status >= 400 && e.status <= 599 ? e.status : 500, e.message);
	}
	throw e;
}
