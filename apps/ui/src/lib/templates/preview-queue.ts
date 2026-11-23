/**
 * Debounced previews, one request at a time. `request` waits `delayMs` after the last call; while a
 * run is going, newer requests make one more run after it, with the last input only. The answer to an
 * input that is no longer the last is dropped.
 */
export function createPreviewQueue<I, O>(
	run: (input: I, signal: AbortSignal) => Promise<O>,
	opts: { delayMs: number; onresult: (o: O) => void; onerror: (e: unknown) => void }
): { request(input: I): void; dispose(): void } {
	let latest: I | undefined;
	let seq = 0;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let running: AbortController | undefined;
	let dirty = false;
	let disposed = false;

	function start() {
		if (disposed || latest === undefined) return;
		dirty = false;
		const mine = seq;
		const ctl = new AbortController();
		running = ctl;
		run(latest, ctl.signal).then(
			(o) => finish(ctl, mine, () => opts.onresult(o)),
			(e) => finish(ctl, mine, () => opts.onerror(e))
		);
	}

	function finish(ctl: AbortController, mine: number, deliver: () => void) {
		if (running === ctl) running = undefined;
		if (disposed || ctl.signal.aborted) return;
		if (mine === seq) deliver();
		if (dirty) start();
	}

	return {
		request(input) {
			if (disposed) return;
			latest = input;
			seq += 1;
			clearTimeout(timer);
			timer = setTimeout(() => {
				timer = undefined;
				if (running) dirty = true;
				else start();
			}, opts.delayMs);
		},
		dispose() {
			disposed = true;
			clearTimeout(timer);
			running?.abort();
		}
	};
}
