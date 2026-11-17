import { pushRecent } from '#lib/app/recent.js';

export interface PanelOptions {
	/** The numbers of the list the order was opened from, as they are now: ‹ › and ↑↓ go along them. */
	neighbors?: () => number[];
	/** Where «Назад» of the panel leads, when it was opened from another screen. */
	back?: { label: string; href: string };
}

/** Asked before the panel leaves its order: true lets it go, false keeps it (a save failed). */
export type LeaveGuard = () => Promise<boolean>;

/**
 * The order panel of the `(app)` layout: it opens from any screen (the list, Ctrl K, the scanner).
 * The panel itself reads `id`, sets the guard and calls `close` and `next`. Without a guard every
 * change happens at once; with one, leaving an order waits for it.
 */
class OrderPanel {
	id = $state<number | null>(null);
	back = $state.raw<PanelOptions['back'] | null>(null);
	#neighbors: (() => number[]) | undefined;
	#guard: LeaveGuard | null = null;

	/** The panel saves its order before another one is shown or the panel closes; returns the removal. */
	guard(fn: LeaveGuard): () => void {
		this.#guard = fn;
		return () => {
			if (this.#guard === fn) this.#guard = null;
		};
	}

	/** Leaves the open order for `to` (null: closes) once the guard allows; resolves to whether it did. */
	#leave(to: number | null, go: () => void): Promise<boolean> {
		const guard = this.#guard;
		if (!guard || this.id === null || this.id === to) {
			go();
			return Promise.resolve(true);
		}
		return guard().then(
			(ok) => {
				if (ok) go();
				return ok;
			},
			() => false
		);
	}

	open(id: number, opts: PanelOptions = {}): Promise<boolean> {
		return this.#leave(id, () => {
			this.id = id;
			this.back = opts.back ?? null;
			this.#neighbors = opts.neighbors;
			pushRecent(id);
		});
	}

	close(): Promise<boolean> {
		return this.#leave(null, () => {
			this.id = null;
			this.back = null;
			this.#neighbors = undefined;
		});
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
	next(dir: 1 | -1): Promise<boolean> {
		const target = this.#neighbor(dir);
		if (target === undefined) return Promise.resolve(false);
		return this.#leave(target, () => {
			// Counted again: the list or the order may have changed while the guard was saving.
			const id = this.#neighbor(dir);
			if (id === undefined) return;
			this.id = id;
			pushRecent(id);
		});
	}
}

export const orderPanel = new OrderPanel();
