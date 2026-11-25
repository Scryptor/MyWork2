// The brief of an earlier order in the client's card: on hover a card, on a phone a second line.
import type { OrderListItem } from '$api/OrderListItem';
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
 * discount when there are any, and the day it was issued — or the status of an order not issued (none
 * while the statuses are not loaded).
 */
export function orderBrief(
	o: OrderListItem,
	statusName: string
): { title: string; lines: string[] } {
	const lines = [deviceOf(o)];
	const malfunction = firstLine(o.malfunction);
	if (malfunction) lines.push(malfunction);
	lines.push(o.final_price === null ? t.noTotal : t.total(formatMoney(o.final_price)));
	if (o.prepayment > 0) lines.push(t.prepayment(formatMoney(o.prepayment)));
	if (o.discount > 0) lines.push(t.discount(formatMoney(o.discount)));
	if (o.issued_at !== null) lines.push(t.issued(formatDate(o.issued_at)));
	else if (statusName) lines.push(statusName);
	return { title: t.title(o.id, formatDate(o.received_at)), lines };
}

/** The second line of a row on a phone: «2 500 ₽ · выдан 12.11.2026», what of it there is. */
export function phoneLine(o: OrderListItem): string {
	return [
		o.final_price === null ? '' : formatMoney(o.final_price),
		o.issued_at === null ? '' : t.issuedShort(formatDate(o.issued_at))
	]
		.filter(Boolean)
		.join(' · ');
}
