import type { ErrorBody } from '$api/ErrorBody';
import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { item, order } from '#lib/app/fixtures.test-helper.js';
import {
	bulkErrorMessage,
	failureText,
	performBulk,
	runBulk,
	type BulkApi,
	type BulkRow
} from './bulk.js';

function apiError(code: 'conflict' | 'validation' | 'not_found', body: Partial<ErrorBody> = {}) {
	const messages = {
		conflict: 'Запись изменена другим пользователем',
		validation: 'Проверьте введённые данные',
		not_found: 'Заказ не найден'
	};
	const status = { conflict: 409, validation: 422, not_found: 404 }[code];
	return new ApiError(status, code, messages[code], {
		code,
		message: messages[code],
		fields: [],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		...body
	});
}

const rows = (...ids: number[]): BulkRow[] => ids.map((id) => ({ id, version: 1 }));

describe('runBulk', () => {
	it('retries a conflict once with the fresh version', async () => {
		const apply = vi.fn(async (row: BulkRow) => {
			if (row.id === 2 && row.version !== 9) throw apiError('conflict');
			return order(row.id, { version: row.version + 1 });
		});
		const getOrder = vi.fn(async (id: number) => order(id, { version: 9 }));
		const emit = vi.fn();
		const r = await runBulk(rows(1, 2), apply, { getOrder, emit });
		expect(r.done.map((o) => o.id)).toEqual([1, 2]);
		expect(r.failed).toEqual([]);
		expect(getOrder).toHaveBeenCalledExactlyOnceWith(2);
		expect(apply).toHaveBeenLastCalledWith({ id: 2, version: 9 });
		expect(emit.mock.calls.map(([o]) => [o.id, o.version])).toEqual([
			[1, 2],
			[2, 10]
		]);
	});

	it('collects failures and goes on', async () => {
		const invalid = apiError('validation');
		const apply = vi.fn(async (row: BulkRow) => {
			if (row.id === 2) throw invalid;
			return order(row.id);
		});
		const emit = vi.fn();
		const r = await runBulk(rows(1, 2, 3), apply, { getOrder: vi.fn(), emit });
		expect(r.done.map((o) => o.id)).toEqual([1, 3]);
		expect(r.failed).toEqual([{ id: 2, error: invalid }]);
		expect(emit).toHaveBeenCalledTimes(2);
	});

	it('goes one order after another', async () => {
		let inFlight = 0;
		let most = 0;
		const apply = async (row: BulkRow) => {
			inFlight += 1;
			most = Math.max(most, inFlight);
			await new Promise((r) => setTimeout(r, 1));
			inFlight -= 1;
			return order(row.id);
		};
		await runBulk(rows(1, 2, 3), apply, { getOrder: vi.fn(), emit: vi.fn() });
		expect(most).toBe(1);
	});

	it('a second conflict is a failure', async () => {
		const apply = vi.fn(async () => {
			throw apiError('conflict');
		});
		const r = await runBulk(rows(5), apply, {
			getOrder: async (id) => order(id, { version: 3 }),
			emit: vi.fn()
		});
		expect(apply).toHaveBeenCalledTimes(2);
		expect(r.failed.map((f) => [f.id, f.error.code])).toEqual([[5, 'conflict']]);
	});

	it('an order that cannot be fetched again after a conflict is a failure', async () => {
		const gone = apiError('not_found');
		const r = await runBulk(
			rows(5),
			async () => {
				throw apiError('conflict');
			},
			{
				getOrder: async () => {
					throw gone;
				},
				emit: vi.fn()
			}
		);
		expect(r.failed).toEqual([{ id: 5, error: gone }]);
	});

	it('an error that is not from the server is a failure too', async () => {
		const r = await runBulk(
			rows(5),
			async () => {
				throw new TypeError('boom');
			},
			{ getOrder: vi.fn(), emit: vi.fn() }
		);
		expect(r.failed[0]?.id).toBe(5);
		expect(r.failed[0]?.error).toBeInstanceOf(ApiError);
	});
});

describe('failure texts', () => {
	it('names the order and the reason', () => {
		const e = apiError('not_found');
		expect(failureText([{ id: 10481, error: e }])).toBe(
			'Не удалось у 1 заказа: № 10481 — Заказ не найден'
		);
	});

	it('lists a few orders and counts the rest', () => {
		const e = apiError('not_found');
		const failed = [1, 2, 3, 4, 5].map((id) => ({ id, error: e }));
		expect(failureText(failed)).toBe(
			'Не удалось у 5 заказов: № 1 — Заказ не найден; № 2 — Заказ не найден; № 3 — Заказ не найден и ещё 2'
		);
	});

	it('a validation error says which fields to fill in', () => {
		const e = apiError('validation', {
			fields: [
				{ field: 'master', code: 'required', message: 'Заполните поле' },
				{ field: 'work_done', code: 'required', message: 'Заполните поле' }
			]
		});
		expect(bulkErrorMessage(e)).toBe('заполните: мастер, выполненные работы');
		const configured = apiError('validation', {
			fields: [
				{ field: 'final_price', code: 'required_for_issue', message: 'Заполните перед выдачей' },
				{ field: 'warranty', code: 'required_for_issue', message: 'Заполните перед выдачей' }
			]
		});
		expect(bulkErrorMessage(configured)).toBe('заполните: итоговая цена, гарантия');
		const other = apiError('validation', {
			fields: [{ field: 'status_id', code: 'invalid', message: 'Недопустимое значение' }]
		});
		expect(bulkErrorMessage(other)).toBe('Недопустимое значение');
	});
});

/** A server of several orders: writes check the version like mw-server does. */
function fakeServer(orders: Order[]) {
	const db = new Map(orders.map((o) => [o.id, o]));
	function write(id: number, version: number, change: (o: Order) => Order): Promise<Order> {
		const current = db.get(id);
		if (!current) return Promise.reject(apiError('not_found'));
		if (current.version !== version) return Promise.reject(apiError('conflict'));
		const next = { ...change(current), version: current.version + 1 };
		db.set(id, next);
		return Promise.resolve(next);
	}
	const api = {
		getOrder: vi.fn(async (id: number) => {
			const o = db.get(id);
			if (!o) throw apiError('not_found');
			return o;
		}),
		changeStatus: vi.fn((id: number, version: number, c: StatusChange) =>
			write(id, version, (o) => ({
				...o,
				status_id: c.status_id,
				issued_at:
					c.status_id === 4 && c.stamp_issued_at !== false
						? (o.issued_at ?? '2026-11-17T12:00:00')
						: o.issued_at
			}))
		),
		updateOrder: vi.fn((id: number, version: number, p: OrderPatch) =>
			write(id, version, (o) => ({ ...o, ...p }) as Order)
		)
	} satisfies BulkApi;
	return { api, db };
}

function harness() {
	const undo: { label: string; run: () => Promise<void> }[] = [];
	const toasts: { message: string; tone?: string }[] = [];
	return {
		undo,
		toasts,
		deps: {
			undo: { push: (label: string, run: () => Promise<void>) => void undo.push({ label, run }) },
			toast: { show: (t: { message: string; tone?: 'danger' }) => void toasts.push(t) },
			emit: vi.fn()
		}
	};
}

const statusName = (id: number) => ({ 1: 'Принят', 3: 'Готов', 4: 'Выдан' })[id] ?? '';

describe('performBulk', () => {
	it('one undo puts back the status of each order', async () => {
		const { api, db } = fakeServer([order(1, { status_id: 1 }), order(2, { status_id: 3 })]);
		const h = harness();
		const r = await performBulk(
			[item(1, { status_id: 1 }), item(2, { status_id: 3 })],
			{ kind: 'status', statusId: 4, stamp: true },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(r.done).toHaveLength(2);
		expect(api.changeStatus).toHaveBeenCalledWith(1, 1, { status_id: 4, stamp_issued_at: true });
		expect(h.undo.map((u) => u.label)).toEqual(['Статус «Выдан» у 2 заказов']);
		expect(h.toasts).toEqual([]);
		expect(db.get(1)?.issued_at).not.toBeNull();

		await h.undo[0]!.run();
		expect(db.get(1)).toMatchObject({ status_id: 1, issued_at: null });
		expect(db.get(2)).toMatchObject({ status_id: 3, issued_at: null });
	});

	it('undo keeps an issue date that was there before, and any date without the right', async () => {
		const { api, db } = fakeServer([
			order(1, { status_id: 3, issued_at: '2020-01-10T10:00:00' }),
			order(2, { status_id: 3 })
		]);
		const h = harness();
		await performBulk(
			[item(1, { status_id: 3, issued_at: '2020-01-10T10:00:00' }), item(2, { status_id: 3 })],
			{ kind: 'status', statusId: 4, stamp: true },
			{ statusName, canClearIssuedAt: false },
			{ api, ...h.deps }
		);
		await h.undo[0]!.run();
		expect(db.get(1)).toMatchObject({ status_id: 3, issued_at: '2020-01-10T10:00:00' });
		expect(db.get(2)).toMatchObject({ status_id: 3, issued_at: '2026-11-17T12:00:00' });
		expect(api.updateOrder).not.toHaveBeenCalled();
	});

	it('undo back to an issued status does not stamp a date it did not have', async () => {
		const { api, db } = fakeServer([order(1, { status_id: 4 })]);
		const h = harness();
		await performBulk(
			[item(1, { status_id: 4 })],
			{ kind: 'status', statusId: 3 },
			{ statusName, canClearIssuedAt: false },
			{ api, ...h.deps }
		);
		expect(db.get(1)?.status_id).toBe(3);
		await h.undo[0]!.run();
		expect(db.get(1)).toMatchObject({ status_id: 4, issued_at: null });
		expect(api.updateOrder).not.toHaveBeenCalled();
	});

	it('issue without a date sends stamp_issued_at false', async () => {
		const { api, db } = fakeServer([order(1, { status_id: 3 })]);
		const h = harness();
		await performBulk(
			[item(1, { status_id: 3 })],
			{ kind: 'status', statusId: 4, stamp: false },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(api.changeStatus).toHaveBeenCalledWith(1, 1, { status_id: 4, stamp_issued_at: false });
		expect(db.get(1)?.issued_at).toBeNull();
		expect(h.undo.map((u) => u.label)).toEqual(['Статус «Выдан» у 1 заказа']);
	});

	it('master and its undo, each order back to its own master', async () => {
		const { api, db } = fakeServer([order(1, { master: 'Антон' }), order(2, { master: '' })]);
		const h = harness();
		await performBulk(
			[item(1, { master: 'Антон' }), item(2, { master: '' })],
			{ kind: 'master', master: 'Борис' },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(db.get(1)?.master).toBe('Борис');
		expect(db.get(2)?.master).toBe('Борис');
		expect(h.undo.map((u) => u.label)).toEqual(['Мастер «Борис» у 2 заказов']);
		await h.undo[0]!.run();
		expect(db.get(1)?.master).toBe('Антон');
		expect(db.get(2)?.master).toBe('');
	});

	it('labels of removing the master and of the marks', async () => {
		const { api } = fakeServer([order(1, { master: 'Антон' })]);
		const h = harness();
		const ctx = { statusName, canClearIssuedAt: true };
		const deps = { api, ...h.deps };
		await performBulk([item(1)], { kind: 'master', master: '' }, ctx, deps);
		await performBulk([item(1)], { kind: 'flag', flag: 'need_call', value: true }, ctx, deps);
		await performBulk([item(1)], { kind: 'flag', flag: 'need_parts', value: false }, ctx, deps);
		expect(h.undo.map((u) => u.label)).toEqual([
			'Мастер снят у 1 заказа',
			'Отметка «Позвонить» у 1 заказа',
			'Отметка «Заказать запчасть» снята у 1 заказа'
		]);
	});

	it('a mark and its undo', async () => {
		const { api, db } = fakeServer([order(1, { need_call: true }), order(2)]);
		const h = harness();
		await performBulk(
			[item(1, { need_call: true }), item(2)],
			{ kind: 'flag', flag: 'need_call', value: true },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(api.updateOrder).toHaveBeenCalledWith(2, 1, { need_call: true });
		await h.undo[0]!.run();
		expect(db.get(1)?.need_call).toBe(true);
		expect(db.get(2)?.need_call).toBe(false);
	});

	it('reports the orders that failed and offers undo for the rest', async () => {
		const { api } = fakeServer([order(1)]);
		const h = harness();
		const r = await performBulk(
			[item(1), item(10481)],
			{ kind: 'master', master: 'Борис' },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(r.failed.map((f) => f.id)).toEqual([10481]);
		expect(h.undo.map((u) => u.label)).toEqual(['Мастер «Борис» у 1 заказа']);
		expect(h.toasts).toEqual([
			{ message: 'Не удалось у 1 заказа: № 10481 — Заказ не найден', tone: 'danger' }
		]);
	});

	it('nothing done: no undo, only the failure', async () => {
		const { api } = fakeServer([]);
		const h = harness();
		await performBulk(
			[item(7)],
			{ kind: 'master', master: 'Борис' },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		expect(h.undo).toEqual([]);
		expect(h.toasts.map((t) => t.message)).toEqual([
			'Не удалось у 1 заказа: № 7 — Заказ не найден'
		]);
	});

	it('undo that fails for an order says so', async () => {
		const { api, db } = fakeServer([order(1, { master: 'Антон' })]);
		const h = harness();
		await performBulk(
			[item(1, { master: 'Антон' })],
			{ kind: 'master', master: 'Борис' },
			{ statusName, canClearIssuedAt: true },
			{ api, ...h.deps }
		);
		db.delete(1);
		await h.undo[0]!.run();
		expect(h.toasts).toEqual([
			{ message: 'Не удалось отменить у 1 заказа: № 1 — Заказ не найден', tone: 'danger' }
		]);
	});
});
