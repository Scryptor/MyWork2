import { afterEach, expect, it, vi } from 'vitest';
import { serverClock } from './server-clock.svelte.js';

afterEach(() => vi.useRealTimers());

it('now follows server time plus elapsed local time', () => {
	vi.useFakeTimers();
	vi.setSystemTime(new Date('2026-01-01T00:00:00Z'));
	serverClock.sync('2026-11-15T10:24:00');
	vi.advanceTimersByTime(90_000);
	expect(serverClock.now()).toMatchObject({
		year: 2026,
		month: 11,
		day: 15,
		hour: 10,
		minute: 25,
		second: 30
	});
});

it('falls back to local time before the first sync and ignores garbage', async () => {
	vi.resetModules();
	const { serverClock } = await import('./server-clock.svelte.js');
	vi.useFakeTimers();
	vi.setSystemTime(new Date(2026, 5, 1, 12, 0, 0));
	serverClock.sync('not a date');
	expect(serverClock.now()).toMatchObject({ year: 2026, month: 6, day: 1, hour: 12 });
});
