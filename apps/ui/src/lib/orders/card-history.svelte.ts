// The history of the full card (spec 6.3): «История изменений» comes by pages, «Статусы и сроки» and
// «Гарантийные случаи» are picked out of the whole journal, loaded in big pages.
import type { AuditEntry } from '$api/AuditEntry';
import type { AuditPage } from '$api/AuditPage';
import type { PageQuery } from '$api/PageQuery';
import { orderHistory } from '#lib/api/orders.js';
import { mergeHistory } from './history.js';

/** «История изменений»: «Показать ещё» asks for this many more. */
export const HISTORY_PAGE = 50;
/** The journal for statuses and warranty cases comes in pages of the server's biggest size… */
export const TIMELINE_PAGE = 500;
/** …up to this many entries; a longer journal shows its newest part and says so. */
export const TIMELINE_MAX = 20 * TIMELINE_PAGE;

export interface CardHistoryApi {
	orderHistory: (id: number, page: PageQuery) => Promise<AuditPage>;
}

export function createCardHistory(
	api: CardHistoryApi = { orderHistory },
	opts: { timelineMax?: number } = {}
) {
	const timelineMax = opts.timelineMax ?? TIMELINE_MAX;
	let orderId: number | null = null;
	let entries = $state.raw<AuditEntry[]>([]);
	let total = $state(0);
	let timeline = $state.raw<AuditEntry[]>([]);
	let timelineTotal = $state(0);
	let failed = $state(false);
	let loadingMore = $state(false);
	let seq = 0;

	return {
		/** «История изменений» shown so far, newest first. */
		get entries(): AuditEntry[] {
			return entries;
		},
		/** The journal, newest first: «Статусы и сроки» and «Гарантийные случаи» come from it. */
		get timeline(): AuditEntry[] {
			return timeline;
		},
		/** The journal is longer than what was loaded for the timeline: how many entries it shows. */
		get timelineCut(): number | null {
			return timeline.length < timelineTotal ? timeline.length : null;
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

		/**
		 * The first page and the newest entries again: on opening and after every new version. The timeline
		 * pages on the first time only (later the newest page is enough), up to `TIMELINE_MAX` entries.
		 */
		async refresh(id: number): Promise<void> {
			if (id !== orderId) {
				orderId = id;
				entries = [];
				total = 0;
				timeline = [];
				timelineTotal = 0;
				failed = false;
			}
			const mine = ++seq;
			try {
				const [first, latest] = await Promise.all([
					api.orderHistory(id, { limit: HISTORY_PAGE }),
					api.orderHistory(id, { limit: TIMELINE_PAGE, offset: 0 })
				]);
				if (mine !== seq) return;
				entries = mergeHistory(entries, first.items);
				total = first.total;
				let all = mergeHistory(timeline, latest.items);
				let allTotal = latest.total;
				while (all.length < allTotal && all.length < timelineMax) {
					const page = await api.orderHistory(id, {
						limit: Math.min(TIMELINE_PAGE, timelineMax - all.length),
						offset: all.length
					});
					if (mine !== seq) return;
					if (page.items.length === 0) break;
					all = mergeHistory(all, page.items);
					allTotal = page.total;
				}
				timeline = all.slice(0, timelineMax);
				timelineTotal = allTotal;
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
