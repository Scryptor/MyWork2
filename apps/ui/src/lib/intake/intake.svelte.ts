import { onSessionReset } from '#lib/app/session-reset.js';

export interface IntakePrefill {
	/** Intake for this client (from the client's card). */
	clientId?: number;
}

/** How long «Принять» waits for the server before the sheet can be used again. */
export const SUBMIT_TIMEOUT_MS = 30_000;

export interface SubmitOptions {
	/** What has to be done before the order goes (photos still uploading); the 30 s start after it. */
	ready?: () => Promise<unknown>;
	timeoutMs?: number;
}

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
	/** The order waits for its photos to upload («Загружаю фото…»); «Отмена» can still call it off. */
	uploading = $state(false);
	/**
	 * The form of the opening that sent the order takes no edits until the answer: what is typed then
	 * would not be in the order, and the answer clears the draft. Closed and opened again, the form can
	 * be changed: the answer then keeps the draft and says it is accepted.
	 */
	formLocked = $state(false);
	/** Openings counted: an answer tells the opening that sent the order from a newer one. */
	#opening = 0;
	#callOff: (() => void) | null = null;

	open(prefill: IntakePrefill = {}): void {
		if (!this.isOpen) this.#opening += 1;
		this.prefill = prefill;
		this.isOpen = true;
	}

	/**
	 * The session ended: the sheet closes, a waiting order is called off. The draft stays on the device:
	 * an expired session keeps it for the same user (`claimDraft` hides it from another one); only
	 * signing out removes it (`forgetDraft`).
	 */
	reset(): void {
		this.callOff();
		this.close();
	}

	close(): void {
		this.isOpen = false;
		this.prefill = {};
		this.formLocked = false;
	}

	/**
	 * «Отмена» while the order waits for its photos: it is not sent. False once it is on its way: an order
	 * sent cannot be called back.
	 */
	callOff(): boolean {
		const off = this.#callOff;
		off?.();
		return off !== null;
	}

	/**
	 * Sends an order once `ready` is done; null at once while another one is on its way, and null when it
	 * was called off before it went. Without an answer in `timeoutMs` after it went the request is called
	 * off through `signal`, and the sheet stops waiting even if it is not.
	 */
	async submit<T>(
		send: (signal: AbortSignal) => Promise<T>,
		{ ready, timeoutMs = SUBMIT_TIMEOUT_MS }: SubmitOptions = {}
	): Promise<Submission<T> | null> {
		if (this.submitting) return null;
		const opening = this.#opening;
		const current = () => this.isOpen && this.#opening === opening;
		this.submitting = true;
		this.formLocked = this.isOpen;
		if (ready) {
			this.uploading = true;
			const off = new Promise<'off'>((resolve) => (this.#callOff = () => resolve('off')));
			const done = ready().then(
				() => 'ready' as const,
				() => 'ready' as const
			);
			const outcome = await Promise.race([done, off]);
			this.#callOff = null;
			this.uploading = false;
			if (outcome === 'off') {
				this.submitting = false;
				this.formLocked = false;
				return null;
			}
		}
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
			this.formLocked = false;
		}
	}
}

export const intake = new Intake();

onSessionReset(() => intake.reset());
