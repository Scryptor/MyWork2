// One dictionary of «Настройки → Справочники»: its values (archived ones too), how many orders use each,
// and the actions on them. Every action answers with the text of a mistake or `null`; the screen shows it.
import type { CatalogUsage } from '$api/CatalogUsage';
import type { DictionaryItem } from '$api/DictionaryItem';
import type { DictionaryKind } from '$api/DictionaryKind';
import {
	addDictionaryItem,
	deleteDictionaryItem,
	dictionaryUsage,
	listDictionary,
	mergeDictionary,
	renameDictionaryItem,
	reorderDictionary,
	updateDictionaryItem
} from '#lib/api/catalogs.js';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';
import { movedId, putOrder } from './order.js';

export interface CatalogApi {
	list: typeof listDictionary;
	usage: typeof dictionaryUsage;
	add: typeof addDictionaryItem;
	update: typeof updateDictionaryItem;
	remove: typeof deleteDictionaryItem;
	reorder: typeof reorderDictionary;
	rename: typeof renameDictionaryItem;
	merge: typeof mergeDictionary;
}

const realApi: CatalogApi = {
	list: listDictionary,
	usage: dictionaryUsage,
	add: addDictionaryItem,
	update: updateDictionaryItem,
	remove: deleteDictionaryItem,
	reorder: reorderDictionary,
	rename: renameDictionaryItem,
	merge: mergeDictionary
};

export interface CatalogRow {
	id: number;
	label: string;
	archived: boolean;
	/** How many records use it; `null` — not known (the counts could not be read, or need a right). */
	count: number | null;
}

/** The counts by id from the answer of the server; a failed reading leaves them unknown (`null`). */
export function usageCounts(usage: CatalogUsage[] | null): Record<number, number> | null {
	return usage === null ? null : Object.fromEntries(usage.map((u) => [u.id, u.count]));
}

export interface Rewritten {
	error: string | null;
	changed: number;
}

/** What is typed in the search box matches without case and without the difference of «е» and «ё». */
export const fold = (s: string): string => s.trim().toLowerCase().replaceAll('ё', 'е');

/** The visible ones, in their new order, take the places the visible ones had; the hidden stay put. */
export function mergeOrder(full: number[], visibleOrdered: number[]): number[] {
	let next = 0;
	return full.map((id) => (visibleOrdered.includes(id) ? visibleOrdered[next++]! : id));
}

function mistake(e: unknown): string {
	if (e instanceof ApiError && e.body) {
		for (const f of e.body.fields) {
			// `value` of a rename, `target_id` of a merge: the orders would get another warranty term.
			if (f.code === 'changes_term') return ru.dictionaries.changesTerm;
			if (f.field !== 'value') continue;
			if (f.code === 'duplicate') return ru.dictionaries.duplicate;
			if (f.code === 'required') return ru.settings.required;
			if (f.code === 'comma') return ru.dictionaries.comma;
		}
	}
	return errorMessage(e);
}

/** `withCounts`: whether the counts may be asked for — the server gives them only to «settings_manage». */
export function createCatalog(api: CatalogApi = realApi, withCounts: () => boolean = () => true) {
	let kind = $state<DictionaryKind>('device_type');
	let items = $state.raw<DictionaryItem[]>([]);
	let counts = $state.raw<Record<number, number> | null>({});
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let query = $state('');
	let seq = 0;

	const sorted = (list: DictionaryItem[]) =>
		[...list].sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const row = (i: DictionaryItem): CatalogRow => ({
		id: i.id,
		label: i.value,
		archived: i.archived,
		count: counts === null ? null : (counts[i.id] ?? 0)
	});
	const matches = (i: DictionaryItem) => !query.trim() || fold(i.value).includes(fold(query));

	async function read(k: DictionaryKind, mine: number) {
		const [list, usage] = await Promise.all([
			api.list(k, true),
			withCounts() ? api.usage(k).catch(() => null) : null
		]);
		if (mine !== seq) return;
		items = sorted(list);
		counts = usageCounts(usage);
		loadError = null;
	}

	/** Reads another dictionary; an older answer that comes late is dropped. */
	async function load(k: DictionaryKind): Promise<void> {
		const mine = ++seq;
		if (k !== kind) {
			items = [];
			counts = {};
			query = '';
		}
		kind = k;
		loading = true;
		try {
			await read(k, mine);
		} catch (e) {
			if (mine === seq) loadError = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}

	/** Read again after a change: the order of the server and the counts. */
	async function refresh(): Promise<void> {
		const mine = ++seq;
		try {
			await read(kind, mine);
		} catch (e) {
			if (mine === seq) loadError = errorMessage(e);
		}
	}

	async function act(fn: () => Promise<unknown>): Promise<string | null> {
		try {
			await fn();
		} catch (e) {
			void refresh();
			return mistake(e);
		}
		await refresh();
		return null;
	}

	async function rewrite(fn: () => Promise<{ records_changed: number }>): Promise<Rewritten> {
		try {
			const { records_changed: changed } = await fn();
			await refresh();
			return { error: null, changed };
		} catch (e) {
			return { error: mistake(e), changed: 0 };
		}
	}

	return {
		get kind() {
			return kind;
		},
		get loading() {
			return loading;
		},
		get loadError() {
			return loadError;
		},
		get query() {
			return query;
		},
		set query(v: string) {
			query = v;
		},
		/** Not archived values that match the search, in order. */
		get active(): CatalogRow[] {
			return items.filter((i) => !i.archived && matches(i)).map(row);
		},
		get archived(): CatalogRow[] {
			return items.filter((i) => i.archived && matches(i)).map(row);
		},
		/** All the values, whatever the search says (for the choice of a merge). */
		get all(): CatalogRow[] {
			return items.map(row);
		},
		load,
		refresh,
		add: (value: string) => act(() => api.add(kind, value)),
		rename: (id: number, value: string) => act(() => api.update(id, { value })),
		setArchived: (id: number, archived: boolean) => act(() => api.update(id, { archived })),
		remove: (id: number) => act(() => api.remove(id)),
		renameEverywhere: (id: number, value: string, inOrders: boolean) =>
			rewrite(() => api.rename(id, { value, in_orders: inOrders })),
		merge: (targetId: number, sourceIds: number[], inOrders: boolean) =>
			rewrite(() =>
				api.merge(kind, { target_id: targetId, source_ids: sourceIds, in_orders: inOrders })
			),
		/** The shown order at once; the old one comes back when the server refuses. */
		async reorder(visibleIds: number[]): Promise<string | null> {
			const before = items;
			const activeIds = before.filter((i) => !i.archived).map((i) => i.id);
			const fullActive = mergeOrder(activeIds, visibleIds);
			const rest = before.filter((i) => i.archived).map((i) => i.id);
			const order = [...fullActive, ...rest];
			items = sorted(before.map((i) => ({ ...i, sort_order: order.indexOf(i.id) })));
			try {
				const saved = await putOrder({
					ordered: order,
					seen: before.map((i) => i.id),
					moved: movedId(activeIds, fullActive),
					put: (ids, seen) => api.reorder(kind, ids, seen),
					fresh: async () => sorted(await api.list(kind, true)).map((i) => i.id)
				});
				if (saved !== order) await refresh();
				return null;
			} catch (e) {
				items = before;
				return errorMessage(e);
			}
		}
	};
}

export type Catalog = ReturnType<typeof createCatalog>;
