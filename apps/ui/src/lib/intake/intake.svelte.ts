export interface IntakePrefill {
	/** Intake for this client (from the client's card). */
	clientId?: number;
}

/**
 * The answer about an order sent by `submit`. `current`: the sheet is still open as it was when the
 * order went; false when it closed meanwhile, or closed and opened again.
 */
export type Submission<T> =
	{ ok: true; value: T; current: boolean } | { ok: false; error: unknown; current: boolean };

/** The intake sheet of the `(app)` layout (task 10 mounts it): N, «＋ Принять», Ctrl K. */
class Intake {
	isOpen = $state(false);
	prefill = $state.raw<IntakePrefill>({});
	/**
	 * An order is on its way to the server. It outlives the sheet: closed and opened again, the sheet
	 * still waits for it, and no second order is sent meanwhile.
	 */
	submitting = $state(false);
	/** Openings counted: an answer tells the opening that sent the order from a newer one. */
	#opening = 0;

	open(prefill: IntakePrefill = {}): void {
		if (!this.isOpen) this.#opening += 1;
		this.prefill = prefill;
		this.isOpen = true;
	}

	close(): void {
		this.isOpen = false;
		this.prefill = {};
	}

	/** Sends an order; null at once while another one is on its way. */
	async submit<T>(send: () => Promise<T>): Promise<Submission<T> | null> {
		if (this.submitting) return null;
		const opening = this.#opening;
		const current = () => this.isOpen && this.#opening === opening;
		this.submitting = true;
		try {
			const value = await send();
			return { ok: true, value, current: current() };
		} catch (error) {
			return { ok: false, error, current: current() };
		} finally {
			this.submitting = false;
		}
	}
}

export const intake = new Intake();
