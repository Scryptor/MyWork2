import type { Order } from '$api/Order';

/** `created` — accepted just now (intake): a list may need it as a new row. */
export type OrderChange = 'created' | 'updated';

type Listener = (o: Order, change: OrderChange) => void;

let listeners: Listener[] = [];

/** An order was changed here (panel, bulk action, intake): lists and the summary catch up. */
export function emitOrderChanged(o: Order, change: OrderChange = 'updated'): void {
	for (const fn of listeners) fn(o, change);
}

/** Listens to changed orders; returns the unsubscribe. */
export function onOrderChanged(fn: Listener): () => void {
	const own: Listener = (o, change) => fn(o, change);
	listeners = [...listeners, own];
	return () => {
		listeners = listeners.filter((l) => l !== own);
	};
}
