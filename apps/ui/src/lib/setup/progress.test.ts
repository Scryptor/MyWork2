import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { ImportJob } from '$api/ImportJob';
import { ApiError } from '#lib/api/client.js';
import { followImport, jobState, stageSteps, type Followed } from './progress.js';

const job = (over: Partial<ImportJob> = {}): ImportJob => ({
	stage: 'reading',
	percent: 0,
	error: null,
	import_id: null,
	...over
});
const refused = (status: number, message: string) =>
	new ApiError(status, status === 401 ? 'unauthenticated' : 'forbidden', message, null);

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('jobState', () => {
	it('done only with the number of the import, which the server gives with «done»', () => {
		expect(jobState(job({ stage: 'done', percent: 100, import_id: 4 }))).toEqual({
			kind: 'done',
			importId: 4
		});
		expect(jobState(job({ stage: 'copying', percent: 40 }))).toMatchObject({ kind: 'running' });
	});

	it('an error is a failure; when the base was written its number comes with it', () => {
		expect(jobState(job({ stage: 'reading', error: 'Папка не найдена' }))).toEqual({
			kind: 'failed',
			message: 'Папка не найдена',
			importId: null
		});
		expect(
			jobState(job({ stage: 'placing', error: 'Файлы не поставлены на место', import_id: 2 }))
		).toEqual({ kind: 'failed', message: 'Файлы не поставлены на место', importId: 2 });
	});
});

describe('followImport', () => {
	it('asks once a second until the import is done, then stops', async () => {
		const answers = [
			job({ stage: 'copying', percent: 40 }),
			job({ stage: 'writing', percent: 0 }),
			job({ stage: 'done', percent: 100, import_id: 3 })
		];
		const current = vi.fn(async () => answers.shift() ?? null);
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		// The start already answered with the job: the first question waits a second.
		expect(current).not.toHaveBeenCalled();
		await vi.advanceTimersByTimeAsync(1000);
		expect(current).toHaveBeenCalledTimes(1);
		expect(states.at(-1)).toEqual({
			kind: 'running',
			job: job({ stage: 'copying', percent: 40 }),
			offline: false
		});
		await vi.advanceTimersByTimeAsync(2000);
		expect(states.at(-1)).toEqual({ kind: 'done', importId: 3 });
		await vi.advanceTimersByTimeAsync(5000);
		expect(current).toHaveBeenCalledTimes(3);
	});

	it('a failure stops it with the reason', async () => {
		const current = vi.fn(async () => job({ stage: 'writing', error: 'База не пуста' }));
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(3000);
		expect(current).toHaveBeenCalledTimes(1);
		expect(states).toEqual([{ kind: 'failed', message: 'База не пуста', importId: null }]);
	});

	it('401: the written base has users now, and one of them signs in to see the report', async () => {
		const current = vi.fn(async () => {
			throw refused(401, 'Войдите');
		});
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(3000);
		expect(current).toHaveBeenCalledTimes(1);
		expect(states).toEqual([{ kind: 'signin' }]);
	});

	it('403: the reason in the words of the server', async () => {
		const current = vi.fn(async () => {
			throw refused(403, 'Недостаточно прав');
		});
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(1000);
		expect(states).toEqual([{ kind: 'forbidden', message: 'Недостаточно прав' }]);
	});

	it('no connection for a while: it keeps asking and says so, then goes on', async () => {
		const answers: (ImportJob | 'offline')[] = [
			job({ stage: 'copying', percent: 10 }),
			'offline',
			job({ stage: 'copying', percent: 70 })
		];
		const current = vi.fn(async () => {
			const next = answers.shift();
			if (next === 'offline') throw new ApiError(0, 'network', 'Нет связи с сервером', null);
			return next ?? null;
		});
		const states: Followed[] = [];
		const stop = followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(3000);
		expect(states).toEqual([
			{ kind: 'running', job: job({ stage: 'copying', percent: 10 }), offline: false },
			{ kind: 'running', job: job({ stage: 'copying', percent: 10 }), offline: true },
			{ kind: 'running', job: job({ stage: 'copying', percent: 70 }), offline: false }
		]);
		stop();
	});

	it('another refusal stops it with its text', async () => {
		const current = vi.fn(async () => {
			throw new ApiError(500, 'internal', 'Сбой сервера', null);
		});
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(1000);
		expect(states).toEqual([{ kind: 'failed', message: 'Сбой сервера', importId: null }]);
	});

	it('null: the server knows of no import (it was restarted)', async () => {
		const current = vi.fn(async () => null);
		const states: Followed[] = [];
		followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(3000);
		expect(current).toHaveBeenCalledTimes(1);
		expect(states).toEqual([{ kind: 'gone' }]);
	});

	it('the next question waits for the answer to the previous one', async () => {
		let release!: (j: ImportJob) => void;
		const current = vi.fn(
			() =>
				new Promise<ImportJob>((resolve) => {
					release = resolve;
				})
		);
		const stop = followImport(current, () => {});
		await vi.advanceTimersByTimeAsync(5000);
		expect(current).toHaveBeenCalledTimes(1);
		release(job({ stage: 'copying' }));
		await vi.advanceTimersByTimeAsync(1000);
		expect(current).toHaveBeenCalledTimes(2);
		stop();
	});

	it('after stop nothing is asked and an answer on its way is not reported', async () => {
		let release!: (j: ImportJob) => void;
		const current = vi.fn(
			() =>
				new Promise<ImportJob>((resolve) => {
					release = resolve;
				})
		);
		const states: Followed[] = [];
		const stop = followImport(current, (s) => states.push(s));
		await vi.advanceTimersByTimeAsync(1000);
		stop();
		release(job({ stage: 'copying' }));
		await vi.advanceTimersByTimeAsync(5000);
		expect(current).toHaveBeenCalledTimes(1);
		expect(states).toEqual([]);
	});
});

describe('stageSteps', () => {
	it('the stages before are done, the current one has its percent, the rest wait', () => {
		expect(stageSteps({ stage: 'copying', percent: 45 })).toEqual([
			{ stage: 'reading', label: 'Чтение базы', state: 'done', percent: 100 },
			{ stage: 'converting', label: 'Разбор', state: 'done', percent: 100 },
			{ stage: 'copying', label: 'Копирование файлов', state: 'current', percent: 45 },
			{ stage: 'writing', label: 'Запись', state: 'pending', percent: 0 },
			{ stage: 'placing', label: 'Установка файлов', state: 'pending', percent: 0 }
		]);
	});

	it('«done» has every stage done', () => {
		expect(stageSteps({ stage: 'done', percent: 100 }).map((s) => s.state)).toEqual([
			'done',
			'done',
			'done',
			'done',
			'done'
		]);
	});

	it('the base is known to be written: everything to «Запись» is done', () => {
		expect(
			stageSteps({ stage: 'reading', percent: 0 }, { written: true }).map((s) => s.state)
		).toEqual(['done', 'done', 'done', 'done', 'pending']);
	});

	it('a failure marks the stage it stopped at', () => {
		expect(stageSteps({ stage: 'converting', percent: 0 }, { failed: true })[1]).toMatchObject({
			stage: 'converting',
			state: 'failed'
		});
	});
});
