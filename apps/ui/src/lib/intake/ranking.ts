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
