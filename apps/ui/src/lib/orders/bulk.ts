// Bulk actions of the orders list (spec 6.1): one order after another, each on its newest version; one
// «Отменить» for the whole batch puts back what every order had before.
import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { changeStatus, getOrder, updateOrder } from '#lib/api/orders.js';
import { undo as appUndo } from '#lib/app/undo.svelte.js';
import { countLabel } from '#lib/format/plural.js';
import { ru } from '#lib/i18n/ru.js';
import { toast as appToast } from '#lib/ui/toast/toast.svelte.js';
import { emitOrderChanged } from './events.js';

/** An order of the batch and the version the request is sent with. */
export interface BulkRow {
	id: number;
	version: number;
}

export interface BulkFailure {
	id: number;
	error: ApiError;
}

export interface BulkResult {
	done: Order[];
	failed: BulkFailure[];
}

export interface BulkDeps {
	getOrder?: (id: number) => Promise<Order>;
	/** Every changed order goes here: lists, the summary and open editors catch up. */
	emit?: (o: Order) => void;
}

const asApiError = (e: unknown): ApiError =>
	e instanceof ApiError ? e : new ApiError(0, 'internal', errorMessage(e), null);
const isConflict = (e: unknown) => e instanceof ApiError && e.code === 'conflict';

/**
 * Applies a change to each order, one after another. A conflict (the order was changed elsewhere)
 * takes the fresh version and tries once more; any other failure is collected and the batch goes on.
 */
export async function runBulk(
	rows: readonly BulkRow[],
	apply: (row: BulkRow) => Promise<Order>,
	deps: BulkDeps = {}
): Promise<BulkResult> {
	const reload = deps.getOrder ?? ((id: number) => getOrder(id));
	const emit = deps.emit ?? emitOrderChanged;
	const done: Order[] = [];
	const failed: BulkFailure[] = [];
	for (const row of rows) {
		try {
			let o: Order;
			try {
				o = await apply(row);
			} catch (e) {
				if (!isConflict(e)) throw e;
				const fresh = await reload(row.id);
				o = await apply({ id: row.id, version: fresh.version });
			}
			emit(o);
			done.push(o);
		} catch (e) {
			failed.push({ id: row.id, error: asApiError(e) });
		}
	}
	return { done, failed };
}

/** One reason in words: a validation error names the fields to fill in. */
export function bulkErrorMessage(e: ApiError): string {
	const fields = e.code === 'validation' ? (e.body?.fields ?? []) : [];
	if (fields.length === 0) return errorMessage(e);
	const required = fields.filter(
		(f) => (f.code === 'required' || f.code === 'required_for_issue') && ru.bulk.fields[f.field]
	);
	if (required.length === fields.length) {
		return ru.bulk.fill(required.map((f) => ru.bulk.fields[f.field]).join(', '));
	}
	return fields.map((f) => f.message).join('; ');
}

/** Orders listed by the text of a failure; the rest only counted. */
const LISTED = 3;

/** «Не удалось у 1 заказа: № 10481 — <сообщение>». */
export function failureText(
	failed: readonly BulkFailure[],
	text: (orders: string, details: string) => string = ru.bulk.failed
): string {
	const listed = failed
		.slice(0, LISTED)
		.map((f) => ru.bulk.failure(f.id, bulkErrorMessage(f.error)));
	const rest = failed.length - LISTED;
	const details = listed.join('; ') + (rest > 0 ? ` ${ru.bulk.andMore(rest)}` : '');
	return text(countLabel(failed.length, ru.bulk.forms), details);
}

/** What the selection bar changes. */
export type BulkChange =
	| { kind: 'status'; statusId: number; stamp?: boolean }
	/** `masterId` null — «Снять мастера»; `name` — for the toast. */
	| { kind: 'master'; masterId: number | null; name: string }
	| { kind: 'flag'; flag: 'need_call' | 'need_parts'; value: boolean };

export interface BulkApi {
	getOrder: (id: number) => Promise<Order>;
	changeStatus: (id: number, version: number, change: StatusChange) => Promise<Order>;
	updateOrder: (id: number, version: number, patch: OrderPatch) => Promise<Order>;
}

export interface BulkContext {
	statusName: (id: number) => string;
	/** The user may clear the issue date: undo of an issue takes away a date it stamped. */
	canClearIssuedAt: boolean;
}

export interface PerformDeps {
	api?: BulkApi;
	emit?: (o: Order) => void;
	undo?: { push: (label: string, run: () => Promise<void>) => void };
	toast?: { show: (o: { message: string; tone?: 'danger' }) => void };
}

const DEFAULT_API: BulkApi = { getOrder: (id) => getOrder(id), changeStatus, updateOrder };
const FLAG_LABELS = { need_call: ru.flags.needCall, need_parts: ru.flags.needParts } as const;

/** «Статус «Готов» у 3 заказов». */
function doneText(change: BulkChange, n: number, ctx: BulkContext): string {
	const orders = countLabel(n, ru.bulk.forms);
	switch (change.kind) {
		case 'status':
			return ru.bulk.statusDone(ctx.statusName(change.statusId), orders);
		case 'master':
			return change.masterId === null
				? ru.bulk.masterCleared(orders)
				: ru.bulk.masterDone(change.name, orders);
		case 'flag':
			return change.value
				? ru.bulk.markOnDone(FLAG_LABELS[change.flag], orders)
				: ru.bulk.markOffDone(FLAG_LABELS[change.flag], orders);
	}
}

function send(change: BulkChange, row: BulkRow, api: BulkApi): Promise<Order> {
	switch (change.kind) {
		case 'status': {
			const c: StatusChange = { status_id: change.statusId };
			if (change.stamp !== undefined) c.stamp_issued_at = change.stamp;
			return api.changeStatus(row.id, row.version, c);
		}
		case 'master':
			return api.updateOrder(row.id, row.version, { master_user_id: change.masterId });
		case 'flag':
			return api.updateOrder(row.id, row.version, { [change.flag]: change.value });
	}
}

/** Puts back what the list showed before the change; a stamped issue date goes away if allowed. */
async function putBack(
	change: BulkChange,
	before: Order,
	row: BulkRow,
	api: BulkApi,
	ctx: BulkContext
): Promise<Order> {
	switch (change.kind) {
		case 'status': {
			// Back to an issued status: as it was, no new date.
			let o = await api.changeStatus(row.id, row.version, {
				status_id: before.status_id,
				stamp_issued_at: false
			});
			if (before.issued_at === null && o.issued_at !== null && ctx.canClearIssuedAt) {
				o = await api.updateOrder(row.id, o.version, { issued_at: null });
			}
			return o;
		}
		case 'master':
			return api.updateOrder(row.id, row.version, { master_user_id: before.master_user_id });
		case 'flag':
			return api.updateOrder(row.id, row.version, { [change.flag]: before[change.flag] });
	}
}

const rowOf = (o: { id: number; version: number }): BulkRow => ({ id: o.id, version: o.version });

/**
 * A change of the selection bar on the selected orders: the toast «… · Отменить» for those done, one
 * undo for all of them, and the toast of the orders that failed.
 */
export async function performBulk(
	items: readonly OrderListItem[],
	change: BulkChange,
	ctx: BulkContext,
	deps: PerformDeps = {}
): Promise<BulkResult> {
	const api = deps.api ?? DEFAULT_API;
	const undo = deps.undo ?? appUndo;
	const toast = deps.toast ?? appToast;
	const runDeps: BulkDeps = { getOrder: api.getOrder, emit: deps.emit };
	// What each order had a moment before its own change: «Отменить» puts back this, not what the list
	// showed when it was loaded (the order may have been changed since).
	const before = new Map<number, Order>();
	const apply = async (row: BulkRow): Promise<Order> => {
		const current = await api.getOrder(row.id);
		before.set(row.id, current);
		return send(change, { id: row.id, version: current.version }, api);
	};

	const r = await runBulk(items.map(rowOf), apply, runDeps);
	if (r.done.length > 0) {
		undo.push(doneText(change, r.done.length, ctx), async () => {
			const back = await runBulk(
				r.done.map(rowOf),
				(row) => putBack(change, before.get(row.id)!, row, api, ctx),
				runDeps
			);
			if (back.failed.length > 0) {
				toast.show({ message: failureText(back.failed, ru.bulk.undoFailed), tone: 'danger' });
			}
		});
	}
	if (r.failed.length > 0) {
		// Nothing changed at all: say so, the selection stays for another try.
		const text = r.done.length === 0 ? ru.bulk.nothingChanged : ru.bulk.failed;
		toast.show({ message: failureText(r.failed, text), tone: 'danger' });
	}
	return r;
}
