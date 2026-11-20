// The master of an order is a user marked «Мастер» (plan 5.8): a select of them, with «Не выбран».
import type { MasterUser } from '$api/MasterUser';
import { ru } from '#lib/i18n/ru.js';

/** «Не выбран» in a select of masters: selects take numbers, an order takes `null`. */
export const NO_MASTER = 0;

export interface MasterItem {
	value: number;
	label: string;
	disabled?: boolean;
}

/**
 * «Не выбран» and the active masters. The chosen one that is disabled (or not in the list) stays as a
 * disabled item named `currentName`: an older order keeps it, a new one cannot take it.
 */
export function masterItems(
	masters: readonly MasterUser[],
	current: number | null = null,
	currentName = ''
): MasterItem[] {
	const items: MasterItem[] = [{ value: NO_MASTER, label: ru.fields.noMaster }];
	for (const m of masters) if (m.active) items.push({ value: m.id, label: m.display_name });
	if (current !== null && !items.some((i) => i.value === current)) {
		const known = masters.find((m) => m.id === current);
		items.push({ value: current, label: known?.display_name ?? currentName, disabled: true });
	}
	return items;
}

/** The select's value of a master: `null` is «Не выбран». */
export const masterValue = (id: number | null): number => id ?? NO_MASTER;

/** The master of the select's value: «Не выбран» is `null`. */
export const masterOf = (value: number | null): number | null =>
	value === null || value === NO_MASTER ? null : value;

/** The name of the master `id` in the list; `fallback` when it is not there. */
export function masterName(
	masters: readonly MasterUser[],
	id: number | null,
	fallback = ''
): string {
	if (id === null) return '';
	return masters.find((m) => m.id === id)?.display_name ?? fallback;
}
