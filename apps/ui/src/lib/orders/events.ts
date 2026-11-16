import type { Order } from '$api/Order';

type Listener = (o: Order) => void;

let listeners: Listener[] = [];

/** An order was changed here (panel, bulk action, intake): lists and the summary catch up. */
export function emitOrderChanged(o: Order): void {
	for (const fn of listeners) fn(o);
}

/** Listens to changed orders; returns the unsubscribe. */
export function onOrderChanged(fn: Listener): () => void {
	const own: Listener = (o) => fn(o);
	listeners = [...listeners, own];
	return () => {
		listeners = listeners.filter((l) => l !== own);
	};
}
