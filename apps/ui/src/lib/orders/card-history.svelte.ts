// The history of the full card (spec 6.3): «История изменений» comes by pages, «Статусы и сроки» and
// «Гарантийные случаи» are picked out of the newest entries.
import type { AuditEntry } from '$api/AuditEntry';
import type { AuditPage } from '$api/AuditPage';
import type { PageQuery } from '$api/PageQuery';
import { orderHistory } from '#lib/api/orders.js';
import { mergeHistory } from './history.js';

/** «История изменений»: «Показать ещё» asks for this many more. */
export const HISTORY_PAGE = 50;
/** Statuses and warranty cases are looked for among this many newest entries (the server's limit). */
export const TIMELINE_LIMIT = 500;

export interface CardHistoryApi {
	orderHistory: (id: number, page: PageQuery) => Promise<AuditPage>;
}

export function createCardHistory(api: CardHistoryApi = { orderHistory }) {
	let orderId: number | null = null;
	let entries = $state.raw<AuditEntry[]>([]);
	let total = $state(0);
	let newest = $state.raw<AuditEntry[]>([]);
	let failed = $state(false);
	let loadingMore = $state(false);
	let seq = 0;

	return {
		/** «История изменений» shown so far, newest first. */
		get entries(): AuditEntry[] {
			return entries;
		},
		/** The newest entries: «Статусы и сроки» and «Гарантийные случаи» come from them. */
		get newest(): AuditEntry[] {
			return newest;
		},
		get hasMore(): boolean {
			return entries.length < total;
		},
		get loadingMore(): boolean {
			return loadingMore;
		},
		/** Nothing came: the history could not be loaded. */
		get failed(): boolean {
			return failed;
		},

		/** The first page and the newest entries again: on opening and after every new version. */
		async refresh(id: number): Promise<void> {
			if (id !== orderId) {
				orderId = id;
				entries = [];
				total = 0;
				newest = [];
				failed = false;
			}
			const mine = ++seq;
			try {
				const [first, latest] = await Promise.all([
					api.orderHistory(id, { limit: HISTORY_PAGE }),
					api.orderHistory(id, { limit: TIMELINE_LIMIT })
				]);
				if (mine !== seq) return;
				entries = mergeHistory(entries, first.items);
				total = first.total;
				newest = latest.items;
				failed = false;
			} catch {
				// What was shown stays; with nothing shown the sections say it failed.
				if (mine === seq) failed = entries.length === 0;
			}
		},

		/** «Показать ещё»: the next page; throws when it could not be loaded. */
		async more(): Promise<void> {
			const id = orderId;
			if (id === null || loadingMore || entries.length >= total) return;
			loadingMore = true;
			try {
				const page = await api.orderHistory(id, { limit: HISTORY_PAGE, offset: entries.length });
				if (id !== orderId) return;
				entries = mergeHistory(entries, page.items);
				total = page.total;
			} finally {
				loadingMore = false;
			}
		}
	};
}

export type CardHistory = ReturnType<typeof createCardHistory>;
