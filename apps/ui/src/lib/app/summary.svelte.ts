import type { OrderSummary } from '$api/OrderSummary';
import type { Status } from '$api/Status';
import { onSessionReset } from '#lib/app/session-reset.js';
import { orderSummary } from '#lib/api/orders.js';
import { workStatusIds, type AttentionKey } from '#lib/orders/view.js';

export { ATTENTION_KEYS, type AttentionKey } from '#lib/orders/view.js';

/** The summary is asked for at most once in this many milliseconds. */
export const SUMMARY_GAP_MS = 1000;

/** Counts of «Требуют внимания» by item. */
export function attentionCounts(s: OrderSummary | null): Record<AttentionKey, number> | null {
	if (!s) return null;
	return {
		call: s.need_call,
		parts: s.need_parts,
		longDiagnostics: s.overdue_diagnostics,
		longReady: s.stale_ready
	};
}

/** «В работе»: orders in statuses of kinds `open` and `diagnostics`. */
export function inWorkCount(s: OrderSummary | null, statuses: Status[]): number | undefined {
	if (!s) return undefined;
	const work = workStatusIds(statuses);
	return s.by_status.reduce((n, c) => (work.includes(c.status_id) ? n + c.count : n), 0);
}

/**
 * The order summary of the chosen point: counts of the sidebar and of the quick filters. `refresh`
 * asks at once, or once the gap since the last request has passed; the newest point wins.
 */
export function createSummary(load: (point: number | null) => Promise<OrderSummary>) {
	let current = $state.raw<OrderSummary | null>(null);
	let last = -Infinity;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let wanted: number | null = null;
	let seq = 0;

	function ask() {
		timer = undefined;
		last = Date.now();
		const mine = ++seq;
		load(wanted).then(
			(s) => {
				if (mine === seq) current = s;
			},
			// The counts keep their last value; the screens report lost connection themselves.
			() => {}
		);
	}

	return {
		get current(): OrderSummary | null {
			return current;
		},
		/** Forgets the counts of the user who signed out; an answer still on its way is ignored. */
		reset(): void {
			clearTimeout(timer);
			timer = undefined;
			seq += 1;
			last = -Infinity;
			current = null;
		},
		refresh(point: number | null): void {
			wanted = point;
			if (timer !== undefined) return;
			const wait = last + SUMMARY_GAP_MS - Date.now();
			if (wait <= 0) ask();
			else timer = setTimeout(ask, wait);
		}
	};
}

export const summary = createSummary(orderSummary);

onSessionReset(() => summary.reset());
