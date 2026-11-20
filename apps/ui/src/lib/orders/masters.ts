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
		const name = known?.display_name ?? currentName;
		items.push({
			value: current,
			label: name ? ru.fields.masterDisabled(name) : '',
			disabled: true
		});
	}
	return items;
}

/** The select's value of a master: `null` is «Не выбран». */
export const masterValue = (id: number | null): number => id ?? NO_MASTER;

/** The master of the select's value: «Не выбран» is `null`. */
export const masterOf = (value: number | null): number | null =>
	value === null || value === NO_MASTER ? null : value;

/** The masters as a screen has them: the list, `null` while it is read, `'failed'` when it was not. */
export type MasterList = readonly MasterUser[] | null | 'failed';

/** The name of the master `id` in the list; `fallback` when it is not there (or not read). */
export function masterName(masters: MasterList, id: number | null, fallback = ''): string {
	if (id === null) return '';
	if (!Array.isArray(masters)) return fallback;
	return masters.find((m) => m.id === id)?.display_name ?? fallback;
}

/**
 * The chosen master before the masters are known: whether it is still active is not known, so it is
 * never «(отключён)» — its name (`name`, as an order keeps it), else «Загрузка…», or «Мастер № 4» after a
 * failure.
 */
export function pendingMaster(current: number, failed: boolean, name = ''): MasterItem {
	const label = name || (failed ? ru.fields.masterNumber(current) : ru.fields.masterLoading);
	return { value: current, label, disabled: true };
}

/** «Не выбран» and the chosen master (`pendingMaster`) while the masters load, or when loading failed. */
export function pendingMasterItems(
	current: number | null,
	failed: boolean,
	name = ''
): MasterItem[] {
	const items: MasterItem[] = [{ value: NO_MASTER, label: ru.fields.noMaster }];
	if (current !== null) items.push(pendingMaster(current, failed, name));
	return items;
}

/** The items of a select of masters: `pendingMasterItems` until the masters are read, then `masterItems`. */
export function masterChoices(
	masters: MasterList,
	current: number | null = null,
	currentName = ''
): MasterItem[] {
	return masters === null || masters === 'failed'
		? pendingMasterItems(current, masters === 'failed', currentName)
		: masterItems(masters, current, currentName);
}

/**
 * The masters of «Фильтры»: every one, the disabled marked «(отключён)» (an old order may have them). Before
 * they are read only the chosen one (`chosen`), as `pendingMaster`: the select is not blank meanwhile.
 */
export function filterMasterItems(masters: MasterList, chosen: number | undefined): MasterItem[] {
	if (!Array.isArray(masters)) {
		return chosen === undefined ? [] : [pendingMaster(chosen, masters === 'failed')];
	}
	return masters.map((m) => ({
		value: m.id,
		label: m.active ? m.display_name : ru.fields.masterDisabled(m.display_name)
	}));
}
