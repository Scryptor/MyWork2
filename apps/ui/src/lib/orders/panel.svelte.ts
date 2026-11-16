import { pushRecent } from '#lib/app/recent.js';

export interface PanelOptions {
	/** The numbers of the list the order was opened from, as they are now: ‹ › and ↑↓ go along them. */
	neighbors?: () => number[];
	/** Where «Назад» of the panel leads, when it was opened from another screen. */
	back?: { label: string; href: string };
}

/**
 * The order panel of the `(app)` layout: it opens from any screen (the list, Ctrl K, the scanner).
 * The panel itself (task 8) reads `id` and calls `close` and `next`.
 */
class OrderPanel {
	id = $state<number | null>(null);
	back = $state.raw<PanelOptions['back'] | null>(null);
	#neighbors: (() => number[]) | undefined;

	open(id: number, opts: PanelOptions = {}): void {
		this.id = id;
		this.back = opts.back ?? null;
		this.#neighbors = opts.neighbors;
		pushRecent(id);
	}

	close(): void {
		this.id = null;
		this.back = null;
		this.#neighbors = undefined;
	}

	#neighbor(dir: 1 | -1): number | undefined {
		if (this.id === null || !this.#neighbors) return undefined;
		const list = this.#neighbors();
		const at = list.indexOf(this.id);
		return at < 0 ? undefined : list[at + dir];
	}

	/** Is there a neighbouring order in this direction. */
	hasNext(dir: 1 | -1): boolean {
		return this.#neighbor(dir) !== undefined;
	}

	/** The neighbouring order of the list, if there is one. */
	next(dir: 1 | -1): void {
		const id = this.#neighbor(dir);
		if (id === undefined) return;
		this.id = id;
		pushRecent(id);
	}
}

export const orderPanel = new OrderPanel();
