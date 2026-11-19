// Editing one record on the server (spec 6.2): autosave with the record's version, the conflict when it
// was changed elsewhere, server actions on the newest version. Orders and clients wrap it with their
// own fields and requests; each open record gets one editor.
/* eslint-disable svelte/prefer-svelte-reactivity -- maps and sets here are replaced whole in $state.raw */
import { ApiError } from '#lib/api/client.js';
import { trackQueue } from '#lib/app/session-reset.js';
import { createAutosave, type Autosave, type SaveState } from '#lib/orders/autosave.js';
import { mergeRemote, sameValue } from '#lib/orders/merge.js';

/** Text a moment after the last key (spec 6.2). */
export const SAVE_DELAY_MS = 600;

/** What every edited record has: its number and the version a write is checked against. */
export interface Versioned {
	id: number;
	version: number;
}

/** A field changed here and elsewhere: own value and the one on the server now. */
export interface Conflict<V = unknown> {
	mine: V;
	theirs: V;
}

/** `conflict`: own changes wait for «Оставить моё» or «Взять новое». */
export type EditorSaveState = SaveState | 'conflict';

export interface EntityEditorOptions<T extends Versioned, F extends keyof T> {
	/** The record as the server gave it. */
	initial: T;
	/** The fields edited here; the others are only shown. */
	fields: readonly F[];
	/** The record as it is on the server now (after a conflict). */
	load: () => Promise<T>;
	/** Writes the changes over this version; the answer is the record after it. */
	save: (patch: Partial<Pick<T, F>>, version: number) => Promise<T>;
	/** Every answer of the server goes here: lists and the summary catch up. */
	emit?: (r: T) => void;
	/**
	 * Why the server will not take own changes however often they are sent (the record was deleted,
	 * the right to edit was taken away); null when a retry may help. Without it: no right to edit.
	 */
	lostText?: (e: ApiError | null) => string | null;
	/** The changes as they go to the server (and as they are compared with it); as they are by default. */
	prepare?: (patch: Partial<Pick<T, F>>) => Partial<Pick<T, F>>;
	/** A value sent that the server kept in its own form: the field shows it as typed until `release`. */
	keeps?: (field: F, typed: unknown, server: unknown) => boolean;
	delayMs?: number;
}

/** What the save indicator of a panel reads and offers. */
export interface SaveStatus {
	readonly saveState: EditorSaveState;
	readonly lost: string | null;
	readonly autosave: { readonly error: ApiError | null; retry(): void };
	drop(): void;
}

export interface EntityEditor<T extends Versioned, F extends keyof T> extends SaveStatus {
	/** The record as the server last gave it. */
	readonly record: T;
	/** What fields show and change: own unsaved values first, the disputed ones as own until decided. */
	readonly autosave: Autosave<Pick<T, F>>;
	readonly conflicts: ReadonlyMap<F, Conflict<T[F]>>;
	/** Fields just taken from another computer: highlighted for a moment. */
	readonly flashed: ReadonlySet<F>;
	readonly saveState: EditorSaveState;
	/** Own changes not on the server: waiting, being sent, failed, or disputed. */
	readonly unsaved: boolean;
	/**
	 * Why the server will not take own changes however often they are sent (see `lostText`); null
	 * otherwise. Only `drop` lets such a record go. It holds while they are sent again (leaving a field),
	 * until a save succeeds.
	 */
	readonly lost: string | null;
	/** The record as the fields show it now. */
	readonly current: T;
	/** «Оставить моё»: own value goes over the newer version. */
	keep(field: F): void;
	/** «Взять новое»: the field shows what came from the server. */
	take(field: F): void;
	/** «Отбросить мои правки»: own unsaved and disputed values go, the fields show the server's record. */
	drop(): void;
	endFlash(field: F): void;
	/** A newer version of this record from elsewhere in the program (an undo, a bulk action). */
	receive(r: T): void;
	/** Saves everything; false when a save failed or a conflict waits for a choice. */
	flush(): Promise<boolean>;
	/**
	 * Before the record is left: saves everything. A dispute still undecided (a field changed here and
	 * elsewhere) is put to `ask`: true drops own changes and lets go, false stays. Without a dispute a
	 * failed save stays too. Resolves to whether the record may be left.
	 */
	leave(ask: () => Promise<boolean>): Promise<boolean>;
	/** A server action on the newest version: what is typed goes first; one retry after a conflict. */
	run(action: (version: number) => Promise<T>): Promise<T>;
	dispose(): void;
}

export const isConflict = (e: unknown) => e instanceof ApiError && e.code === 'conflict';

const noRight = (e: ApiError | null) => (e?.code === 'forbidden' ? e.message : null);

export function createEntityEditor<T extends Versioned, F extends keyof T>(
	opts: EntityEditorOptions<T, F>
): EntityEditor<T, F> {
	type Values = Pick<T, F>;
	const fields = opts.fields;
	const emit = opts.emit ?? (() => {});
	const lostText = opts.lostText ?? noRight;
	const prepare = opts.prepare ?? ((p: Partial<Values>) => p);
	let record = $state.raw(opts.initial);
	let conflicts = $state.raw<ReadonlyMap<F, Conflict<T[F]>>>(new Map());
	let flashed = $state.raw<ReadonlySet<F>>(new Set());
	let lost = $state<string | null>(null);
	let resolving: Promise<void> | null = null;
	let again = false;
	let tail: Promise<unknown> = Promise.resolve();

	const valuesOf = (r: T) => Object.fromEntries(fields.map((f) => [f, r[f]])) as Values;

	/**
	 * Every write of this record goes one after another, saves and actions alike: each one is sent with
	 * the version the previous answer left, and after a conflict only once it is resolved.
	 */
	function serial<R>(write: () => Promise<R>): Promise<R> {
		const next = tail.then(async () => {
			if (resolving) await resolving;
			return write();
		});
		tail = next.catch(() => {});
		return next;
	}

	/** An answer of the server; one older than the record shown changes nothing. */
	function apply(r: T): T {
		if (r.id !== record.id || r.version < record.version) return r;
		record = r;
		emit(r);
		return r;
	}

	const inner = createAutosave<Values>({
		delayMs: opts.delayMs ?? SAVE_DELAY_MS,
		keeps: opts.keeps as
			((field: keyof Values, typed: unknown, server: unknown) => boolean) | undefined,
		save: (patch) =>
			serial(async () => {
				// A field that became disputed while this save waited is not sent over the other computer.
				const send = Object.fromEntries(
					Object.entries(patch).filter(([f]) => !conflicts.has(f as F))
				) as Partial<Values>;
				if (Object.keys(send).length === 0) return;
				apply(await opts.save(prepare(send), record.version));
			}),
		onstate: (s) => {
			if (s === 'error') lost = lostText(inner.error);
			else if (s !== 'saving') lost = null;
			if (s === 'error' && isConflict(inner.error)) void resolveConflict();
		}
	});

	/** Own new text in a disputed field decides it: it is own. */
	function settle(field: F) {
		if (!conflicts.has(field)) return;
		const next = new Map(conflicts);
		next.delete(field);
		conflicts = next;
	}

	/** The record as it is on the server now, against own unsaved changes. */
	function mergeIn(remote: T) {
		const mine = Object.fromEntries([...conflicts].map(([f, c]) => [f, c.mine]));
		const local = prepare({ ...mine, ...inner.pending } as Partial<Values>);
		const r = mergeRemote(valuesOf(record), local, valuesOf(remote));
		const next = new Map<F, Conflict<T[F]>>();
		for (const f of r.conflicts) next.set(f, { mine: local[f]!, theirs: remote[f] });
		for (const [f, c] of conflicts) {
			if (!next.has(f) && !sameValue(c.mine, remote[f])) next.set(f, { ...c, theirs: remote[f] });
		}
		const alike = (Object.keys(local) as F[]).filter(
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
					mergeIn(await opts.load());
				} while (again);
			} catch {
				// No answer: «Не сохранено · Повторить» stays, «Повторить» tries again.
			} finally {
				resolving = null;
			}
		})();
		return resolving;
	}

	const autosave: Autosave<Values> = {
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
		discard: (...fs) => inner.discard(...fs),
		release: () => inner.release(),
		dispose: () => inner.dispose()
	};

	const editor: EntityEditor<T, F> = {
		get record() {
			return record;
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
		get unsaved() {
			return conflicts.size > 0 || Object.keys(inner.pending).length > 0;
		},
		get lost() {
			return lost;
		},
		get current() {
			const shown = Object.fromEntries(fields.map((f) => [f, autosave.value(f, record[f])]));
			return { ...record, ...shown };
		},
		keep(field) {
			const c = conflicts.get(field);
			if (!c) return;
			settle(field);
			inner.commit(field, c.mine as Values[F]);
		},
		take(field) {
			settle(field);
			if (conflicts.size === 0) inner.retry();
		},
		drop() {
			lost = null;
			conflicts = new Map();
			inner.discard(...fields);
			// Nothing left to send: the error goes with it.
			inner.retry();
		},
		endFlash(field) {
			if (!flashed.has(field)) return;
			const next = new Set(flashed);
			next.delete(field);
			flashed = next;
		},
		receive(r) {
			if (r.id === record.id && r.version > record.version) record = r;
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
		async leave(ask) {
			if (await this.flush()) return true;
			if (conflicts.size === 0 || !(await ask())) return false;
			this.drop();
			return true;
		},
		async run(action) {
			// What was typed goes first; a failed save shows in the indicator, the action is still tried.
			await inner.flush().catch(() => {});
			return serial(async () => {
				try {
					return apply(await action(record.version));
				} catch (e) {
					if (!isConflict(e)) throw e;
					await resolveConflict();
					return apply(await action(record.version));
				}
			});
		},
		dispose() {
			untrack();
			inner.dispose();
		}
	};
	// Signing out waits for this editor's changes, or gives them up.
	const untrack = trackQueue({
		unsaved: () => editor.unsaved,
		flush: () => editor.flush(),
		discard: () => editor.drop()
	});
	return editor;
}
