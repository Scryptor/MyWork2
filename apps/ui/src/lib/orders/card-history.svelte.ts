// The history of the full card (spec 6.3): «История изменений» comes by pages, «Статусы и сроки» and
// «Гарантийные случаи» are picked out of the whole journal, loaded in big pages.
import type { AuditEntry } from '$api/AuditEntry';
import type { AuditPage } from '$api/AuditPage';
import type { HistoryQuery } from '$api/HistoryQuery';
import { orderHistory } from '#lib/api/orders.js';
import { historyCursor, mergeHistory, type HistoryCursor } from './history.js';

/** «История изменений»: «Показать ещё» asks for this many more. */
export const HISTORY_PAGE = 50;
/** The journal for statuses and warranty cases comes in pages of the server's biggest size… */
export const TIMELINE_PAGE = 500;
/** …up to this many entries; a longer journal shows its newest part and says so. */
export const TIMELINE_MAX = 20 * TIMELINE_PAGE;

export interface CardHistoryApi {
	orderHistory: (id: number, page: HistoryQuery) => Promise<AuditPage>;
}

export function createCardHistory(
	api: CardHistoryApi = { orderHistory },
	opts: { timelineMax?: number } = {}
) {
	const timelineMax = opts.timelineMax ?? TIMELINE_MAX;
	let orderId: number | null = null;
	let entries = $state.raw<AuditEntry[]>([]);
	let timeline = $state.raw<AuditEntry[]>([]);
	let timelineTotal = $state(0);
	/** «История изменений» has reached the oldest entry; the timeline likewise. */
	let entriesDone = $state(false);
	let timelineDone = false;
	let failed = $state(false);
	let moreFailed = $state(false);
	let timelineFailed = $state(false);
	let loadingMore = $state(false);
	/** Where «Показать ещё» and the timeline go on: after the last page each of them read. */
	let entriesCursor: HistoryCursor | undefined;
	let timelineCursor: HistoryCursor | undefined;
	let seq = 0;

	function reset() {
		entries = [];
		timeline = [];
		entriesCursor = undefined;
		timelineCursor = undefined;
		timelineTotal = 0;
		entriesDone = false;
		timelineDone = false;
		failed = false;
		moreFailed = false;
		timelineFailed = false;
	}

	/** The timeline goes on to older entries after the last page it read, until the journal or the limit ends. */
	async function extend(id: number, mine: number): Promise<void> {
		timelineFailed = false;
		try {
			while (!timelineDone && timeline.length < timelineMax) {
				const limit = Math.min(TIMELINE_PAGE, timelineMax - timeline.length);
				const page = await api.orderHistory(id, { limit, ...timelineCursor });
				if (mine !== seq) return;
				timelineCursor = historyCursor(page.items) ?? timelineCursor;
				timeline = mergeHistory(timeline, page.items);
				timelineTotal = page.total;
				if (page.items.length < limit) timelineDone = true;
			}
		} catch {
			// What came stays; «Повторить» asks again and goes on from the oldest entry shown.
			if (mine === seq) timelineFailed = true;
		}
	}

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
			return !timelineFailed && timeline.length < timelineTotal ? timeline.length : null;
		},
		get hasMore(): boolean {
			return !entriesDone;
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
		/** An older part of the journal for the timeline could not be loaded. */
		get timelineFailed(): boolean {
			return timelineFailed;
		},

		/**
		 * On opening and after every new version: one request for the newest page of the journal. It
		 * serves both lists; later it only adds the entries that are new (a gap of a whole page starts over).
		 * The timeline then pages on by id up to `TIMELINE_MAX` entries.
		 */
		async refresh(id: number): Promise<void> {
			if (id !== orderId) {
				orderId = id;
				reset();
			}
			const mine = ++seq;
			try {
				const latest = await api.orderHistory(id, { limit: TIMELINE_PAGE });
				if (mine !== seq) return;
				const items = latest.items;
				const newest = timeline[0]?.id;
				// A whole page of new entries: what is shown no longer joins the newest ones.
				if (
					newest !== undefined &&
					items.length === TIMELINE_PAGE &&
					(items.at(-1)?.id ?? 0) > newest
				) {
					reset();
				}
				if (entries.length === 0) {
					entries = items.slice(0, HISTORY_PAGE);
					entriesCursor = historyCursor(entries);
					entriesDone = items.length <= HISTORY_PAGE;
				} else {
					const top = entries[0]?.id ?? 0;
					entries = mergeHistory(
						entries,
						items.filter((e) => e.id > top)
					);
				}
				if (timeline.length === 0) {
					timelineDone = items.length < TIMELINE_PAGE;
					timelineCursor = historyCursor(items);
				}
				timeline = mergeHistory(timeline, items);
				timelineTotal = latest.total;
				failed = false;
			} catch {
				// What was shown stays; with nothing shown the sections say it failed.
				if (mine === seq) failed = entries.length === 0;
				return;
			}
			await extend(id, mine);
		},

		/** «Показать ещё» and «Повторить»: the next page after the oldest entry shown. */
		async more(): Promise<void> {
			const id = orderId;
			if (id === null || loadingMore || entriesDone) return;
			loadingMore = true;
			moreFailed = false;
			try {
				const page = await api.orderHistory(id, { limit: HISTORY_PAGE, ...entriesCursor });
				if (id !== orderId) return;
				entriesCursor = historyCursor(page.items) ?? entriesCursor;
				entries = mergeHistory(entries, page.items);
				if (page.items.length < HISTORY_PAGE) entriesDone = true;
			} catch {
				if (id === orderId) moreFailed = true;
			} finally {
				loadingMore = false;
			}
		}
	};
}

export type CardHistory = ReturnType<typeof createCardHistory>;
