// What a kind of status means and what a change of it does to the orders in the status.
import type { StatusKind } from '$api/StatusKind';
import { countLabel, plural } from '#lib/format/index.js';
import { ru } from '#lib/i18n/ru.js';

const t = ru.statuses;

export const KIND_HINTS: Record<StatusKind, string> = t.kindHints;

/** The kinds in the order of the choice. */
export const STATUS_KINDS = Object.keys(t.kindNames) as StatusKind[];

/**
 * The warning before the kind of a status is changed, or `null` when no order is in it.
 * «Выдан» and «Готов» matter most: reports, warranty and the ready lists follow the kind.
 */
export function kindChangeWarning(from: StatusKind, to: StatusKind, count: number): string | null {
	if (count <= 0 || from === to) return null;
	const what =
		to === 'issued'
			? t.warnToIssued
			: from === 'issued'
				? t.warnFromIssued
				: to === 'ready'
					? t.warnToReady
					: t.warnChange(t.kindNames[to]);
	return `${countLabel(count, t.orders)} ${plural(count, what)}`;
}
