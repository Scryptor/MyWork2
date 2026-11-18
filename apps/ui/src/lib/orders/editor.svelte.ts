// Editing one order on the server (spec 6.2): the order's fields and requests over the shared editor
// (autosave with the version, the conflict, actions on the newest version). The panel and the full card
// each make one editor per open order.
import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { ApiError } from '#lib/api/client.js';
import { changeStatus, getOrder, updateOrder } from '#lib/api/orders.js';
import {
	createEntityEditor,
	type Conflict as FieldConflict,
	type EntityEditor
} from '#lib/app/entity-editor.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import { emitOrderChanged } from './events.js';

export { SAVE_DELAY_MS, type EditorSaveState } from '#lib/app/entity-editor.svelte.js';

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
export type Conflict = FieldConflict<OrderValues[OrderField]>;

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

/** The shared editor of one order; `order` is its record. */
export interface OrderEditor extends EntityEditor<Order, OrderField> {
	/** The order as the server last gave it. */
	readonly order: Order;
}

const isDeleted = (e: ApiError | null) =>
	e?.code === 'validation' &&
	(e.body?.fields ?? []).some((f) => f.field === 'id' && f.code === 'deleted');

/** A save no retry can help: the order is deleted, or editing it is not allowed. */
export function isLost(e: ApiError | null): boolean {
	return e?.code === 'forbidden' || isDeleted(e);
}

/** Why a save failed, for the indicator and the toasts: the server's text, the deleted order told plainly. */
export function saveErrorText(e: ApiError | null): string {
	if (isDeleted(e)) return ru.save.deleted;
	return e?.message || ru.api.failed;
}

export function createOrderEditor(initial: Order, opts: EditorOptions = {}): OrderEditor {
	const api = opts.api ?? DEFAULT_API;
	const ed = createEntityEditor<Order, OrderField>({
		initial,
		fields: ORDER_FIELDS,
		load: () => api.getOrder(initial.id),
		save: (patch, version) => api.updateOrder(initial.id, version, patch),
		emit: opts.emit ?? emitOrderChanged,
		lostText: (e) => (isLost(e) ? saveErrorText(e) : null),
		delayMs: opts.delayMs
	});
	return Object.defineProperty(ed, 'order', {
		get: () => ed.record,
		enumerable: true
	}) as OrderEditor;
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
