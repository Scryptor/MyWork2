import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { createAutosave, type Autosave, type SaveState } from './autosave.js';

interface Fields {
	malfunction: string;
	work_done: string;
	need_call: boolean;
}

interface Call {
	patch: Partial<Fields>;
	resolve: () => void;
	reject: (e: unknown) => void;
}

/** A save whose answers the test gives by hand. */
function fakeSave() {
	const calls: Call[] = [];
	const save = vi.fn(
		(patch: Partial<Fields>) =>
			new Promise<void>((resolve, reject) => calls.push({ patch, resolve, reject }))
	);
	return { calls, save };
}

/** Lets promise callbacks run without moving the fake clock. */
const settle = async () => {
	for (let i = 0; i < 5; i++) await Promise.resolve();
};

let a: Autosave<Fields>;
let calls: Call[];
let save: ReturnType<typeof fakeSave>['save'];
let states: SaveState[];

beforeEach(() => {
	vi.useFakeTimers();
	({ calls, save } = fakeSave());
	states = [];
	a = createAutosave<Fields>({ save, delayMs: 600, onstate: (s) => states.push(s) });
});

afterEach(() => {
	a.dispose();
	vi.useRealTimers();
});

describe('createAutosave', () => {
	it('debounces text and sends two fields in one patch', async () => {
		a.edit('malfunction', 'x');
		vi.advanceTimersByTime(300);
		a.edit('work_done', 'y');
		vi.advanceTimersByTime(599);
		expect(save).not.toHaveBeenCalled();
		expect(a.state).toBe('saving');
		vi.advanceTimersByTime(1);
		expect(save).toHaveBeenCalledTimes(1);
		expect(save).toHaveBeenCalledWith({ malfunction: 'x', work_done: 'y' });
		calls[0]!.resolve();
		await settle();
		expect(a.state).toBe('saved');
		expect(states).toEqual(['saving', 'saved']);
	});

	it('commit sends at once together with pending text', () => {
		a.edit('malfunction', 'не включается');
		a.commit('need_call', true);
		expect(save).toHaveBeenCalledTimes(1);
		expect(save).toHaveBeenCalledWith({ malfunction: 'не включается', need_call: true });
		// The text went with it: its timer sends nothing more.
		vi.advanceTimersByTime(600);
		expect(save).toHaveBeenCalledTimes(1);
	});

	it('never has two requests in flight', async () => {
		a.edit('malfunction', 'a');
		const flushed = a.flush();
		expect(save).toHaveBeenCalledTimes(1);
		a.edit('work_done', 'b');
		vi.advanceTimersByTime(600);
		expect(save).toHaveBeenCalledTimes(1);
		a.commit('need_call', true);
		expect(save).toHaveBeenCalledTimes(1);

		calls[0]!.resolve();
		await settle();
		// Everything typed while the first one was out goes in one patch.
		expect(save).toHaveBeenCalledTimes(2);
		expect(calls[1]!.patch).toEqual({ work_done: 'b', need_call: true });
		let done = false;
		void flushed.then(() => (done = true));
		await settle();
		expect(done).toBe(false);
		calls[1]!.resolve();
		await settle();
		expect(done).toBe(true);
		expect(a.state).toBe('saved');
	});

	it('stale response keeps newer local text', async () => {
		a.edit('malfunction', 'не за');
		vi.advanceTimersByTime(600); // request 1 is out
		expect(a.value('malfunction', 'старое')).toBe('не за');
		a.edit('malfunction', 'не заряжается'); // typed later
		calls[0]!.resolve();
		await settle();
		expect(a.value('malfunction', 'не за')).toBe('не заряжается');
		expect(a.state).toBe('saving');

		vi.advanceTimersByTime(600);
		expect(calls[1]!.patch).toEqual({ malfunction: 'не заряжается' });
		calls[1]!.resolve();
		await settle();
		// Saved: the field follows the server again.
		expect(a.value('malfunction', 'не заряжается')).toBe('не заряжается');
		expect(a.value('malfunction', 'изменили')).toBe('изменили');
		expect(a.pending).toEqual({});
	});

	it('failure keeps changes and retry resends', async () => {
		a.edit('malfunction', 'x');
		const flushed = a.flush();
		const offline = new ApiError(0, 'network', 'Нет связи с сервером', null);
		calls[0]!.reject(offline);
		await expect(flushed).rejects.toBe(offline);
		expect(a.state).toBe('error');
		expect(a.error).toBe(offline);
		expect(a.value('malfunction', 'old')).toBe('x');
		expect(a.pending).toEqual({ malfunction: 'x' });

		a.retry();
		expect(save).toHaveBeenCalledTimes(2);
		expect(calls[1]!.patch).toEqual({ malfunction: 'x' });
		expect(a.state).toBe('saving');
		expect(a.error).toBeNull();
		calls[1]!.resolve();
		await settle();
		expect(a.state).toBe('saved');
		expect(a.pending).toEqual({});
	});

	it('a failed request comes back under newer edits of the same field', async () => {
		a.edit('malfunction', 'первое');
		void a.flush().catch(() => {});
		a.edit('malfunction', 'второе');
		a.edit('work_done', 'работа');
		calls[0]!.reject(new ApiError(500, 'internal', 'Сбой', null));
		await settle();
		expect(a.pending).toEqual({ malfunction: 'второе', work_done: 'работа' });
		a.retry();
		expect(calls[1]!.patch).toEqual({ malfunction: 'второе', work_done: 'работа' });
	});

	it('a conflict rejects flush and keeps the edits', async () => {
		a.edit('work_done', 'замена');
		const flushed = a.flush();
		const conflict = new ApiError(409, 'conflict', 'Изменено', {
			code: 'conflict',
			message: 'Изменено',
			fields: [],
			current_version: 7,
			permission: null,
			retry_after_secs: null
		});
		calls[0]!.reject(conflict);
		await expect(flushed).rejects.toBe(conflict);
		expect(a.error?.code).toBe('conflict');
		expect(a.pending).toEqual({ work_done: 'замена' });
		// Nothing is sent again by itself.
		vi.advanceTimersByTime(5000);
		expect(save).toHaveBeenCalledTimes(1);
	});

	it('flush with nothing to save resolves at once', async () => {
		await expect(a.flush()).resolves.toBeUndefined();
		expect(save).not.toHaveBeenCalled();
		expect(a.state).toBe('idle');
	});

	it('discard drops a change that has not gone yet', async () => {
		a.edit('malfunction', 'x');
		a.edit('work_done', 'y');
		a.discard('malfunction');
		expect(a.value('malfunction', 'server')).toBe('server');
		vi.advanceTimersByTime(600);
		expect(calls[0]!.patch).toEqual({ work_done: 'y' });
		calls[0]!.resolve();
		await settle();
		a.edit('need_call', true);
		a.discard('need_call');
		expect(a.state).toBe('saved');
		vi.advanceTimersByTime(600);
		expect(save).toHaveBeenCalledTimes(1);
	});

	it('retry with nothing left clears the error', async () => {
		a.edit('malfunction', 'x');
		void a.flush().catch(() => {});
		calls[0]!.reject(new ApiError(0, 'network', 'Нет связи с сервером', null));
		await settle();
		a.discard('malfunction');
		expect(a.state).toBe('error');
		a.retry();
		expect(a.state).toBe('idle');
		expect(a.error).toBeNull();
		expect(save).toHaveBeenCalledTimes(1);
	});

	it('a field dropped while its save is out does not come back when that save fails', async () => {
		a.edit('malfunction', 'x');
		a.edit('work_done', 'y');
		const flushed = a.flush();
		expect(calls).toHaveLength(1);
		a.discard('malfunction');
		// What is dropped is no longer own: the field shows the server's value, nothing waits for it.
		expect(a.value('malfunction', 'с сервера')).toBe('с сервера');
		expect(a.pending).toEqual({ work_done: 'y' });
		calls[0]!.reject(new ApiError(0, 'network', 'Нет связи с сервером', null));
		await expect(flushed).rejects.toMatchObject({ code: 'network' });
		expect(a.pending).toEqual({ work_done: 'y' });
		expect(a.state).toBe('error');

		// Everything of a failed save dropped: nothing failed that is still wanted.
		const again = a.flush();
		a.discard('work_done');
		calls[1]!.reject(new ApiError(0, 'network', 'Нет связи с сервером', null));
		await expect(again).resolves.toBeUndefined();
		expect(a.pending).toEqual({});
		expect(a.error).toBeNull();
		expect(a.state).toBe('idle');
	});

	it('a save the server only trimmed keeps the typed text until the field is left', async () => {
		a.edit('malfunction', 'Замена ');
		vi.advanceTimersByTime(600);
		calls[0]!.resolve();
		await settle();
		// The server keeps «Замена»: the field still shows the space typed, the next word goes after it.
		expect(a.value('malfunction', 'Замена')).toBe('Замена ');
		expect(a.state).toBe('saved');
		// Something else on the server is not a trimmed copy: the server's.
		expect(a.value('malfunction', 'Другое')).toBe('Другое');
		a.release();
		expect(a.value('malfunction', 'Замена')).toBe('Замена');
	});

	it('a trailing newline survives the save while the field is in use', async () => {
		a.edit('work_done', 'Чистка\n');
		vi.advanceTimersByTime(600);
		calls[0]!.resolve();
		await settle();
		expect(a.value('work_done', 'Чистка')).toBe('Чистка\n');
		a.edit('work_done', 'Чистка\nЗамена');
		expect(a.value('work_done', 'Чистка')).toBe('Чистка\nЗамена');
	});

	it('leaving the field while its save is out shows the server text after the answer', async () => {
		a.edit('malfunction', 'Замена ');
		void a.flush();
		a.release();
		calls[0]!.resolve();
		await settle();
		expect(a.value('malfunction', 'Замена')).toBe('Замена');
	});

	it('a value the server keeps in its own form stays as sent until release, when told so', async () => {
		interface Rows {
			rows: string[];
		}
		const sent: Partial<Rows>[] = [];
		const rows = createAutosave<Rows>({
			delayMs: 600,
			save: async (patch) => void sent.push(patch),
			// The server drops empty rows.
			keeps: (field, typed, server) =>
				field === 'rows' &&
				JSON.stringify((typed as string[]).filter(Boolean)) === JSON.stringify(server)
		});
		rows.commit('rows', ['a', '']);
		await settle();
		expect(sent).toEqual([{ rows: ['a', ''] }]);
		expect(rows.value('rows', ['a'])).toEqual(['a', '']);
		// Another list on the server is not the sent one in its form: the server's.
		expect(rows.value('rows', ['b'])).toEqual(['b']);
		rows.release();
		expect(rows.value('rows', ['a'])).toEqual(['a']);
		rows.dispose();
	});

	it('dispose stops the timer', () => {
		a.edit('malfunction', 'x');
		a.dispose();
		vi.advanceTimersByTime(600);
		expect(save).not.toHaveBeenCalled();
	});
});
