// Saving the settings of «Общие» and «Заказы» as they are edited (spec 6.8): one queue of changes
// per screen, one PATCH at a time, an error of the server under its field. The settings have no
// version, so the plain `createAutosave` is enough.
import type { AppSettings } from '$api/AppSettings';
import type { SettingsPatch } from '$api/SettingsPatch';
import { ApiError } from '#lib/api/client.js';
import { trackQueue } from '#lib/app/session-reset.js';
import { ru } from '#lib/i18n/ru.js';
import { createAutosave, type SaveState } from '#lib/orders/autosave.js';

const t = ru.settings;

export type SettingsField = keyof SettingsPatch;

/** Fields kept as a whole number: what is typed stays in `drafts` until the field is left. */
const INTEGER = /^\d+$/;

/** The field of a patch a path of the server belongs to: `firm_links.2.url` is `firm_links`. */
const fieldOfPath = (path: string) => path.split('.')[0];

/** What the server meant by a code in the list of links: the path says the row and the field. */
function linkMessage(path: string, code: string, fallback: string): string {
	const part = path.split('.')[2];
	if (code === 'too_long') return part === 'name' ? t.linkNameTooLong : t.linkTooLong;
	if (code === 'duplicate') return part === 'name' ? t.linkNameTaken : fallback;
	if (code === 'too_many') return t.linkTooMany;
	return fallback;
}

/** What the server meant by a code in a field, in words of this field. */
function fieldMessage(field: string, code: string, fallback: string): string {
	if (code === 'required') return t.required;
	if (code === 'out_of_range') {
		if (field === 'master_percent') return t.range(0, 100);
		if (field === 'diagnostics_overdue_days' || field === 'ready_stale_days')
			return t.range(1, 365);
		if (field === 'currency_symbol') return t.currencyLong;
	}
	if (code === 'not_a_link') return t.notALink;
	if (fieldOfPath(field) === 'firm_links') return linkMessage(field, code, fallback);
	if (code === 'invalid') {
		if (field === 'timezone') return t.unknownTimezone;
		if (field === 'default_service_point_id') return t.unknownPoint;
	}
	return fallback;
}

export interface SettingsEditor {
	/** Without `settings_manage` the screen only shows the settings. */
	readonly canEdit: boolean;
	readonly state: SaveState;
	/** The failed save as a whole (no answer, no right), not a field's mistake. */
	readonly error: ApiError | null;
	/** The value a field shows: own unsaved one first, then the server's. */
	value<K extends SettingsField>(
		field: K,
		server: NonNullable<AppSettings[K]> | AppSettings[K]
	): SettingsPatch[K];
	/** Text: saved a moment after the last key. */
	edit<K extends SettingsField>(field: K, value: SettingsPatch[K]): void;
	/** Text that is saved only on Enter or when the field is left (`commitTyped`), not after a pause. */
	type<K extends SettingsField>(field: K, value: SettingsPatch[K]): void;
	/** Enter or leaving the field: what was typed there goes into the queue. */
	commitTyped(field: SettingsField): void;
	/** A choice or a switch: saved at once. */
	commit<K extends SettingsField>(field: K, value: SettingsPatch[K]): void;
	/** A whole number typed into a field; anything else is shown as a mistake and not sent. */
	editNumber(field: SettingsField, raw: string): void;
	/** The text of a number field: as typed while it is in use. */
	draft(field: SettingsField, server: number): string;
	/** The field is left: its draft goes, and the mistake that was only about the draft with it. */
	leave(field: SettingsField): void;
	/** A field or a path of the server inside it (`firm_links.2.url`). */
	fieldError(field: string): string | undefined;
	/** Some fields have a mistake (of the server or of what was typed), and it is not a failure of the whole save. */
	readonly fieldsFailed: boolean;
	pendingFields(): string[];
	flush(): Promise<void>;
	retry(): void;
	dispose(): void;
}

export function createSettingsEditor(opts: {
	can: () => boolean;
	save: (patch: SettingsPatch) => Promise<AppSettings>;
	/** Called after every answer: the catalogs of the program catch up. */
	refresh: () => Promise<void>;
	delayMs?: number;
}): SettingsEditor {
	let errors = $state<Record<string, string>>({});
	let drafts = $state<Record<string, string>>({});
	/** Text typed but not yet handed to the queue. */
	let typed = $state<Record<string, SettingsPatch[SettingsField]>>({});
	/** Mistakes of what was typed that the server never saw. */
	let local = $state<Record<string, true>>({});

	const setError = (field: string, text: string | undefined) => {
		const next = { ...errors };
		if (text === undefined) delete next[field];
		else next[field] = text;
		errors = next;
	};
	const clearLocal = (field: string) => {
		if (!local[field]) return;
		const next = { ...local };
		delete next[field];
		local = next;
		setError(field, undefined);
	};

	async function send(patch: SettingsPatch) {
		await opts.save(patch);
		for (const f of Object.keys(patch)) setError(f, undefined);
		try {
			await opts.refresh();
		} catch {
			// The answer was saved; the lists catch up with the next read.
		}
	}

	const inner = createAutosave<SettingsPatch>({
		delayMs: opts.delayMs ?? 600,
		save: async (patch) => {
			try {
				await send(patch);
			} catch (e) {
				if (!(e instanceof ApiError) || e.code !== 'validation' || !e.body) throw e;
				const bad = e.body.fields.map((f) => fieldOfPath(f.field));
				for (const f of e.body.fields) setError(f.field, fieldMessage(f.field, f.code, f.message));
				// The server takes a patch whole or not at all: what is right goes again without the wrong.
				const rest = Object.fromEntries(
					Object.entries(patch).filter(([f]) => !bad.includes(f))
				) as SettingsPatch;
				const restFields = Object.keys(rest) as SettingsField[];
				if (restFields.length > 0) {
					await send(rest);
					inner.discard(...restFields);
				}
				throw e;
			}
		}
	});

	const unset = (field: SettingsField) => {
		clearLocal(field);
		// The mistakes of a list sit under the paths of its rows.
		const rest = Object.fromEntries(
			Object.entries(errors).filter(([path]) => fieldOfPath(path) !== field)
		);
		if (Object.keys(rest).length !== Object.keys(errors).length) errors = rest;
	};

	/** Text typed and not yet left goes into the queue too: signing out sends it. */
	function commitAllTyped() {
		const all = Object.entries(typed) as [SettingsField, SettingsPatch[SettingsField]][];
		if (all.length === 0) return;
		typed = {};
		for (const [field, value] of all) inner.commit(field, value as never);
	}

	// Signing out sends what waits (an edit in its pause) under this user's token, or asks about it.
	const untrack = trackQueue({
		unsaved: () => Object.keys(inner.pending).length > 0 || Object.keys(typed).length > 0,
		flush: async () => {
			commitAllTyped();
			try {
				await inner.flush();
			} catch {
				return false;
			}
			return Object.keys(inner.pending).length === 0;
		},
		discard: () => {
			typed = {};
			inner.discard(...(Object.keys(inner.pending) as SettingsField[]));
		}
	});

	return {
		get canEdit() {
			return opts.can();
		},
		get state() {
			return inner.state;
		},
		get error() {
			const e = inner.error;
			return e && e.code !== 'validation' ? e : null;
		},
		value: (field, server) =>
			(field in typed
				? typed[field]
				: inner.value(field, server as SettingsPatch[typeof field])) as SettingsPatch[typeof field],
		type(field, value) {
			if (!opts.can()) return;
			unset(field);
			typed = { ...typed, [field]: value };
		},
		commitTyped(field) {
			if (!(field in typed) || !opts.can()) return;
			const value = typed[field];
			const next = { ...typed };
			delete next[field];
			typed = next;
			inner.commit(field, value as never);
		},
		get fieldsFailed() {
			const e = inner.error;
			return Object.keys(errors).length > 0 && (e === null || e.code === 'validation');
		},
		edit(field, value) {
			if (!opts.can()) return;
			unset(field);
			inner.edit(field, value);
		},
		commit(field, value) {
			if (!opts.can()) return;
			unset(field);
			if (field in typed) {
				const rest = { ...typed };
				delete rest[field];
				typed = rest;
			}
			inner.commit(field, value);
		},
		editNumber(field, raw) {
			if (!opts.can()) return;
			drafts = { ...drafts, [field]: raw };
			unset(field);
			const text = raw.trim();
			if (INTEGER.test(text)) {
				inner.edit(field, Number(text) as never);
				return;
			}
			inner.discard(field);
			local = { ...local, [field]: true };
			setError(field, text === '' ? t.required : t.notNumber);
		},
		draft: (field, server) => drafts[field] ?? String(inner.value(field, server as never)),
		leave(field) {
			if (field in drafts) {
				const next = { ...drafts };
				delete next[field];
				drafts = next;
			}
			clearLocal(field);
		},
		fieldError: (field) => errors[field],
		pendingFields: () => Object.keys(inner.pending),
		flush: () => inner.flush().catch(() => {}),
		retry: () => inner.retry(),
		dispose: () => {
			untrack();
			inner.dispose();
		}
	};
}
