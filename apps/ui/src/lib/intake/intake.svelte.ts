export interface IntakePrefill {
	/** Intake for this client (from the client's card). */
	clientId?: number;
}

/** The intake sheet of the `(app)` layout (task 10 mounts it): N, «＋ Принять», Ctrl K. */
class Intake {
	isOpen = $state(false);
	prefill = $state.raw<IntakePrefill>({});

	open(prefill: IntakePrefill = {}): void {
		this.prefill = prefill;
		this.isOpen = true;
	}

	close(): void {
		this.isOpen = false;
		this.prefill = {};
	}
}

export const intake = new Intake();
