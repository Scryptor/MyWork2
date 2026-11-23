import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createPreviewQueue } from './preview-queue.js';

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup() {
	const calls: { input: number; signal: AbortSignal; done: (o: string) => void }[] = [];
	let active = 0;
	let maxActive = 0;
	const results: string[] = [];
	const errors: unknown[] = [];
	const q = createPreviewQueue<number, string>(
		(input, signal) => {
			active++;
			maxActive = Math.max(maxActive, active);
			return new Promise((resolve) =>
				calls.push({
					input,
					signal,
					done: (o) => {
						active--;
						resolve(o);
					}
				})
			);
		},
		{ delayMs: 400, onresult: (o) => results.push(o), onerror: (e) => errors.push(e) }
	);
	return { q, calls, results, errors, maxActive: () => maxActive };
}

describe('createPreviewQueue', () => {
	it('preview_queue_runs_one_at_a_time_with_the_latest', async () => {
		const { q, calls, results, maxActive } = setup();
		q.request(1);
		vi.advanceTimersByTime(100);
		q.request(2);
		vi.advanceTimersByTime(100);
		q.request(3);
		vi.advanceTimersByTime(399);
		expect(calls).toHaveLength(0);
		vi.advanceTimersByTime(1);
		expect(calls.map((c) => c.input)).toEqual([3]);

		// Requests during a run: one more run after it, with the last input only.
		q.request(4);
		vi.advanceTimersByTime(400);
		q.request(5);
		vi.advanceTimersByTime(400);
		expect(calls).toHaveLength(1);
		calls[0]!.done('r3');
		await vi.advanceTimersByTimeAsync(0);
		// The answer to 3 is stale: not delivered.
		expect(results).toEqual([]);
		expect(calls.map((c) => c.input)).toEqual([3, 5]);
		calls[1]!.done('r5');
		await vi.advanceTimersByTimeAsync(0);
		expect(results).toEqual(['r5']);
		expect(calls).toHaveLength(2);
		expect(maxActive()).toBe(1);
	});

	it('the result of the latest input is delivered', async () => {
		const { q, calls, results } = setup();
		q.request(1);
		vi.advanceTimersByTime(400);
		calls[0]!.done('r1');
		await vi.advanceTimersByTimeAsync(0);
		expect(results).toEqual(['r1']);
	});

	it('an error of the latest run goes to onerror', async () => {
		const errors: unknown[] = [];
		const q = createPreviewQueue<number, string>(() => Promise.reject(new Error('x')), {
			delayMs: 10,
			onresult: () => {},
			onerror: (e) => errors.push(e)
		});
		q.request(1);
		await vi.advanceTimersByTimeAsync(10);
		expect(errors).toHaveLength(1);
	});

	it('dispose aborts the run and silences it', async () => {
		const { q, calls, results, errors } = setup();
		q.request(1);
		vi.advanceTimersByTime(400);
		q.dispose();
		expect(calls[0]!.signal.aborted).toBe(true);
		calls[0]!.done('late');
		await vi.advanceTimersByTimeAsync(0);
		expect(results).toEqual([]);
		expect(errors).toEqual([]);
	});

	it('dispose cancels a pending request', () => {
		const { q, calls } = setup();
		q.request(1);
		q.dispose();
		vi.advanceTimersByTime(1000);
		expect(calls).toHaveLength(0);
	});
});
