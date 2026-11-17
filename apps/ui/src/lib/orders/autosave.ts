import { createSubscriber } from 'svelte/reactivity';
import { ApiError } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

export type SaveState = 'idle' | 'saving' | 'saved' | 'error';

/**
 * Saving of a record as it is edited (spec 6.2): text a moment after the last key, choices at once, one
 * request at a time. Reads (`state`, `error`, `pending`, `value`) are reactive in components and effects.
 */
export interface Autosave<T extends object> {
	readonly state: SaveState;
	readonly error: ApiError | null;
	/** Every change not saved yet, waiting or in flight: the newest value of each field. */
	readonly pending: Partial<T>;
	/** Текст: уйдёт через delayMs после последней правки или при flush(). */
	edit<K extends keyof T>(field: K, value: T[K]): void;
	/** Выбор и переключатели: уходит сразу, вместе с ждущими текстовыми правками. */
	commit<K extends keyof T>(field: K, value: T[K]): void;
	/** Sends what waits now; resolves once everything is saved, rejects with the error of a failed save. */
	flush(): Promise<void>;
	/** Sends the changes a failed save left; with none left the error goes. */
	retry(): void;
	/** Значение, которое должно быть в поле: своё несохранённое или с сервера. */
	value<K extends keyof T>(field: K, server: T[K]): T[K];
	/** Drops the unsent changes of these fields: a conflict decided, an amount that is not one. */
	discard(...fields: (keyof T)[]): void;
	/**
	 * The fields are left. Until then a text the server saved only trimmed («Замена » → «Замена») stays
	 * as typed, so the next word does not stick to it; after it the field shows the server's text.
	 */
	release(): void;
	dispose(): void;
}

interface Waiter {
	resolve: () => void;
	reject: (e: ApiError) => void;
}

function asApiError(e: unknown): ApiError {
	return e instanceof ApiError ? e : new ApiError(0, 'internal', ru.api.failed, null);
}

export function createAutosave<T extends object>(opts: {
	save: (patch: Partial<T>) => Promise<void>;
	delayMs: number; // 600
	onstate?: (s: SaveState) => void;
}): Autosave<T> {
	/** Changes not sent yet. */
	let waiting: Partial<T> = {};
	/** The patch of the request in flight. */
	let sending: Partial<T> | null = null;
	/** Texts as they were sent, kept for the field in use (see `release`). */
	let typed: Partial<T> = {};
	let state: SaveState = 'idle';
	let error: ApiError | null = null;
	let savedOnce = false;
	let timer: ReturnType<typeof setTimeout> | undefined;
	/** What waits should go as soon as no request is in flight. */
	let due = false;
	let disposed = false;
	let waiters: Waiter[] = [];

	let notify = () => {};
	const subscribe = createSubscriber((update) => {
		notify = update;
		return () => (notify = () => {});
	});

	const isEmpty = (p: Partial<T>) => Object.keys(p).length === 0;

	function refresh() {
		const busy = sending !== null || (!isEmpty(waiting) && error === null);
		const next: SaveState = busy ? 'saving' : error ? 'error' : savedOnce ? 'saved' : 'idle';
		if (next !== state) {
			state = next;
			if (!disposed) opts.onstate?.(next);
		}
		notify();
	}

	function stopTimer() {
		clearTimeout(timer);
		timer = undefined;
	}

	function pump() {
		if (disposed || sending !== null || isEmpty(waiting)) return;
		if (!due && waiters.length === 0) return;
		stopTimer();
		due = false;
		const patch = waiting;
		waiting = {};
		sending = patch;
		for (const [k, v] of Object.entries(patch))
			if (typeof v === 'string') typed = { ...typed, [k]: v };
		error = null;
		refresh();
		opts.save(patch).then(
			() => {
				sending = null;
				savedOnce = true;
				pump();
				if (sending === null && isEmpty(waiting)) {
					const done = waiters;
					waiters = [];
					for (const w of done) w.resolve();
				}
				refresh();
			},
			(e: unknown) => {
				// Edits made meanwhile are newer than the failed ones.
				waiting = { ...patch, ...waiting };
				sending = null;
				error = asApiError(e);
				const failed = waiters;
				waiters = [];
				refresh();
				for (const w of failed) w.reject(error);
			}
		);
	}

	function put<K extends keyof T>(field: K, value: T[K]) {
		waiting = { ...waiting, [field]: value };
	}

	return {
		get state() {
			subscribe();
			return state;
		},
		get error() {
			subscribe();
			return error;
		},
		get pending() {
			subscribe();
			return { ...sending, ...waiting };
		},
		edit(field, value) {
			if (disposed) return;
			put(field, value);
			stopTimer();
			timer = setTimeout(() => {
				timer = undefined;
				due = true;
				pump();
			}, opts.delayMs);
			refresh();
		},
		commit(field, value) {
			if (disposed) return;
			put(field, value);
			due = true;
			pump();
			refresh();
		},
		flush() {
			if (disposed || (sending === null && isEmpty(waiting))) return Promise.resolve();
			return new Promise<void>((resolve, reject) => {
				waiters.push({ resolve, reject });
				pump();
			});
		},
		retry() {
			if (disposed) return;
			if (sending === null && isEmpty(waiting)) error = null;
			due = true;
			pump();
			refresh();
		},
		value(field, server) {
			subscribe();
			if (field in waiting) return waiting[field] as T[typeof field];
			if (sending && field in sending) return sending[field] as T[typeof field];
			const own = typed[field];
			if (typeof own === 'string' && typeof server === 'string' && own.trim() === server) {
				return own as T[typeof field];
			}
			return server;
		},
		discard(...fields) {
			const next = { ...waiting };
			for (const f of fields) delete next[f];
			waiting = next;
			if (isEmpty(waiting)) stopTimer();
			refresh();
		},
		release() {
			if (Object.keys(typed).length === 0) return;
			typed = {};
			notify();
		},
		dispose() {
			disposed = true;
			stopTimer();
			const left = waiters;
			waiters = [];
			for (const w of left) w.resolve();
		}
	};
}
