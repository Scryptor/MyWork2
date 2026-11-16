import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { ScanResult } from '#lib/design/scanner.js';
import { ru } from '#lib/i18n/ru.js';

export type ScanAction =
	| { kind: 'open'; id: number }
	| { kind: 'choose'; orders: OrderListItem[] }
	| { kind: 'notFound'; text: string }
	| { kind: 'search'; text: string };

export interface ScanDeps {
	/** Orders with this barcode; old MyWork 2 codes may repeat. */
	byBarcode(code: string): Promise<OrderListItem[]>;
	/** The order with this number, null when there is none. */
	byNumber(n: number): Promise<Order | null>;
}

/** What a scan outside fields does (spec 6.10): open an order, ask which one, report, or search. */
export async function resolveScan(r: ScanResult, deps: ScanDeps): Promise<ScanAction> {
	switch (r.kind) {
		case 'barcode': {
			const found = await deps.byBarcode(r.value);
			if (found.length === 1) return { kind: 'open', id: found[0]!.id };
			if (found.length > 1) return { kind: 'choose', orders: found };
			return { kind: 'notFound', text: ru.scan.barcodeNotFound(r.value) };
		}
		case 'order': {
			const found = await deps.byNumber(r.id);
			return found
				? { kind: 'open', id: found.id }
				: { kind: 'notFound', text: ru.scan.numberNotFound(r.id) };
		}
		case 'text':
			return { kind: 'search', text: r.value };
	}
}
