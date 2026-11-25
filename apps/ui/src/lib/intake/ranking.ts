import type { CatalogUsage } from '$api/CatalogUsage';
import type { DictionaryItem } from '$api/DictionaryItem';

/**
 * The values of a dictionary, the ones most used in orders first; equal counts (the unused ones too)
 * keep the order of the dictionary. Without counts (not loaded, refused) — the order of the dictionary.
 */
export function rankByUsage(
	items: readonly Pick<DictionaryItem, 'id' | 'value'>[],
	usage: readonly CatalogUsage[]
): string[] {
	const counts = new Map(usage.map((u) => [u.id, u.count]));
	const count = (id: number) => counts.get(id) ?? 0;
	// Array.prototype.sort is stable: equal counts stay in the order given.
	return [...items].sort((a, b) => count(b.id) - count(a.id)).map((i) => i.value);
}

type Item = Pick<DictionaryItem, 'id' | 'value'>;

/**
 * Loads dictionaries for the fields of a form. `show` gets every list as soon as the lists come, in the
 * order of each dictionary (a list that failed is empty), and then each of `ranked` again, the most used
 * first, once its counts come: the fields never wait for the counts. Counts refused, failed or empty leave
 * the order of the dictionary. The caller drops what comes for an older opening.
 */
export async function loadDictionaries<K extends string>(
	kinds: readonly K[],
	ranked: readonly K[],
	api: {
		list: (kind: K) => Promise<readonly Item[]>;
		usage: (kind: K) => Promise<readonly CatalogUsage[]>;
	},
	show: (lists: Partial<Record<K, string[]>>) => void
): Promise<void> {
	// Asked together with the lists; a refusal is no counts.
	const counts = ranked.map((k) => api.usage(k).catch(() => []));
	const items = await Promise.all(kinds.map((k) => api.list(k).catch((): Item[] => [])));
	const itemsOf = (k: K) => items[kinds.indexOf(k)] ?? [];
	type Lists = Partial<Record<K, string[]>>;
	show(Object.fromEntries(kinds.map((k) => [k, itemsOf(k).map((i) => i.value)])) as Lists);
	await Promise.all(
		ranked.map(async (k, i) => {
			const usage = await counts[i]!;
			if (usage.length > 0) show({ [k]: rankByUsage(itemsOf(k), usage) } as Lists);
		})
	);
}
