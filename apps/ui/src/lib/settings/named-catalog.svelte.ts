// Statuses and service points of «Настройки»: the same list — rows with the count of orders, the actions on
// them, a drag order. The list is read with the archived ones; after every change the program's own copy of
// the catalogs is read again (`changed`), so the filters and the status picker follow at once.
import type { CatalogUsage } from '$api/CatalogUsage';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';
import { mergeOrder, usageCounts, type CatalogRow } from './catalog.svelte.js';
import { movedId, putOrder } from './order.js';

export interface NamedItem {
	id: number;
	name: string;
	sort_order: number;
	archived: boolean;
}

export interface NamedApi<T extends NamedItem, N, P> {
	list: () => Promise<T[]>;
	usage: () => Promise<CatalogUsage[]>;
	add: (value: N) => Promise<T>;
	update: (id: number, patch: P) => Promise<T>;
	/** `seenIds`: the order the new one was made from. */
	reorder: (orderedIds: number[], seenIds?: number[]) => Promise<void>;
}

/** The text of a refusal: the known codes of the server in words, anything else as the server says. */
export function mistake(e: unknown): string {
	if (e instanceof ApiError && e.body) {
		for (const f of e.body.fields) {
			if (f.code === 'last_issued_status') return ru.statuses.lastIssued;
			if (f.field === 'name' && f.code === 'duplicate') return ru.statuses.duplicate;
			if (f.field === 'name' && f.code === 'required') return ru.settings.required;
		}
	}
	return errorMessage(e);
}

export function createNamedCatalog<T extends NamedItem, N, P>(
	api: NamedApi<T, N, P>,
	changed: () => Promise<unknown> | void = () => {}
) {
	let items = $state.raw<T[]>([]);
	let counts = $state.raw<Record<number, number> | null>({});
	let loading = $state(true);
	let loadError = $state<string | null>(null);
	let seq = 0;

	const sorted = (list: T[]) =>
		[...list].sort((a, b) => a.sort_order - b.sort_order || a.id - b.id);
	const row = (i: T): CatalogRow => ({
		id: i.id,
		label: i.name,
		archived: i.archived,
		count: counts === null ? null : (counts[i.id] ?? 0)
	});

	async function read(mine: number) {
		const [list, usage] = await Promise.all([api.list(), api.usage().catch(() => null)]);
		if (mine !== seq) return;
		items = sorted(list);
		counts = usageCounts(usage);
		loadError = null;
	}

	/** The first reading; a late older answer is dropped. */
	async function load(): Promise<void> {
		const mine = ++seq;
		loading = true;
		try {
			await read(mine);
		} catch (e) {
			if (mine === seq) loadError = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}

	async function refresh(): Promise<void> {
		const mine = ++seq;
		try {
			await read(mine);
		} catch (e) {
			if (mine === seq) loadError = errorMessage(e);
		}
	}

	async function tell() {
		try {
			await changed();
		} catch {
			// The program's copy is read again on the next start; the list here is already right.
		}
	}

	/** Runs a change; answers the text of the mistake or `null`. */
	async function act(fn: () => Promise<unknown>): Promise<string | null> {
		try {
			await fn();
		} catch (e) {
			void refresh();
			return mistake(e);
		}
		await Promise.all([refresh(), tell()]);
		return null;
	}

	return {
		get loading() {
			return loading;
		},
		get loadError() {
			return loadError;
		},
		get items(): T[] {
			return items;
		},
		get active(): CatalogRow[] {
			return items.filter((i) => !i.archived).map(row);
		},
		get archived(): CatalogRow[] {
			return items.filter((i) => i.archived).map(row);
		},
		get rows(): CatalogRow[] {
			return [...items.filter((i) => !i.archived), ...items.filter((i) => i.archived)].map(row);
		},
		/** `null` when the counts are not known. */
		countOf: (id: number): number | null => (counts === null ? null : (counts[id] ?? 0)),
		load,
		refresh,
		act,
		add: (value: N) => act(() => api.add(value)),
		update: (id: number, patch: P) => act(() => api.update(id, patch)),
		/** The shown order at once; the old one comes back when the server refuses. */
		async reorder(visibleIds: number[]): Promise<string | null> {
			const before = items;
			const activeIds = before.filter((i) => !i.archived).map((i) => i.id);
			const order = [
				...mergeOrder(activeIds, visibleIds),
				...before.filter((i) => i.archived).map((i) => i.id)
			];
			items = sorted(before.map((i) => ({ ...i, sort_order: order.indexOf(i.id) })));
			try {
				const saved = await putOrder({
					ordered: order,
					seen: before.map((i) => i.id),
					moved: movedId(activeIds, mergeOrder(activeIds, visibleIds)),
					put: api.reorder,
					fresh: async () => sorted(await api.list()).map((i) => i.id)
				});
				if (saved !== order) await refresh();
			} catch (e) {
				items = before;
				return errorMessage(e);
			}
			await tell();
			return null;
		}
	};
}

export type NamedCatalog<T extends NamedItem, N, P> = ReturnType<
	typeof createNamedCatalog<T, N, P>
>;
