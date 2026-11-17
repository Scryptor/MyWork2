import type { Order } from '$api/Order';
import type { OrderPatch } from '$api/OrderPatch';
import type { StatusChange } from '$api/StatusChange';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { order } from '#lib/app/fixtures.test-helper.js';
import { createOrderEditor, revertIssue, revertStatus, type EditorApi } from './editor.svelte.js';

const conflict = () =>
	new ApiError(409, 'conflict', 'Запись изменена', {
		code: 'conflict',
		message: 'Запись изменена',
		fields: [],
		current_version: 0,
		permission: null,
		retry_after_secs: null
	});

/** A server of one order: PATCH and status check the version like mw-server does. */
function fakeServer(start: Order) {
	let current = start;
	const api = {
		getOrder: vi.fn(async () => current),
		updateOrder: vi.fn(async (_id: number, version: number, patch: OrderPatch) => {
			if (version !== current.version) throw conflict();
			current = { ...current, ...patch, version: current.version + 1 } as Order;
			return current;
		}),
		changeStatus: vi.fn(async (_id: number, version: number, change: StatusChange) => {
			if (version !== current.version) throw conflict();
			current = { ...current, status_id: change.status_id, version: current.version + 1 };
			return current;
		})
	} satisfies EditorApi;
	return {
		api,
		/** Someone at another computer edits the order. */
		elsewhere(patch: Partial<Order>) {
			current = { ...current, ...patch, version: current.version + 1 };
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
		expect(server.api.changeStatus).toHaveBeenCalledWith(7, 4, { status_id: 1 });
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
