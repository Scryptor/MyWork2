// Editing one client on the server (spec 6.6): the client's fields over the shared editor — one queue of
// writes per client, text a moment after the last key, choices at once, a 409 merged with the newer
// version and the disputed fields asked about, as in an order.
import type { Client } from '$api/Client';
import type { ClientPatch } from '$api/ClientPatch';
import { ApiError } from '#lib/api/client.js';
import { getClient, updateClient } from '#lib/api/clients.js';
import {
	createEntityEditor,
	type Conflict,
	type EntityEditor
} from '#lib/app/entity-editor.svelte.js';
import { displayName } from '#lib/format/name.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { keptPhones, savedPhones } from './phones.js';

const t = ru.client;

/** The fields of the client panel. */
export const CLIENT_FIELDS = [
	'full_name',
	'phones',
	'address',
	'note',
	'source',
	'blacklisted'
] as const satisfies readonly (keyof Client & keyof ClientPatch)[];

export type ClientField = (typeof CLIENT_FIELDS)[number];
export type ClientValues = Pick<Client, ClientField>;
export type ClientConflict = Conflict<ClientValues[ClientField]>;

export interface ClientEditorApi {
	getClient: (id: number) => Promise<Client>;
	updateClient: (id: number, version: number, patch: ClientPatch) => Promise<Client>;
}

export interface ClientEditorOptions {
	api?: ClientEditorApi;
	/** Every answer of the server goes here: the list catches up. */
	emit?: (c: Client) => void;
	delayMs?: number;
}

/** The shared editor of one client; `client` is its record. */
export interface ClientEditor extends EntityEditor<Client, ClientField> {
	/** The client as the server last gave it. */
	readonly client: Client;
}

const DEFAULT_API: ClientEditorApi = { getClient, updateClient };

/** A save no retry can help: the client is gone (merged into another), or editing is not allowed. */
function lostText(e: ApiError | null): string | null {
	if (e?.code === 'not_found') return ru.save.clientDeleted;
	if (e?.code === 'forbidden') return e.message;
	return null;
}

export function createClientEditor(initial: Client, opts: ClientEditorOptions = {}): ClientEditor {
	const api = opts.api ?? DEFAULT_API;
	const ed = createEntityEditor<Client, ClientField>({
		initial,
		fields: CLIENT_FIELDS,
		load: () => api.getClient(initial.id),
		save: (patch, version) => api.updateClient(initial.id, version, patch),
		emit: opts.emit,
		lostText,
		// The phones go as the server keeps them; the field shows them as typed until it is left.
		prepare: (patch) => (patch.phones ? { ...patch, phones: savedPhones(patch.phones) } : patch),
		keeps: (field, typed, server) =>
			field === 'phones'
				? keptPhones(typed, server)
				: typeof typed === 'string' && typeof server === 'string' && typed.trim() === server,
		delayMs: opts.delayMs
	});
	return Object.defineProperty(ed, 'client', {
		get: () => ed.record,
		enumerable: true
	}) as ClientEditor;
}

/** What is wrong with a field, as the server said it (a 422): shown under the field. */
export function clientFieldErrors(e: ApiError | null): Partial<Record<ClientField, string>> {
	if (e?.code !== 'validation') return {};
	const out: Partial<Record<ClientField, string>> = {};
	for (const f of e.body?.fields ?? []) {
		if (f.field === 'phones') {
			out.phones = t.phoneErrors[f.code as keyof typeof t.phoneErrors] ?? e.message;
		} else if (f.field === 'full_name') out.full_name = t.nameRequired;
	}
	return out;
}

/** The client's name for texts: as the screen shows it, or «Без имени». */
export function clientTitle(c: Pick<Client, 'full_name'>): string {
	return displayName(c.full_name) || t.noName;
}

/** Why the client's own changes are not saved: the failed save, or a conflict waiting for a choice. */
export function clientUnsavedText(ed: ClientEditor): string {
	const reason =
		ed.conflicts.size > 0
			? t.conflictTitle
			: (ed.lost ?? ed.autosave.error?.message ?? ru.api.failed);
	return t.unsaved(clientTitle(ed.client), reason);
}

/**
 * An editor that is no longer shown (another client, the panel closed): what is left is saved; a failure
 * or an undecided conflict is told; then it stops.
 */
export function closeClientEditor(ed: ClientEditor): void {
	void ed
		.flush()
		.then((ok) => {
			if (!ok) toast.show({ message: clientUnsavedText(ed), tone: 'danger' });
		})
		.finally(() => ed.dispose());
}
