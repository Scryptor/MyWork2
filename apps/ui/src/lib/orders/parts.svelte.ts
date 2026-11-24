// The parts written off from the stock into one order (decisions 3–6): the rows, their sum, writing off and
// giving back. A write-off changes the stock, never the order: the card's own edits are not disturbed.
import { SvelteMap } from 'svelte/reactivity';
import type { OrderPart } from '$api/OrderPart';
import type { OrderParts } from '$api/OrderParts';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { addOrderPart, listOrderParts, returnOrderPart } from '#lib/api/stock.js';
import { stockCount } from '#lib/stock/summary.svelte.js';

/** The cost of an order, in kopecks: «Расходы» and the parts written off from the stock. */
export function orderCost(extra: number, parts: number): number {
	return extra + parts;
}

export interface OrderPartsState {
	readonly parts: readonly OrderPart[];
	/** Σ price × quantity of the rows, in kopecks. */
	readonly total: number;
	readonly loading: boolean;
	/** The text of the last failed read of the list; null when it is read. */
	readonly error: string | null;
	/** A write-off is on its way. */
	readonly adding: boolean;
	/**
	 * Grows with every write-off or return that went through: what shows the order's journal (a part
	 * action leaves the order's version as it is) asks it again.
	 */
	readonly revision: number;
	/** The row is being given back. */
	isReturning(partId: number): boolean;
	load(): Promise<void>;
	/**
	 * Writes `quantity` of the position off. Pressed again before the answer, it is the same request. A
	 * repeat after a lost connection carries the same key, so the server writes off once. Throws the error
	 * of the server (the dialog shows it at its field).
	 */
	add(itemId: number, quantity: number): Promise<void>;
	/** Gives the row back to the stock. A row already gone (404) counts as done: the list is read again. */
	giveBack(partId: number): Promise<void>;
	/** The order is left: an answer that comes later is ignored. */
	detach(): void;
}

export function createOrderParts(orderId: number): OrderPartsState {
	let parts = $state.raw<OrderPart[]>([]);
	let total = $state(0);
	let loading = $state(false);
	let error = $state<string | null>(null);
	let adding = $state(false);
	let revision = $state(0);
	let alive = true;
	let seq = 0;
	/** The write-off on its way, and the key of the last one that was lost to the network. */
	let sending: Promise<void> | null = null;
	let lost: { itemId: number; quantity: number; key: string } | null = null;
	/** The rows being given back, with their requests: pressed again, a row is the same request. */
	const giving = new SvelteMap<number, Promise<void>>();

	function take(answer: OrderParts) {
		parts = answer.parts;
		total = answer.total;
		error = null;
	}

	async function load() {
		const mine = ++seq;
		loading = true;
		try {
			const answer = await listOrderParts(orderId);
			if (!alive || mine !== seq) return;
			take(answer);
		} catch (e) {
			if (!alive || mine !== seq) return;
			error = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}

	function add(itemId: number, quantity: number): Promise<void> {
		if (sending) return sending;
		const key =
			lost && lost.itemId === itemId && lost.quantity === quantity ? lost.key : crypto.randomUUID();
		lost = null;
		adding = true;
		const run = (async () => {
			try {
				const answer = await addOrderPart(orderId, {
					stock_item_id: itemId,
					quantity,
					request_key: key
				});
				if (alive) {
					seq += 1;
					take(answer);
				}
				revision += 1;
				stockCount.refresh();
			} catch (e) {
				// The answer may have been lost after the write-off: the repeat asks with the same key.
				if (e instanceof ApiError && e.code === 'network') lost = { itemId, quantity, key };
				throw e;
			} finally {
				sending = null;
				adding = false;
			}
		})();
		sending = run;
		return run;
	}

	function giveBack(partId: number): Promise<void> {
		const known = giving.get(partId);
		if (known) return known;
		const run = (async () => {
			try {
				const answer = await returnOrderPart(orderId, partId);
				if (alive) {
					seq += 1;
					take(answer);
				}
				revision += 1;
				stockCount.refresh();
			} catch (e) {
				if (!(e instanceof ApiError && e.status === 404)) throw e;
				// Given back already (another window): the list shows what is left.
				await load();
				revision += 1;
				stockCount.refresh();
			} finally {
				giving.delete(partId);
			}
		})();
		giving.set(partId, run);
		return run;
	}

	return {
		get parts() {
			return parts;
		},
		get total() {
			return total;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},
		get revision() {
			return revision;
		},
		get adding() {
			return adding;
		},
		isReturning: (partId) => giving.has(partId),
		load,
		add,
		giveBack,
		detach() {
			alive = false;
		}
	};
}
