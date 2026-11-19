// The question «Есть несохранённый спор» when an order with an undecided dispute is about to be left
// (another order in the panel, another screen from the card). One dialog for the whole program.

import { onSessionReset } from '#lib/app/session-reset.js';

class Dispute {
	/** The order whose dispute is asked about; null: no question. */
	pending = $state<number | null>(null);
	#resolve: ((leave: boolean) => void) | null = null;

	/** Resolves to true for «Отбросить и перейти», false for «Остаться» (or when another question comes). */
	ask(orderId: number): Promise<boolean> {
		this.#resolve?.(false);
		return new Promise((resolve) => {
			this.#resolve = resolve;
			this.pending = orderId;
		});
	}

	answer(leave: boolean): void {
		const resolve = this.#resolve;
		this.#resolve = null;
		this.pending = null;
		resolve?.(leave);
	}
}

export const dispute = new Dispute();

onSessionReset(() => dispute.answer(false));
