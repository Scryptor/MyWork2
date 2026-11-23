// «Заказ для примера» of the template editor: «Пример» (the sample order) or an order found by its number,
// client or device. The text of the field and the order chosen are the model's; the search waits for a
// pause in the typing and an answer of a query that was superseded is dropped.

export interface OrderChoice {
	id: number;
	/** What the field shows when the order is chosen. */
	label: string;
	hint?: string;
}

export interface OrderSearch {
	/** The order chosen; null: the sample. */
	readonly orderId: number | null;
	/** What the field shows. */
	readonly text: string;
	/** «Пример» and what was found. */
	readonly items: { value: string; hint?: string }[];
	setText(text: string): void;
	/** An item chosen by its label. */
	pick(label: string): void;
	/** Left with a search that picked nothing: the field shows the order chosen again. */
	leave(): void;
	dispose(): void;
}

export function createOrderSearch(o: {
	find: (query: string) => Promise<OrderChoice[]>;
	sample: string;
	sampleHint: string;
	delayMs?: number;
}): OrderSearch {
	const delay = o.delayMs ?? 200;
	let orderId = $state<number | null>(null);
	let text = $state(o.sample);
	let found = $state.raw<OrderChoice[]>([]);
	/** Every label offered so far and its order; the sample is null. */
	let known = $state.raw<[string, number | null][]>([[o.sample, null]]);
	const idOf = (label: string) => known.find(([l]) => l === label)?.[1];
	let timer: ReturnType<typeof setTimeout> | undefined;
	let token = 0;

	const shown = () => known.find(([, id]) => id === orderId)?.[0] ?? o.sample;

	function setText(next: string) {
		text = next;
		clearTimeout(timer);
		const mine = ++token;
		if (next.trim() === '' || idOf(next) !== undefined) {
			// Nothing to look for: what the last query found is not offered any more.
			if (next.trim() === '') found = [];
			return;
		}
		timer = setTimeout(() => {
			o.find(next).then(
				(list) => {
					if (mine !== token) return;
					found = list;
					known = [...known, ...list.map((c): [string, number] => [c.label, c.id])];
				},
				() => {
					// No suggestions without an answer.
				}
			);
		}, delay);
	}

	return {
		get orderId() {
			return orderId;
		},
		get text() {
			return text;
		},
		get items() {
			return [
				{ value: o.sample, hint: o.sampleHint },
				...found.map((c) => ({ value: c.label, hint: c.hint }))
			];
		},
		setText,
		pick(label) {
			const id = idOf(label);
			if (id === undefined) return;
			orderId = id;
			// The sample is the end of the search: the orders it found are not offered next time.
			if (id === null) found = [];
		},
		leave() {
			if (idOf(text) === undefined) text = shown();
		},
		dispose() {
			clearTimeout(timer);
			token++;
		}
	};
}
