// Everything a signed-in user leaves behind in memory: open editors with their unsaved edits, the caches
// and the state of the screens. Singletons register their reset here (`onSessionReset`); editors register
// their queue (`trackQueue`). Signing out settles the queues first and then resets everything, so the
// next user of the device sees nothing of the previous one and no edit goes out in their name.
// Per-device settings (`display`, `prefs`, the chosen point on disk) are not user data: they stay.

/** An edit queue of an open record. */
export interface TrackedQueue {
	/** Own changes that are not on the server. */
	unsaved(): boolean;
	/** Sends what waits; false when something stays unsaved (a failure, a dispute). */
	flush(): Promise<boolean>;
	/** Gives the unsaved changes up. */
	discard(): void;
}

/** How long signing out waits for the queues to empty. */
export const SETTLE_TIMEOUT_MS = 5000;

let epoch = 0;
/** Counts the sessions: work queued under one must not run under the next. */
export const sessionEpoch = (): number => epoch;

const queues = new Set<TrackedQueue>();
const resets = new Set<() => void>();

/** Registers an edit queue; returns the removal (the editor is disposed). */
export function trackQueue(q: TrackedQueue): () => void {
	queues.add(q);
	return () => void queues.delete(q);
}

/** Registers the reset of a module-level singleton; returns the removal. */
export function onSessionReset(reset: () => void): () => void {
	resets.add(reset);
	return () => void resets.delete(reset);
}

/** Is there an open record with changes the server does not have. */
export function hasUnsaved(): boolean {
	return [...queues].some((q) => q.unsaved());
}

/**
 * Sends what every queue holds and waits (up to `timeoutMs`) until nothing is left. True when nothing
 * is unsaved after it; false when something could not be sent in time or at all.
 */
export async function settleQueues(timeoutMs = SETTLE_TIMEOUT_MS): Promise<boolean> {
	const all = [...queues];
	let timer: ReturnType<typeof setTimeout> | undefined;
	const timeout = new Promise<false>((resolve) => {
		timer = setTimeout(() => resolve(false), timeoutMs);
	});
	try {
		await Promise.race([Promise.all(all.map((q) => q.flush().catch(() => false))), timeout]);
	} finally {
		clearTimeout(timer);
	}
	return !hasUnsaved();
}

/** «Выйти без сохранения»: the unsaved changes of every open record are given up. */
export function discardQueues(): void {
	for (const q of [...queues]) q.discard();
}

/** The session ends: queues are emptied, every singleton goes back to its start. */
export function resetSession(): void {
	epoch += 1;
	discardQueues();
	for (const reset of [...resets]) reset();
}
