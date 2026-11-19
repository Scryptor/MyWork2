import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BLOB_LIFETIME_MS, downloadText } from './download.js';

describe('downloadText', () => {
	const revoke = vi.fn();

	beforeEach(() => {
		vi.useFakeTimers();
		// jsdom has no blob addresses (each test file has a jsdom of its own).
		URL.createObjectURL = () => 'blob:x';
		URL.revokeObjectURL = revoke;
	});
	afterEach(() => {
		vi.useRealTimers();
		revoke.mockReset();
	});

	it('keeps the address of the file for a minute: the save may start after the click returns', () => {
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
		downloadText('отчёт.txt', 'текст');
		expect(click).toHaveBeenCalledOnce();
		expect(document.querySelector('a[download]')).toBeNull();
		vi.advanceTimersByTime(1000);
		expect(revoke).not.toHaveBeenCalled();
		expect(BLOB_LIFETIME_MS).toBe(60_000);
		vi.advanceTimersByTime(BLOB_LIFETIME_MS);
		expect(revoke).toHaveBeenCalledWith('blob:x');
		click.mockRestore();
	});
});
