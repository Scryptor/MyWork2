// «История» of a position: the newest page of its journal and «Показать ещё» for the older ones. The same
// cursor and merging as the history of the order card (`orders/history.ts`).
import type { AuditEntry } from '$api/AuditEntry';
import type { AuditPage } from '$api/AuditPage';
import type { HistoryQuery } from '$api/HistoryQuery';
import { stockItemHistory } from '#lib/api/stock.js';
import { historyCursor, mergeHistory, type HistoryCursor } from '#lib/orders/history.js';

/** Entries asked at a time. */
export const STOCK_HISTORY_PAGE = 20;

export function createStockHistory(
	api: (id: number, page?: HistoryQuery) => Promise<AuditPage> = stockItemHistory,
	pageSize = STOCK_HISTORY_PAGE
) {
	let itemId: number | null = null;
	let entries = $state.raw<AuditEntry[]>([]);
	let done = $state(true);
	let failed = $state(false);
	let moreFailed = $state(false);
	let loadingMore = $state(false);
	let cursor: HistoryCursor | undefined;
	let seq = 0;

	return {
		/** The entries shown so far, newest first. */
		get entries(): AuditEntry[] {
			return entries;
		},
		get hasMore(): boolean {
			return !done;
		},
		get loadingMore(): boolean {
			return loadingMore;
		},
		/** Nothing came: the history could not be loaded. */
		get failed(): boolean {
			return failed;
		},
		/** «Показать ещё» failed: the page shown stays, the message offers «Повторить». */
		get moreFailed(): boolean {
			return moreFailed;
		},

		/**
		 * On opening and after every new version: the newest page. Later it only adds the entries that are
		 * new on top (a whole page of new ones starts over).
		 */
		async refresh(id: number): Promise<void> {
			const mine = ++seq;
			try {
				const page = await api(id, { limit: pageSize });
				if (mine !== seq) return;
				const items = page.items;
				const newest = entries[0]?.id;
				const same = id === itemId;
				// A whole page of new entries: what is shown no longer joins the newest ones.
				const gap =
					newest !== undefined && items.length === pageSize && (items.at(-1)?.id ?? 0) > newest;
				if (!same || entries.length === 0 || gap) {
					itemId = id;
					entries = items;
					cursor = historyCursor(items);
					done = items.length < pageSize;
				} else {
					entries = mergeHistory(
						entries,
						items.filter((e) => e.id > newest!)
					);
				}
				failed = false;
			} catch {
				// What was shown stays; with nothing shown the section says it failed.
				if (mine === seq && (id !== itemId || entries.length === 0)) failed = true;
			}
		},

		/** «Показать ещё» and «Повторить»: the next page after the last page read. */
		async more(): Promise<void> {
			const id = itemId;
			if (id === null || loadingMore || done) return;
			loadingMore = true;
			moreFailed = false;
			try {
				const page = await api(id, { limit: pageSize, ...cursor });
				if (id !== itemId) return;
				cursor = historyCursor(page.items) ?? cursor;
				entries = mergeHistory(entries, page.items);
				if (page.items.length < pageSize) done = true;
			} catch {
				if (id === itemId) moreFailed = true;
			} finally {
				loadingMore = false;
			}
		}
	};
}

export type StockHistory = ReturnType<typeof createStockHistory>;
