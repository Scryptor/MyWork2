// Editing one order on the server (spec 6.2): autosave with the order's version, the conflict when it
// was changed elsewhere, server actions (status, issue) on the newest version. The panel and the full
// card (task 9) each make one editor per open order.
/* eslint-disable svelte/prefer-svelte-reactivity -- maps and sets here are replaced whole in $state.raw */
import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { ApiError } from '#lib/api/client.js';
import { changeStatus, getOrder, updateOrder } from '#lib/api/orders.js';
import { createAutosave, type Autosave, type SaveState } from './autosave.js';
import { emitOrderChanged } from './events.js';
import { mergeRemote, sameValue } from './merge.js';

/** Text a moment after the last key (spec 6.2). */
export const SAVE_DELAY_MS = 600;

/** The fields edited in the panel and the card. */
export const ORDER_FIELDS = [
	'device_type',
	'brand',
	'model',
	'color',
	'serial',
	'device_password',
	'completeness',
	'device_condition',
	'malfunction',
	'work_done',
	'master',
	'final_price',
	'discount',
	'extra_costs',
	'warranty',
	'need_call',
	'need_parts',
	'internal_comment'
] as const satisfies readonly (keyof Order & keyof OrderPatch)[];

export type OrderField = (typeof ORDER_FIELDS)[number];
export type OrderValues = Pick<Order, OrderField>;

/** A field changed here and elsewhere: own value and the one on the server now. */
export interface Conflict {
	mine: OrderValues[OrderField];
	theirs: OrderValues[OrderField];
}

/** `conflict`: own changes wait for «Оставить моё» or «Взять новое». */
export type EditorSaveState = SaveState | 'conflict';

export interface EditorApi {
	getOrder: (id: number) => Promise<Order>;
	updateOrder: (id: number, version: number, patch: OrderPatch) => Promise<Order>;
	changeStatus: (id: number, version: number, change: StatusChange) => Promise<Order>;
}

export interface EditorOptions {
	api?: EditorApi;
	/** Every answer of the server goes here: lists and the summary catch up. */
	emit?: (o: Order) => void;
	delayMs?: number;
}

const DEFAULT_API: EditorApi = { getOrder: (id) => getOrder(id), updateOrder, changeStatus };

export interface OrderEditor {
	/** The order as the server last gave it. */
	readonly order: Order;
	/** What fields show and change: own unsaved values first, the disputed ones as own until decided. */
	readonly autosave: Autosave<OrderValues>;
	readonly conflicts: ReadonlyMap<OrderField, Conflict>;
	/** Fields just taken from another computer: highlighted for a moment. */
	readonly flashed: ReadonlySet<OrderField>;
	readonly saveState: EditorSaveState;
	/** The order as the fields show it now. */
	readonly current: Order;
	/** «Оставить моё»: own value goes over the newer version. */
	keep(field: OrderField): void;
	/** «Взять новое»: the field shows what came from the server. */
	take(field: OrderField): void;
	endFlash(field: OrderField): void;
	/** A newer version of this order from elsewhere in the program (an undo, a bulk action). */
	receive(o: Order): void;
	/** Saves everything; false when a save failed or a conflict waits for a choice. */
	flush(): Promise<boolean>;
	/** A server action on the newest version: what is typed goes first; one retry after a conflict. */
	run(action: (version: number) => Promise<Order>): Promise<Order>;
	dispose(): void;
}

function valuesOf(o: Order): OrderValues {
	return Object.fromEntries(ORDER_FIELDS.map((f) => [f, o[f]])) as OrderValues;
}

const isConflict = (e: unknown) => e instanceof ApiError && e.code === 'conflict';

export function createOrderEditor(initial: Order, opts: EditorOptions = {}): OrderEditor {
	const api = opts.api ?? DEFAULT_API;
	const emit = opts.emit ?? emitOrderChanged;
	let order = $state.raw(initial);
	let conflicts = $state.raw<ReadonlyMap<OrderField, Conflict>>(new Map());
	let flashed = $state.raw<ReadonlySet<OrderField>>(new Set());
	let resolving: Promise<void> | null = null;
	let again = false;
	let tail: Promise<unknown> = Promise.resolve();

	/**
	 * Every write of this order goes one after another, saves and actions alike: each one is sent with the
	 * version the previous answer left, and after a conflict only once it is resolved.
	 */
	function serial<T>(write: () => Promise<T>): Promise<T> {
		const next = tail.then(async () => {
			if (resolving) await resolving;
			return write();
		});
		tail = next.catch(() => {});
		return next;
	}

	/** An answer of the server; one older than the order shown changes nothing. */
	function apply(o: Order): Order {
		if (o.id !== order.id || o.version < order.version) return o;
		order = o;
		emit(o);
		return o;
	}

	const inner = createAutosave<OrderValues>({
		delayMs: opts.delayMs ?? SAVE_DELAY_MS,
		save: (patch) =>
			serial(async () => {
				// A field that became disputed while this save waited is not sent over the other computer.
				const send = Object.fromEntries(
					Object.entries(patch).filter(([f]) => !conflicts.has(f as OrderField))
				) as Partial<OrderValues>;
				if (Object.keys(send).length === 0) return;
				apply(await api.updateOrder(order.id, order.version, send));
			}),
		onstate: (s) => {
			if (s === 'error' && isConflict(inner.error)) void resolveConflict();
		}
	});

	/** Own new text in a disputed field decides it: it is own. */
	function settle(field: OrderField) {
		if (!conflicts.has(field)) return;
		const next = new Map(conflicts);
		next.delete(field);
		conflicts = next;
	}

	/** The order as it is on the server now, against own unsaved changes. */
	function mergeIn(remote: Order) {
		const mine = Object.fromEntries([...conflicts].map(([f, c]) => [f, c.mine]));
		const local = { ...mine, ...inner.pending } as Partial<OrderValues>;
		const r = mergeRemote(valuesOf(order), local, valuesOf(remote));
		const next = new Map<OrderField, Conflict>();
		for (const f of r.conflicts) next.set(f, { mine: local[f]!, theirs: remote[f] });
		for (const [f, c] of conflicts) {
			if (!next.has(f) && !sameValue(c.mine, remote[f])) next.set(f, { ...c, theirs: remote[f] });
		}
		const alike = (Object.keys(local) as OrderField[]).filter(
			(f) => !next.has(f) && sameValue(local[f], remote[f])
		);
		// Disputed fields wait for a choice; the same change on both sides needs no saving.
		inner.discard(...next.keys(), ...alike);
		conflicts = next;
		flashed = new Set(r.changedByOthers);
		apply(remote);
		// The rest goes with the new version.
		inner.retry();
	}

	function resolveConflict(): Promise<void> {
		if (resolving) {
			again = true;
			return resolving;
		}
		resolving = (async () => {
			try {
				do {
					again = false;
					mergeIn(await api.getOrder(order.id));
				} while (again);
			} catch {
				// No answer: «Не сохранено · Повторить» stays, «Повторить» tries again.
			} finally {
				resolving = null;
			}
		})();
		return resolving;
	}

	const autosave: Autosave<OrderValues> = {
		get state() {
			return inner.state;
		},
		get error() {
			return inner.error;
		},
		get pending() {
			return inner.pending;
		},
		edit(field, value) {
			settle(field);
			inner.edit(field, value);
		},
		commit(field, value) {
			settle(field);
			inner.commit(field, value);
		},
		flush: () => inner.flush(),
		retry: () => inner.retry(),
		value(field, server) {
			const c = conflicts.get(field);
			return c ? (c.mine as typeof server) : inner.value(field, server);
		},
		discard: (...fields) => inner.discard(...fields),
		release: () => inner.release(),
		dispose: () => inner.dispose()
	};

	return {
		get order() {
			return order;
		},
		autosave,
		get conflicts() {
			return conflicts;
		},
		get flashed() {
			return flashed;
		},
		get saveState() {
			return conflicts.size > 0 ? 'conflict' : inner.state;
		},
		get current() {
			const shown = Object.fromEntries(ORDER_FIELDS.map((f) => [f, autosave.value(f, order[f])]));
			return { ...order, ...shown };
		},
		keep(field) {
			const c = conflicts.get(field);
			if (!c) return;
			settle(field);
			inner.commit(field, c.mine as never);
		},
		take(field) {
			settle(field);
			if (conflicts.size === 0) inner.retry();
		},
		endFlash(field) {
			if (!flashed.has(field)) return;
			const next = new Set(flashed);
			next.delete(field);
			flashed = next;
		},
		receive(o) {
			if (o.id === order.id && o.version > order.version) order = o;
		},
		async flush() {
			try {
				await inner.flush();
			} catch (e) {
				if (!isConflict(e)) return false;
				await (resolving ?? resolveConflict());
				if (conflicts.size > 0) return false;
				try {
					await inner.flush();
				} catch {
					return false;
				}
			}
			return conflicts.size === 0;
		},
		async run(action) {
			// What was typed goes first; a failed save shows in the indicator, the action is still tried.
			await inner.flush().catch(() => {});
			return serial(async () => {
				try {
					return apply(await action(order.version));
				} catch (e) {
					if (!isConflict(e)) throw e;
					await resolveConflict();
					return apply(await action(order.version));
				}
			});
		},
		dispose() {
			inner.dispose();
		}
	};
}

/**
 * «Отменить» of a status change: the old status on the newest version. Going back to an issued status
 * restores it as it was: no new issue date is stamped.
 */
export async function revertStatus(
	id: number,
	statusId: number,
	opts: Pick<EditorOptions, 'api' | 'emit'> = {}
): Promise<Order> {
	const api = opts.api ?? DEFAULT_API;
	const emit = opts.emit ?? emitOrderChanged;
	const current = await api.getOrder(id);
	const o = await api.changeStatus(id, current.version, {
		status_id: statusId,
		stamp_issued_at: false
	});
	emit(o);
	return o;
}

/**
 * «Отменить» of an issue: the status before it and, when the order had no issue date before, none again
 * (`clearIssuedAt` needs the right `order_dates_edit`; without it the date stays).
 */
export async function revertIssue(
	id: number,
	back: { statusId: number; clearIssuedAt: boolean },
	opts: Pick<EditorOptions, 'api' | 'emit'> = {}
): Promise<Order> {
	const api = opts.api ?? DEFAULT_API;
	const emit = opts.emit ?? emitOrderChanged;
	let o = await revertStatus(id, back.statusId, opts);
	if (back.clearIssuedAt && o.issued_at !== null) {
		o = await api.updateOrder(id, o.version, { issued_at: null });
		emit(o);
	}
	return o;
}
