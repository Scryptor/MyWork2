// The state of «Настройки → Резервные копии»: the overview read from the server, the progress of a copy
// (read again every second while it runs), the actions, and the settings saved as they are edited.
import { untrack } from 'svelte';
import type { BackupOverview } from '$api/BackupOverview';
import type { BackupSettings } from '$api/BackupSettings';
import type { BackupSettingsPatch } from '$api/BackupSettingsPatch';
import {
	cancelRestore,
	chooseLegacyBackup,
	deleteBackup,
	getBackups,
	requestRestore,
	startBackup,
	updateBackupSettings
} from '#lib/api/backups.js';
import { ApiError, errorMessage, errorReason } from '#lib/api/client.js';
import { trackQueue } from '#lib/app/session-reset.js';
import { ru } from '#lib/i18n/ru.js';
import { createAutosave, type SaveState } from '#lib/orders/autosave.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

const t = ru.backups;

/** How often the overview is read again while a copy is being made. */
export const POLL_MS = 1000;

type Field = keyof BackupSettingsPatch;

const INTEGER = /^\d+$/;
const KEEP_MIN = 1;
const KEEP_MAX = 100;

export interface BackupsScreen {
	/** The overview; null until the first answer. */
	readonly overview: BackupOverview | null;
	/** Instead of the screen: why there is none (no right, a refusal of the server, no answer). */
	readonly blocked: string | null;
	/** A request of «Сделать копию сейчас» is out, or a copy is running. */
	readonly making: boolean;
	/** The name of a copy being restored or deleted: one action at a time. */
	readonly working: boolean;
	readonly saveState: SaveState;
	/** The failed save as a whole (no answer), not the mistake of a field. */
	readonly saveError: ApiError | null;
	/** What a field shows: its own unsaved value first. */
	auto(): boolean;
	keepText(): string;
	folderText(): string;
	fieldError(field: Field): string | undefined;
	setAuto(on: boolean): void;
	typeKeep(raw: string): void;
	typeFolder(raw: string): void;
	/** The field is left: what was typed goes, the server's value is shown again. */
	leave(field: 'keep' | 'folder'): void;
	/** A folder chosen in a dialog or «По умолчанию» (`null`): saved at once. */
	setFolder(folder: string | null): void;
	retrySave(): void;
	reload(): Promise<void>;
	makeCopy(): Promise<void>;
	restore(name: string): Promise<void>;
	remove(name: string): Promise<void>;
	/** «Отменить восстановление» and «Скрыть»: the same call. */
	dismissRestore(): Promise<void>;
	chooseLegacy(apply: boolean): Promise<void>;
	dispose(): void;
}

export function createBackupsScreen(opts: {
	/** `settings_manage`: without it the server is not asked. */
	allowed: boolean;
	delayMs?: number;
}): BackupsScreen {
	let overview = $state.raw<BackupOverview | null>(null);
	let blocked = $state<string | null>(opts.allowed ? null : t.rights);
	let starting = $state(false);
	let working = $state(false);
	let errors = $state<Record<string, string>>({});
	/** The text of a field as typed, while it is in use. */
	let drafts = $state<Record<string, string>>({});
	let timer: ReturnType<typeof setTimeout> | undefined;
	let disposed = false;
	/** A copy seen running: when it is over the screen says so once. */
	let watched = false;

	const running = (o: BackupOverview | null) => !!o?.job && o.job.finished_at === null;

	function setError(field: string, text: string | undefined) {
		const next = { ...errors };
		if (text === undefined) delete next[field];
		else next[field] = text;
		errors = next;
	}

	function schedule() {
		clearTimeout(timer);
		timer = undefined;
		if (disposed || !running(overview)) return;
		timer = setTimeout(() => void reload(), POLL_MS);
	}

	async function reload() {
		try {
			const next = await getBackups();
			if (disposed) return;
			const was = watched;
			overview = next;
			blocked = next.supported ? null : (next.reason ?? t.rights);
			watched = running(next);
			if (was && !watched && next.job && !next.job.error)
				toast.show({ message: t.made, tone: 'success' });
		} catch (e) {
			if (disposed) return;
			// A failed reading of a running copy is tried again; any other one is the answer of the screen.
			if (!running(overview)) blocked = errorMessage(e);
		}
		schedule();
	}

	if (opts.allowed) void reload();

	const inner = createAutosave<BackupSettingsPatch>({
		delayMs: opts.delayMs ?? 600,
		save: async (patch) => {
			try {
				await send(patch);
			} catch (e) {
				if (!(e instanceof ApiError) || e.code !== 'validation' || !e.body) throw e;
				const bad = e.body.fields.map((f) => f.field as Field);
				for (const f of e.body.fields)
					setError(f.field, f.field === 'keep' ? t.keepRange : f.message);
				// The server takes a patch whole or not at all: what is right goes again without the wrong.
				const rest = Object.fromEntries(
					Object.entries(patch).filter(([f]) => !bad.includes(f as Field))
				) as BackupSettingsPatch;
				if (Object.keys(rest).length > 0) {
					await send(rest);
					inner.discard(...(Object.keys(rest) as Field[]));
				}
				inner.discard(...bad);
				throw e;
			}
		}
	});

	async function send(patch: BackupSettingsPatch) {
		const saved: BackupSettings = await updateBackupSettings(patch);
		for (const f of Object.keys(patch)) setError(f, undefined);
		if (overview) overview = { ...overview, settings: saved };
		// The folder decides the list and the path shown.
		if ('folder' in patch) await reload();
	}

	const untrackQueue = trackQueue({
		unsaved: () => Object.keys(inner.pending).length > 0,
		flush: async () => {
			try {
				await inner.flush();
			} catch {
				return false;
			}
			return Object.keys(inner.pending).length === 0;
		},
		discard: () => inner.discard(...(Object.keys(inner.pending) as Field[]))
	});

	/** An action of the buttons: one at a time, its refusal in a toast. */
	async function act(run: () => Promise<void>): Promise<void> {
		if (working) return;
		working = true;
		try {
			await run();
		} catch (e) {
			toast.show({ message: errorReason(e), tone: 'danger' });
			throw e;
		} finally {
			working = false;
		}
	}

	return {
		get overview() {
			return overview;
		},
		get blocked() {
			return blocked;
		},
		get making() {
			return starting || running(overview);
		},
		get working() {
			return working;
		},
		get saveState() {
			return inner.state;
		},
		get saveError() {
			const e = inner.error;
			return e && e.code !== 'validation' ? e : null;
		},
		auto: () => inner.value('auto', overview?.settings.auto ?? false) ?? false,
		keepText: () => drafts.keep ?? String(inner.value('keep', overview?.settings.keep ?? 0)),
		folderText: () =>
			drafts.folder ??
			inner.value('folder', overview?.settings.folder ?? null) ??
			overview?.folder ??
			'',
		fieldError: (field) => errors[field],
		setAuto(on) {
			setError('auto', undefined);
			inner.commit('auto', on);
		},
		typeKeep(raw) {
			drafts = { ...drafts, keep: raw };
			setError('keep', undefined);
			const text = raw.trim();
			const n = Number(text);
			if (INTEGER.test(text) && n >= KEEP_MIN && n <= KEEP_MAX) inner.edit('keep', n);
			else {
				inner.discard('keep');
				setError('keep', t.keepRange);
			}
		},
		typeFolder(raw) {
			drafts = { ...drafts, folder: raw };
			setError('folder', undefined);
			// An empty path is the default folder.
			inner.edit('folder', raw.trim() === '' ? null : raw.trim());
		},
		leave(field) {
			if (field in drafts) {
				const next = { ...drafts };
				delete next[field];
				drafts = next;
			}
			void inner.flush().catch(() => {});
			// A mistake of what was typed is gone with the draft; the server's own stays under the field.
			if (errors[field] === t.keepRange && field === 'keep') setError('keep', undefined);
		},
		setFolder(folder) {
			setError('folder', undefined);
			inner.commit('folder', folder);
		},
		retrySave: () => inner.retry(),
		reload,
		async makeCopy() {
			if (starting || running(overview)) return;
			starting = true;
			try {
				const job = await startBackup();
				watched = true;
				if (overview) overview = { ...overview, job };
				await reload();
			} catch (e) {
				toast.show({ message: errorReason(e), tone: 'danger' });
			} finally {
				starting = false;
			}
		},
		restore: (name) =>
			act(async () => {
				const pending = await requestRestore(name);
				if (overview) overview = { ...overview, pending, last_restore: null };
				await reload();
			}),
		remove: (name) =>
			act(async () => {
				await deleteBackup(name);
				await reload();
			}),
		dismissRestore: () =>
			act(async () => {
				await cancelRestore();
				if (overview) overview = { ...overview, pending: null, last_restore: null };
				await reload();
			}).catch(() => {}),
		chooseLegacy: (apply) =>
			act(async () => {
				const before = overview;
				const saved = await chooseLegacyBackup(apply);
				const first = before?.legacy?.folders[0];
				if (apply && saved.folder === before?.settings.folder && first !== undefined)
					toast.show({ message: t.legacyMissing(first) });
				await reload();
			}).catch(() => {}),
		dispose() {
			disposed = true;
			clearTimeout(timer);
			untrackQueue();
			untrack(() => inner.dispose());
		}
	};
}
