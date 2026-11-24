import { stockSummary } from '#lib/api/stock.js';
import { onSessionReset } from '#lib/app/session-reset.js';

let restock = $state<number | null>(null);
let seq = 0;

/**
 * How many positions are to be bought («Докупить»), for the counter of the sidebar. Asked at the start
 * and after every change of the stock made in this tab; the newest answer wins.
 */
export const stockCount = {
	get restock(): number | null {
		return restock;
	},
	refresh(): void {
		const mine = ++seq;
		stockSummary().then(
			(s) => {
				if (mine === seq) restock = s.restock;
			},
			// The counter keeps its last value; the screens report lost connection themselves.
			() => {}
		);
	},
	/** Forgets the count of the user who signed out; an answer still on its way is ignored. */
	reset(): void {
		seq += 1;
		restock = null;
	}
};

onSessionReset(() => stockCount.reset());
