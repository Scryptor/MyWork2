import type { OrderSummary } from '$api/OrderSummary';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { STATUSES } from './fixtures.test-helper.js';
import { SUMMARY_GAP_MS, attentionCounts, createSummary, inWorkCount } from './summary.svelte.js';

const summaryOf = (need_call: number): OrderSummary => ({
	by_status: [
		{ status_id: 1, count: 4 },
		{ status_id: 2, count: 3 },
		{ status_id: 3, count: 10 }
	],
	need_call,
	need_parts: 0,
	overdue_diagnostics: 2,
	stale_ready: 0
});

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('summary', () => {
	it('asks at once, then at most once a second with the newest point', async () => {
		const fetcher = vi.fn(async (point: number | null) => summaryOf(point ?? 0));
		const s = createSummary(fetcher);
		s.refresh(null);
		await vi.advanceTimersByTimeAsync(0);
		expect(fetcher.mock.calls).toEqual([[null]]);
		expect(s.current?.need_call).toBe(0);
		s.refresh(1);
		s.refresh(2);
		await vi.advanceTimersByTimeAsync(SUMMARY_GAP_MS - 1);
		expect(fetcher).toHaveBeenCalledTimes(1);
		await vi.advanceTimersByTimeAsync(1);
		expect(fetcher.mock.calls).toEqual([[null], [2]]);
		expect(s.current?.need_call).toBe(2);
	});

	it('a late answer to an older request does not replace a newer one; a failure keeps the last', async () => {
		const answers: ((v: OrderSummary) => void)[] = [];
		const fetcher = vi.fn<(point: number | null) => Promise<OrderSummary>>(
			() => new Promise<OrderSummary>((done) => answers.push(done))
		);
		const s = createSummary(fetcher);
		s.refresh(null);
		await vi.advanceTimersByTimeAsync(SUMMARY_GAP_MS);
		s.refresh(1);
		await vi.advanceTimersByTimeAsync(0);
		answers[1]!(summaryOf(1));
		answers[0]!(summaryOf(9));
		await vi.advanceTimersByTimeAsync(0);
		expect(s.current?.need_call).toBe(1);

		fetcher.mockRejectedValueOnce(new Error('Нет связи с сервером'));
		await vi.advanceTimersByTimeAsync(SUMMARY_GAP_MS);
		s.refresh(1);
		await vi.advanceTimersByTimeAsync(0);
		expect(s.current?.need_call).toBe(1);
	});

	it('«В работе» counts open and diagnostics statuses; attention follows the summary', () => {
		expect(inWorkCount(summaryOf(5), STATUSES)).toBe(7);
		expect(inWorkCount(null, STATUSES)).toBeUndefined();
		expect(attentionCounts(summaryOf(5))).toEqual({
			call: 5,
			parts: 0,
			longDiagnostics: 2,
			longReady: 0
		});
		expect(attentionCounts(null)).toBeNull();
	});
});
