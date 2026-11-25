// The brief of an earlier order in the client's card: on hover a card; without hover (a phone, a touch
// screen) a second line of the row.
import type { OrderListItem } from '$api/OrderListItem';
import type { Status } from '$api/Status';
import type { StatusKind } from '$api/StatusKind';
import { formatDate } from '#lib/format/date.js';
import { formatMoney } from '#lib/format/money.js';
import { ru } from '#lib/i18n/ru.js';
import { deviceOf } from '#lib/orders/view.js';

const t = ru.client.brief;

/** The first line with text of the malfunction; '' when there is none. */
function firstLine(text: string): string {
	return (
		text
			.split('\n')
			.map((l) => l.trim())
			.find((l) => l !== '') ?? ''
	);
}

/**
 * «№ 41 · 02.11.2026» and the lines under it: the device, the malfunction, the total, the prepayment and
 * discount when there are any, and «Выдан 12.11.2026» for an order issued. Any other order shows its
 * status (none while the statuses are not loaded); one issued before and back in work since (a warranty
 * return keeps the first day of the issue) adds «Выдавался 12.11.2026».
 */
export function orderBrief(
	o: OrderListItem,
	status: Pick<Status, 'name' | 'kind'> | undefined
): { title: string; lines: string[] } {
	const lines = [deviceOf(o)];
	const malfunction = firstLine(o.malfunction);
	if (malfunction) lines.push(malfunction);
	lines.push(o.final_price === null ? t.noTotal : t.total(formatMoney(o.final_price)));
	if (o.prepayment > 0) lines.push(t.prepayment(formatMoney(o.prepayment)));
	if (o.discount > 0) lines.push(t.discount(formatMoney(o.discount)));
	if (status?.kind === 'issued' && o.issued_at !== null) {
		lines.push(t.issued(formatDate(o.issued_at)));
	} else {
		if (status) lines.push(status.name);
		if (o.issued_at !== null) lines.push(t.issuedBefore(formatDate(o.issued_at)));
	}
	return { title: t.title(o.id, formatDate(o.received_at)), lines };
}

/**
 * The second line of a row where there is no hover (a phone, a touch screen): «2 500 ₽ · выдан
 * 12.11.2026», what of it there is; an order back in work after its issue says «выдавался», its badge
 * tells the status.
 */
export function phoneLine(o: OrderListItem, kind: StatusKind | undefined): string {
	const issued =
		o.issued_at === null
			? ''
			: kind === 'issued'
				? t.issuedShort(formatDate(o.issued_at))
				: t.issuedBeforeShort(formatDate(o.issued_at));
	return [o.final_price === null ? '' : formatMoney(o.final_price), issued]
		.filter(Boolean)
		.join(' · ');
}
