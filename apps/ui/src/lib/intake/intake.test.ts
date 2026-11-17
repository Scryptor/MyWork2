import { afterEach, describe, expect, it, vi } from 'vitest';
import { intake, SUBMIT_TIMEOUT_MS } from './intake.svelte.js';

/** A request that answers when told to. */
function later<T>() {
	let resolve: (v: T) => void = () => {};
	let reject: (e: unknown) => void = () => {};
	const promise = new Promise<T>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

afterEach(() => intake.close());

describe('intake', () => {
	it('opens with a prefill and closes without it', () => {
		intake.open({ clientId: 4 });
		expect(intake.isOpen).toBe(true);
		expect(intake.prefill).toEqual({ clientId: 4 });
		intake.close();
		expect(intake.isOpen).toBe(false);
		expect(intake.prefill).toEqual({});
	});

	it('sends one order at a time: «Принять» again while it is on its way sends nothing', async () => {
		intake.open();
		const first = later<number>();
		const send = vi.fn(() => first.promise);
		const pending = intake.submit(send);
		expect(intake.submitting).toBe(true);
		const again = vi.fn(() => Promise.resolve(2));
		await expect(intake.submit(again)).resolves.toBeNull();
		expect(again).not.toHaveBeenCalled();
		first.resolve(1);
		await expect(pending).resolves.toEqual({ ok: true, value: 1, current: true });
		expect(intake.submitting).toBe(false);
		expect(send).toHaveBeenCalledTimes(1);
	});

	it('the sheet closed while the order was on its way: the answer is not about an open sheet', async () => {
		intake.open();
		const req = later<number>();
		const pending = intake.submit(() => req.promise);
		intake.close();
		req.resolve(7);
		await expect(pending).resolves.toEqual({ ok: true, value: 7, current: false });
	});

	it('an answer about an earlier opening leaves a newer one open and says so', async () => {
		intake.open();
		const req = later<number>();
		const pending = intake.submit(() => req.promise);
		intake.close();
		intake.open();
		// The reopened sheet is still waiting for the same order.
		expect(intake.submitting).toBe(true);
		await expect(intake.submit(() => Promise.resolve(8))).resolves.toBeNull();
		req.resolve(7);
		await expect(pending).resolves.toEqual({ ok: true, value: 7, current: false });
		expect(intake.isOpen).toBe(true);
		expect(intake.submitting).toBe(false);
	});

	it('a failure comes back with the error, and the next «Принять» can send again', async () => {
		intake.open();
		const error = new Error('нет связи');
		await expect(intake.submit(() => Promise.reject(error))).resolves.toEqual({
			ok: false,
			error,
			timedOut: false,
			current: true
		});
		expect(intake.submitting).toBe(false);
		await expect(intake.submit(() => Promise.resolve(3))).resolves.toEqual({
			ok: true,
			value: 3,
			current: true
		});
	});

	it('opening an open sheet again is the same opening', async () => {
		intake.open();
		const req = later<number>();
		const pending = intake.submit(() => req.promise);
		intake.open({ clientId: 2 });
		req.resolve(5);
		await expect(pending).resolves.toEqual({ ok: true, value: 5, current: true });
	});

	it('no answer in 30 s: the request is called off and the sheet can be used again', async () => {
		vi.useFakeTimers();
		try {
			intake.open();
			let signal: AbortSignal | undefined;
			const pending = intake.submit((s) => {
				signal = s;
				return new Promise<number>(() => {});
			});
			vi.advanceTimersByTime(SUBMIT_TIMEOUT_MS - 1);
			expect(intake.submitting).toBe(true);
			expect(signal?.aborted).toBe(false);
			vi.advanceTimersByTime(1);
			await expect(pending).resolves.toMatchObject({ ok: false, timedOut: true, current: true });
			expect(signal?.aborted).toBe(true);
			expect(intake.submitting).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});
});
