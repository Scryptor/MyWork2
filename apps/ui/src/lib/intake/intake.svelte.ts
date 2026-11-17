export interface IntakePrefill {
	/** Intake for this client (from the client's card). */
	clientId?: number;
}

/** How long «Принять» waits for the server before the sheet can be used again. */
export const SUBMIT_TIMEOUT_MS = 30_000;

/**
 * The answer about an order sent by `submit`. `current`: the sheet is still open as it was when the
 * order went; false when it closed meanwhile, or closed and opened again. `timedOut`: no answer came in
 * time and the request was called off — the server may still have made the order.
 */
export type Submission<T> =
	| { ok: true; value: T; current: boolean }
	| { ok: false; error: unknown; timedOut: boolean; current: boolean };

const NO_ANSWER = new Error('no answer');

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

	/**
	 * Sends an order; null at once while another one is on its way. Without an answer in `timeoutMs` the
	 * request is called off through `signal`, and the sheet stops waiting even if it is not.
	 */
	async submit<T>(
		send: (signal: AbortSignal) => Promise<T>,
		timeoutMs = SUBMIT_TIMEOUT_MS
	): Promise<Submission<T> | null> {
		if (this.submitting) return null;
		const opening = this.#opening;
		const current = () => this.isOpen && this.#opening === opening;
		this.submitting = true;
		const controller = new AbortController();
		let timer: ReturnType<typeof setTimeout> | undefined;
		const timeout = new Promise<never>((_, reject) => {
			timer = setTimeout(() => {
				reject(NO_ANSWER);
				controller.abort();
			}, timeoutMs);
		});
		try {
			const value = await Promise.race([send(controller.signal), timeout]);
			return { ok: true, value, current: current() };
		} catch (error) {
			return { ok: false, error, timedOut: error === NO_ANSWER, current: current() };
		} finally {
			clearTimeout(timer);
			this.submitting = false;
		}
	}
}

export const intake = new Intake();
