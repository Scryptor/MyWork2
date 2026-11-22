import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { BLOB_LIFETIME_MS, downloadBlob, downloadText } from './download.js';

describe('downloadText and downloadBlob', () => {
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

	it('a file of the server (a PDF) is saved under the name it came with', () => {
		let saved: { name: string; href: string } | null = null;
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
			this: HTMLAnchorElement
		) {
			saved = { name: this.download, href: this.href };
		});
		downloadBlob('akt-priema-42.pdf', new Blob(['%PDF'], { type: 'application/pdf' }));
		expect(saved).toEqual({ name: 'akt-priema-42.pdf', href: 'blob:x' });
		vi.advanceTimersByTime(BLOB_LIFETIME_MS);
		expect(revoke).toHaveBeenCalledWith('blob:x');
		click.mockRestore();
	});
});
