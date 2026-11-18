import type { Client } from '$api/Client';
import type { ClientPatch } from '$api/ClientPatch';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { client } from '#lib/app/fixtures.test-helper.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	clientFieldErrors,
	closeClientEditor,
	createClientEditor,
	type ClientEditorApi
} from './editor.svelte.js';
import { savedPhones } from './phones.js';

const conflict = () =>
	new ApiError(409, 'conflict', 'Запись изменена', {
		code: 'conflict',
		message: 'Запись изменена',
		fields: [],
		current_version: 0,
		permission: null,
		retry_after_secs: null
	});

const validation = (field: string, code: string) =>
	new ApiError(422, 'validation', 'Проверьте введённые данные', {
		code: 'validation',
		message: 'Проверьте введённые данные',
		fields: [{ field, code, message: '' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

/**
 * A server of one client: writes check the version like mw-server does and keep the phones as it does
 * (digits, trimmed notes, no empty rows). It counts writes in flight at once and the 409s.
 */
function fakeServer(start: Client) {
	let current = start;
	let inFlight = 0;
	let gate: Promise<void> = Promise.resolve();
	let open = () => {};
	const stats = { maxInFlight: 0, conflicts: 0 };
	const api = {
		getClient: vi.fn(async () => current),
		updateClient: vi.fn(async (_id: number, version: number, patch: ClientPatch) => {
			inFlight += 1;
			stats.maxInFlight = Math.max(stats.maxInFlight, inFlight);
			try {
				await gate;
				await Promise.resolve();
				if (version !== current.version) {
					stats.conflicts += 1;
					throw conflict();
				}
				const next = { ...current, ...patch } as Client;
				if (patch.phones) next.phones = savedPhones(patch.phones);
				if (typeof patch.note === 'string') next.note = patch.note.trim();
				current = { ...next, version: current.version + 1 };
				return current;
			} finally {
				inFlight -= 1;
			}
		})
	} satisfies ClientEditorApi;
	return {
		api,
		stats,
		/** Someone at another computer edits the client. */
		elsewhere(patch: Partial<Client>) {
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

const PHONES = [
	{ phone: '79211234567', note: '' },
	{ phone: '79001112233', note: 'раб' }
];

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
	vi.useRealTimers();
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('createClientEditor', () => {
	it('saves text a moment after the last key with the version, and tells the others', async () => {
		const server = fakeServer(client(5, { version: 3 }));
		const emit = vi.fn();
		const ed = createClientEditor(server.current, { api: server.api, emit });
		ed.autosave.edit('full_name', 'Тестова Анна Петровна');
		vi.advanceTimersByTime(599);
		expect(server.api.updateClient).not.toHaveBeenCalled();
		vi.advanceTimersByTime(1);
		await settle();
		expect(server.api.updateClient).toHaveBeenCalledWith(5, 3, {
			full_name: 'Тестова Анна Петровна'
		});
		expect(ed.client.version).toBe(4);
		expect(ed.client.full_name).toBe('Тестова Анна Петровна');
		expect(emit).toHaveBeenCalledWith(ed.client);

		// A switch goes at once, with the version the answer left.
		ed.autosave.commit('blacklisted', true);
		await settle();
		expect(server.api.updateClient).toHaveBeenLastCalledWith(5, 4, { blacklisted: true });
		ed.dispose();
	});

	it('the phones go as one list; empty rows are not sent and stay on screen until left', async () => {
		const server = fakeServer(client(5, { phones: PHONES }));
		const ed = createClientEditor(server.current, { api: server.api });
		const typed = [...PHONES, { phone: '79005554433', note: 'жена ' }, { phone: '', note: '' }];
		ed.autosave.commit('phones', typed);
		await settle();
		expect(server.api.updateClient).toHaveBeenCalledWith(5, 1, {
			phones: [...PHONES, { phone: '79005554433', note: 'жена' }]
		});
		expect(server.current.phones).toHaveLength(3);
		// The field in use keeps what was typed («жена » and the next word), then shows the server's.
		expect(ed.autosave.value('phones', ed.client.phones)).toEqual(typed);
		ed.autosave.release();
		expect(ed.autosave.value('phones', ed.client.phones)).toEqual(server.current.phones);
		ed.dispose();
	});

	it('typing in many phones, one request at a time, the last list wins', async () => {
		const server = fakeServer(client(5, { phones: PHONES }));
		const ed = createClientEditor(server.current, { api: server.api });
		server.hold();
		ed.autosave.commit('phones', [PHONES[1]!, PHONES[0]!]);
		await settle();
		ed.autosave.edit('phones', [{ ...PHONES[1]!, note: 'работа' }, PHONES[0]!]);
		ed.autosave.edit('address', 'Ленина, 1');
		vi.advanceTimersByTime(600);
		await settle();
		server.release();
		await ed.flush();
		expect(server.api.updateClient.mock.calls.map((c) => [c[1], c[2]])).toEqual([
			[1, { phones: [PHONES[1], PHONES[0]] }],
			[2, { phones: [{ ...PHONES[1], note: 'работа' }, PHONES[0]], address: 'Ленина, 1' }]
		]);
		expect(server.stats).toEqual({ maxInFlight: 1, conflicts: 0 });
		expect(server.current.phones[0]).toEqual({ phone: '79001112233', note: 'работа' });
		ed.dispose();
	});

	it('a conflict takes what others changed and asks about the fields both changed', async () => {
		const server = fakeServer(client(5, { phones: PHONES, address: '' }));
		const ed = createClientEditor(server.current, { api: server.api });
		server.elsewhere({ phones: [PHONES[0]!], address: 'Мира, 2', note: 'Постоянный' });
		ed.autosave.commit('phones', [...PHONES, { phone: '79005554433', note: 'жена' }]);
		ed.autosave.edit('full_name', 'Тестова Анна');
		await expect(ed.autosave.flush()).rejects.toMatchObject({ code: 'conflict' });
		await settle();

		expect(server.api.getClient).toHaveBeenCalledWith(5);
		expect([...ed.conflicts.keys()]).toEqual(['phones']);
		expect(ed.conflicts.get('phones')?.theirs).toEqual([PHONES[0]]);
		expect([...ed.flashed].sort()).toEqual(['address', 'note']);
		expect(ed.client.address).toBe('Мира, 2');
		// The name went with the new version; the phones wait for a choice.
		expect(server.api.updateClient).toHaveBeenLastCalledWith(5, 2, { full_name: 'Тестова Анна' });
		expect(ed.saveState).toBe('conflict');

		ed.keep('phones');
		await settle();
		expect(server.current.phones).toHaveLength(3);
		expect(ed.conflicts.size).toBe(0);
		expect(ed.saveState).toBe('saved');
		ed.dispose();
	});

	it('the same change on both sides is no conflict, empty rows aside', async () => {
		const server = fakeServer(client(5, { phones: PHONES }));
		const ed = createClientEditor(server.current, { api: server.api });
		const third = { phone: '79005554433', note: 'жена' };
		server.elsewhere({ phones: [...PHONES, third] });
		ed.autosave.commit('phones', [...PHONES, third, { phone: '', note: '' }]);
		await ed.flush();
		await settle();
		expect(ed.conflicts.size).toBe(0);
		expect(server.stats.conflicts).toBe(1);
		expect(server.api.updateClient).toHaveBeenCalledTimes(1);
		ed.dispose();
	});

	it('a client deleted meanwhile (merged into another) is not offered again', async () => {
		const server = fakeServer(client(5));
		const ed = createClientEditor(server.current, { api: server.api });
		server.api.updateClient.mockRejectedValue(
			new ApiError(404, 'not_found', 'Клиент не найден', null)
		);
		ed.autosave.edit('address', 'Ленина, 1');
		await expect(ed.flush()).resolves.toBe(false);
		expect(ed.lost).toBe('клиент удалён');
		ed.drop();
		await settle();
		expect(ed.unsaved).toBe(false);
		expect(ed.lost).toBeNull();
		ed.dispose();

		const other = fakeServer(client(6));
		const noRight = createClientEditor(other.current, { api: other.api });
		other.api.updateClient.mockRejectedValue(
			new ApiError(403, 'forbidden', 'Недостаточно прав: Изменение клиентов', null)
		);
		noRight.autosave.commit('blacklisted', true);
		await noRight.flush();
		expect(noRight.lost).toBe('Недостаточно прав: Изменение клиентов');
		noRight.dispose();
	});

	it('takes newer clients from elsewhere and ignores older ones', () => {
		const server = fakeServer(client(5, { version: 4 }));
		const ed = createClientEditor(server.current, { api: server.api });
		ed.receive(client(5, { version: 3, full_name: 'Старое' }));
		expect(ed.client.version).toBe(4);
		ed.receive(client(6, { version: 9 }));
		expect(ed.client.id).toBe(5);
		ed.receive(client(5, { version: 5, note: 'Новое' }));
		expect(ed.client.note).toBe('Новое');
		ed.dispose();
	});
});

describe('clientFieldErrors', () => {
	it('says what is wrong with the field the server named', () => {
		expect(clientFieldErrors(validation('phones', 'duplicate'))).toEqual({
			phones: 'Этот номер уже есть в списке'
		});
		expect(clientFieldErrors(validation('phones', 'out_of_range'))).toEqual({
			phones: 'Не больше 10 номеров'
		});
		expect(clientFieldErrors(validation('full_name', 'required'))).toEqual({
			full_name: 'Укажите ФИО или телефон'
		});
		expect(clientFieldErrors(conflict())).toEqual({});
		expect(clientFieldErrors(null)).toEqual({});
	});
});

describe('closeClientEditor', () => {
	it('saves what is left; a failure is told with the client', async () => {
		vi.useRealTimers();
		const server = fakeServer(client(5, { full_name: 'ТЕСТОВА АННА' }));
		const ed = createClientEditor(server.current, { api: server.api });
		server.api.updateClient.mockRejectedValueOnce(
			new ApiError(0, 'network', 'Нет связи с сервером', null)
		);
		ed.autosave.edit('address', 'Ленина, 1');
		closeClientEditor(ed);
		await settle();
		await new Promise((r) => setTimeout(r, 0));
		expect(toast.items.map((t) => [t.message, t.tone])).toEqual([
			['Изменения клиента «Тестова Анна» не сохранены: Нет связи с сервером', 'danger']
		]);

		const quiet = createClientEditor(server.current, { api: server.api });
		quiet.autosave.edit('address', 'Мира, 2');
		closeClientEditor(quiet);
		await settle();
		await new Promise((r) => setTimeout(r, 0));
		expect(server.current.address).toBe('Мира, 2');
		expect(toast.items).toHaveLength(1);
	});
});
