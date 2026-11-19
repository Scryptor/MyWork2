import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { order } from '#lib/app/fixtures.test-helper.js';
import {
	createOrderEditor,
	revertIssue,
	revertStatus,
	saveErrorText,
	type EditorApi
} from './editor.svelte.js';

const conflict = () =>
	new ApiError(409, 'conflict', 'Запись изменена', {
		code: 'conflict',
		message: 'Запись изменена',
		fields: [],
		current_version: 0,
		permission: null,
		retry_after_secs: null
	});

/**
 * A server of one order: writes check the version like mw-server does. It counts writes in flight at
 * once and the 409s; `hold` keeps writes waiting until `release`.
 */
function fakeServer(start: Order) {
	let current = start;
	let inFlight = 0;
	let gate: Promise<void> = Promise.resolve();
	let open = () => {};
	const stats = { maxInFlight: 0, conflicts: 0 };
	async function write(version: number, change: () => Order): Promise<Order> {
		inFlight += 1;
		stats.maxInFlight = Math.max(stats.maxInFlight, inFlight);
		try {
			await gate;
			await Promise.resolve();
			if (version !== current.version) {
				stats.conflicts += 1;
				throw conflict();
			}
			current = { ...change(), version: current.version + 1 };
			return current;
		} finally {
			inFlight -= 1;
		}
	}
	const api = {
		getOrder: vi.fn(async () => current),
		updateOrder: vi.fn((_id: number, version: number, patch: OrderPatch) =>
			write(version, () => ({ ...current, ...patch }) as Order)
		),
		changeStatus: vi.fn((_id: number, version: number, change: StatusChange) =>
			write(version, () => ({ ...current, status_id: change.status_id }))
		)
	} satisfies EditorApi;
	return {
		api,
		stats,
		/** Someone at another computer edits the order. */
		elsewhere(patch: Partial<Order>) {
			current = { ...current, ...patch, version: current.version + 1 };
		},
		hold() {
			gate = new Promise((r) => (open = r));
		},
		release() {
			open();
			gate = Promise.resolve();
		},
		get current() {
			return current;
		}
	};
}

const settle = async () => {
	for (let i = 0; i < 20; i++) await Promise.resolve();
};

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createOrderEditor', () => {
	it('saves with the current version, takes the answer and tells the others', async () => {
		const server = fakeServer(order(7, { version: 3 }));
		const emit = vi.fn();
		const ed = createOrderEditor(server.current, { api: server.api, emit });
		ed.autosave.edit('malfunction', 'Не заряжается');
		vi.advanceTimersByTime(600);
		await settle();
		expect(server.api.updateOrder).toHaveBeenCalledWith(7, 3, { malfunction: 'Не заряжается' });
		expect(ed.order.version).toBe(4);
		expect(ed.order.malfunction).toBe('Не заряжается');
		expect(emit).toHaveBeenCalledWith(ed.order);

		ed.autosave.commit('need_call', true);
		await settle();
		expect(server.api.updateOrder).toHaveBeenLastCalledWith(7, 4, { need_call: true });
		ed.dispose();
	});

	it('a conflict takes what others changed and asks about the fields both changed', async () => {
		const server = fakeServer(order(7, { malfunction: 'Не включается', master: '' }));
		const emit = vi.fn();
		const ed = createOrderEditor(server.current, { api: server.api, emit });
		server.elsewhere({ malfunction: 'Не включается после воды', model: 'Другая', master: 'Антон' });
		ed.autosave.edit('malfunction', 'Не включается, разбит экран');
		ed.autosave.edit('work_done', 'Диагностика');
		ed.autosave.edit('master', 'Антон');
		await expect(ed.autosave.flush()).rejects.toMatchObject({ code: 'conflict' });
		await settle();

		expect(server.api.getOrder).toHaveBeenCalledWith(7);
		expect([...ed.conflicts.keys()]).toEqual(['malfunction']);
		expect(ed.conflicts.get('malfunction')).toEqual({
			mine: 'Не включается, разбит экран',
			theirs: 'Не включается после воды'
		});
		expect([...ed.flashed]).toEqual(['model']);
		expect(ed.order.model).toBe('Другая');
		// Own text stays in the field until the choice.
		expect(ed.autosave.value('malfunction', ed.order.malfunction)).toBe(
			'Не включается, разбит экран'
		);
		// The rest went with the new version; the same master was not sent.
		expect(server.api.updateOrder).toHaveBeenLastCalledWith(7, 2, { work_done: 'Диагностика' });
		expect(server.current.work_done).toBe('Диагностика');
		expect(ed.saveState).toBe('conflict');
		await expect(ed.flush()).resolves.toBe(false);

		ed.take('malfunction');
		expect(ed.conflicts.size).toBe(0);
		expect(ed.autosave.value('malfunction', ed.order.malfunction)).toBe('Не включается после воды');
		expect(ed.saveState).toBe('saved');
		await expect(ed.flush()).resolves.toBe(true);
		expect(emit).toHaveBeenCalled();
		ed.dispose();
	});

	it('leaving over an undecided dispute asks: «Остаться» keeps it, «Отбросить и перейти» drops it', async () => {
		const server = fakeServer(order(7, { malfunction: 'Не включается' }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.elsewhere({ malfunction: 'Чужое' });
		ed.autosave.edit('malfunction', 'Моё');
		await ed.autosave.flush().catch(() => {});
		await settle();
		expect(ed.conflicts.size).toBe(1);

		const stay = vi.fn(async () => false);
		await expect(ed.leave(stay)).resolves.toBe(false);
		expect(stay).toHaveBeenCalledTimes(1);
		expect(ed.conflicts.size).toBe(1);

		const go = vi.fn(async () => true);
		await expect(ed.leave(go)).resolves.toBe(true);
		expect(ed.conflicts.size).toBe(0);
		expect(ed.unsaved).toBe(false);
		ed.dispose();
	});

	it('leaving with nothing disputed does not ask', async () => {
		const server = fakeServer(order(7));
		const ed = createOrderEditor(server.current, { api: server.api });
		ed.autosave.edit('malfunction', 'Новое');
		const ask = vi.fn(async () => true);
		await expect(ed.leave(ask)).resolves.toBe(true);
		expect(ask).not.toHaveBeenCalled();
		expect(server.current.malfunction).toBe('Новое');
		ed.dispose();
	});

	it('«Оставить моё» sends own value with the new version', async () => {
		const server = fakeServer(order(7, { work_done: '' }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.elsewhere({ work_done: 'Чистка' });
		ed.autosave.edit('work_done', 'Замена разъёма');
		await ed.autosave.flush().catch(() => {});
		await settle();
		expect(ed.conflicts.has('work_done')).toBe(true);

		ed.keep('work_done');
		await settle();
		expect(server.api.updateOrder).toHaveBeenLastCalledWith(7, 2, { work_done: 'Замена разъёма' });
		expect(server.current.work_done).toBe('Замена разъёма');
		expect(ed.conflicts.size).toBe(0);
		expect(ed.saveState).toBe('saved');
		ed.dispose();
	});

	it('typing in a disputed field keeps the new text as own', async () => {
		const server = fakeServer(order(7));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.elsewhere({ malfunction: 'Чужое' });
		ed.autosave.edit('malfunction', 'Моё');
		await ed.autosave.flush().catch(() => {});
		await settle();
		ed.autosave.edit('malfunction', 'Моё, дописал');
		expect(ed.conflicts.size).toBe(0);
		vi.advanceTimersByTime(600);
		await settle();
		expect(server.current.malfunction).toBe('Моё, дописал');
		ed.dispose();
	});

	it('a status change first saves the text, and retries once with a fresh version', async () => {
		const server = fakeServer(order(7, { status_id: 1 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		ed.autosave.edit('malfunction', 'Текст');
		const changed = ed.run((version) => server.api.changeStatus(7, version, { status_id: 2 }));
		await settle();
		expect(server.api.updateOrder).toHaveBeenCalledWith(7, 1, { malfunction: 'Текст' });
		const o = await changed;
		expect(o.status_id).toBe(2);
		expect(ed.order.version).toBe(3);

		server.elsewhere({ model: 'Новая' });
		const again = await ed.run((version) => server.api.changeStatus(7, version, { status_id: 3 }));
		expect(again.status_id).toBe(3);
		expect(ed.order.model).toBe('Новая');
		expect(server.api.changeStatus).toHaveBeenCalledTimes(3);
		ed.dispose();
	});

	it('pending text and a status change: one request at a time, no conflict, both land', async () => {
		const server = fakeServer(order(7, { status_id: 1 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		ed.autosave.edit('malfunction', 'Разбит экран');
		ed.autosave.edit('work_done', 'Замена экрана');
		const o = await ed.run((v) => server.api.changeStatus(7, v, { status_id: 2 }));
		await settle();
		expect(o.status_id).toBe(2);
		expect(server.current).toMatchObject({
			status_id: 2,
			malfunction: 'Разбит экран',
			work_done: 'Замена экрана'
		});
		expect(server.stats).toEqual({ maxInFlight: 1, conflicts: 0 });
		expect(ed.autosave.state).toBe('saved');
		ed.dispose();
	});

	it('typing during a status change sends after it, with the new version', async () => {
		const server = fakeServer(order(7, { status_id: 1 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.hold();
		const changed = ed.run((v) => server.api.changeStatus(7, v, { status_id: 2 }));
		await settle();
		expect(server.api.changeStatus).toHaveBeenCalledTimes(1);

		ed.autosave.commit('need_call', true);
		ed.autosave.edit('malfunction', 'Текст');
		vi.advanceTimersByTime(600);
		await settle();
		// Waiting for the status: nothing else is out.
		expect(server.api.updateOrder).not.toHaveBeenCalled();
		expect(ed.autosave.state).toBe('saving');

		server.release();
		await changed;
		await settle();
		// The switch left at once and the text after it: each with the version the previous answer left.
		expect(server.api.updateOrder.mock.calls.map((c) => [c[1], c[2]])).toEqual([
			[2, { need_call: true }],
			[3, { malfunction: 'Текст' }]
		]);
		expect(server.current).toMatchObject({ status_id: 2, need_call: true, malfunction: 'Текст' });
		expect(server.stats).toEqual({ maxInFlight: 1, conflicts: 0 });
		expect(ed.order.version).toBe(4);
		ed.dispose();
	});

	it('two status picks in a row go one after the other; the second one stays', async () => {
		const server = fakeServer(order(7, { status_id: 1 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.hold();
		const first = ed.run((v) => server.api.changeStatus(7, v, { status_id: 2 }));
		const second = ed.run((v) => server.api.changeStatus(7, v, { status_id: 3 }));
		await settle();
		server.release();
		await Promise.all([first, second]);
		expect(server.api.changeStatus.mock.calls.map((c) => c[1])).toEqual([1, 2]);
		expect(server.current.status_id).toBe(3);
		expect(ed.order.status_id).toBe(3);
		expect(server.stats).toEqual({ maxInFlight: 1, conflicts: 0 });
		ed.dispose();
	});

	it('a status change over a conflict: the remaining text goes first, then the status', async () => {
		const server = fakeServer(order(7, { status_id: 1 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.elsewhere({ model: 'Другая' });
		ed.autosave.edit('work_done', 'Чистка');
		const o = await ed.run((v) => server.api.changeStatus(7, v, { status_id: 4 }));
		await settle();
		expect(o.status_id).toBe(4);
		expect(server.current).toMatchObject({ status_id: 4, work_done: 'Чистка', model: 'Другая' });
		// The one 409 is the edit made elsewhere; nothing ran side by side.
		expect(server.stats).toEqual({ maxInFlight: 1, conflicts: 1 });
		expect(ed.order.version).toBe(server.current.version);
		ed.dispose();
	});

	it('a disputed field waiting in the queue is not sent over the other computer', async () => {
		const server = fakeServer(order(7, { status_id: 1, malfunction: 'Старое' }));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.hold();
		const changed = ed.run((v) => server.api.changeStatus(7, v, { status_id: 2 }));
		await settle();
		ed.autosave.commit('malfunction', 'Моё');
		server.elsewhere({ malfunction: 'Чужое' });
		server.release();
		await changed;
		await settle();
		expect(ed.conflicts.get('malfunction')).toEqual({ mine: 'Моё', theirs: 'Чужое' });
		expect(server.current.malfunction).toBe('Чужое');
		expect(ed.autosave.value('malfunction', ed.order.malfunction)).toBe('Моё');
		ed.dispose();
	});

	it('unsaved: while text waits, while it is sent, after a failure and with a conflict', async () => {
		const server = fakeServer(order(7));
		const ed = createOrderEditor(server.current, { api: server.api });
		expect(ed.unsaved).toBe(false);
		ed.autosave.edit('work_done', 'Чистка');
		expect(ed.unsaved).toBe(true);
		server.hold();
		vi.advanceTimersByTime(600);
		await settle();
		expect(ed.saveState).toBe('saving');
		expect(ed.unsaved).toBe(true);
		server.release();
		await settle();
		expect(ed.saveState).toBe('saved');
		expect(ed.unsaved).toBe(false);

		server.api.updateOrder.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи', null));
		ed.autosave.edit('work_done', 'Чистка контактов');
		await ed.flush();
		expect(ed.saveState).toBe('error');
		expect(ed.unsaved).toBe(true);
		ed.autosave.retry();
		await settle();
		expect(ed.unsaved).toBe(false);

		server.elsewhere({ work_done: 'Чужое' });
		ed.autosave.edit('work_done', 'Моё');
		await ed.flush();
		expect(ed.saveState).toBe('conflict');
		expect(ed.unsaved).toBe(true);
		ed.take('work_done');
		expect(ed.unsaved).toBe(false);
		ed.dispose();
	});

	it('a save refused for good is not offered again: dropping own edits lets the order go', async () => {
		const deleted = new ApiError(422, 'validation', 'Проверьте введённые данные', {
			code: 'validation',
			message: 'Проверьте введённые данные',
			fields: [{ field: 'id', code: 'deleted', message: 'Запись удалена' }],
			current_version: null,
			permission: null,
			retry_after_secs: null
		});
		const forbidden = new ApiError(403, 'forbidden', 'Недостаточно прав: Изменение заказов', null);
		for (const refusal of [deleted, forbidden]) {
			const server = fakeServer(order(7, { work_done: '' }));
			const ed = createOrderEditor(server.current, { api: server.api });
			let gate = Promise.resolve();
			server.api.updateOrder.mockImplementation(async () => {
				await gate;
				throw refusal;
			});
			ed.autosave.edit('work_done', 'Чистка');
			await expect(ed.flush()).resolves.toBe(false);
			expect(ed.saveState).toBe('error');
			expect(ed.lost).toBe(saveErrorText(refusal));
			expect(ed.unsaved).toBe(true);
			// Leaving the field sends it again: meanwhile it stays refused, «Отбросить» stays on screen.
			let answer = () => {};
			gate = new Promise((r) => (answer = r));
			void ed.autosave.flush().catch(() => {});
			await settle();
			expect(ed.saveState).toBe('saving');
			expect(ed.lost).toBe(saveErrorText(refusal));

			ed.drop();
			answer();
			await settle();
			expect(ed.unsaved).toBe(false);
			expect(ed.lost).toBeNull();
			expect(ed.saveState).not.toBe('error');
			expect(ed.current.work_done).toBe('');
			const calls = server.api.updateOrder.mock.calls.length;
			await expect(ed.flush()).resolves.toBe(true);
			expect(server.api.updateOrder).toHaveBeenCalledTimes(calls);
			ed.dispose();
		}
		expect(saveErrorText(deleted)).toBe('заказ удалён');
		expect(saveErrorText(forbidden)).toBe('Недостаточно прав: Изменение заказов');
	});

	it('no connection or a failure of the server can still be retried', async () => {
		const server = fakeServer(order(7));
		const ed = createOrderEditor(server.current, { api: server.api });
		server.api.updateOrder.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи', null));
		ed.autosave.edit('work_done', 'Чистка');
		await ed.flush();
		expect(ed.saveState).toBe('error');
		expect(ed.lost).toBeNull();
		server.api.updateOrder.mockRejectedValueOnce(new ApiError(500, 'internal', 'Сбой', null));
		ed.autosave.retry();
		await settle();
		expect(ed.saveState).toBe('error');
		expect(ed.lost).toBeNull();
		ed.dispose();
	});

	it('takes newer orders from elsewhere in the program and ignores older ones', () => {
		const server = fakeServer(order(7, { version: 5 }));
		const ed = createOrderEditor(server.current, { api: server.api });
		ed.receive(order(7, { version: 4, model: 'Старая' }));
		expect(ed.order.version).toBe(5);
		ed.receive(order(8, { version: 9 }));
		expect(ed.order.id).toBe(7);
		ed.receive(order(7, { version: 6, status_id: 4 }));
		expect(ed.order.status_id).toBe(4);
		ed.dispose();
	});
});

describe('undo of a status', () => {
	it('revertStatus sets the old status on the newest version', async () => {
		const server = fakeServer(order(7, { status_id: 2, version: 4 }));
		const emit = vi.fn();
		await revertStatus(7, 1, { api: server.api, emit });
		// Undo restores: going back to an issued status never stamps a new issue date.
		expect(server.api.changeStatus).toHaveBeenCalledWith(7, 4, {
			status_id: 1,
			stamp_issued_at: false
		});
		expect(emit).toHaveBeenCalledWith(expect.objectContaining({ status_id: 1, version: 5 }));
	});

	it('revertIssue also takes the issue date away when there was none', async () => {
		const server = fakeServer(order(7, { status_id: 8, issued_at: '2026-11-17T10:00:00' }));
		const emit = vi.fn();
		await revertIssue(7, { statusId: 6, clearIssuedAt: true }, { api: server.api, emit });
		expect(server.current.status_id).toBe(6);
		expect(server.api.updateOrder).toHaveBeenCalledWith(7, 2, { issued_at: null });
		expect(server.current.issued_at).toBeNull();
		expect(emit).toHaveBeenCalledTimes(2);

		await revertIssue(7, { statusId: 5, clearIssuedAt: false }, { api: server.api, emit });
		expect(server.api.updateOrder).toHaveBeenCalledTimes(1);
	});
});
