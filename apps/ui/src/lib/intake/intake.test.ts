import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const platform = vi.hoisted(() => ({
	canPrintDirectly: vi.fn<() => boolean>(),
	listPrinters: vi.fn(),
	printFormat: vi.fn(),
	printPdf: vi.fn(),
	printPages: vi.fn()
}));
vi.mock('#lib/print/platform.js', () => platform);
const documents = vi.hoisted(() => ({
	documentPdf: vi.fn(),
	documentPages: vi.fn(),
	documentBatchPdf: vi.fn()
}));
vi.mock('#lib/api/documents.js', () => documents);
vi.mock('$app/navigation', () => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import { printDocuments, preview } from '#lib/print/print.svelte.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	intake,
	intakeDocuments,
	intakePrintOptions,
	printSheet,
	SUBMIT_TIMEOUT_MS
} from './intake.svelte.js';

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

afterEach(() => {
	intake.close();
	preview.reset();
	toast.clear();
	vi.clearAllMocks();
});

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

	it('photos on their way go first: «Загружаю фото…», and the 30 s start once they are done', async () => {
		vi.useFakeTimers();
		try {
			intake.open();
			const photos = later<void>();
			const send = vi.fn(() => new Promise<number>(() => {}));
			const pending = intake.submit(send, { ready: () => photos.promise });
			expect(intake.submitting).toBe(true);
			expect(intake.uploading).toBe(true);
			// Slow uploads are not «no answer».
			vi.advanceTimersByTime(SUBMIT_TIMEOUT_MS * 3);
			expect(send).not.toHaveBeenCalled();
			photos.resolve();
			await vi.waitFor(() => expect(send).toHaveBeenCalledTimes(1));
			expect(intake.uploading).toBe(false);
			expect(intake.submitting).toBe(true);
			vi.advanceTimersByTime(SUBMIT_TIMEOUT_MS);
			await expect(pending).resolves.toMatchObject({ ok: false, timedOut: true });
		} finally {
			vi.useRealTimers();
		}
	});

	it('the form of the opening that sent the order is locked until the answer; a new opening is not', async () => {
		intake.open();
		const photos = later<void>();
		const req = later<number>();
		const pending = intake.submit(() => req.promise, { ready: () => photos.promise });
		// While the photos upload and while the order is on its way: what is typed would not be in it.
		expect(intake.formLocked).toBe(true);
		photos.resolve();
		await vi.waitFor(() => expect(intake.uploading).toBe(false));
		expect(intake.formLocked).toBe(true);
		// Closed and opened again: the answer will say the draft is accepted, the form can be changed.
		intake.close();
		expect(intake.formLocked).toBe(false);
		intake.open();
		expect(intake.formLocked).toBe(false);
		req.resolve(1);
		await pending;
		expect(intake.formLocked).toBe(false);

		const failing = intake.submit(() => Promise.reject(new Error('нет связи')));
		expect(intake.formLocked).toBe(true);
		await failing;
		expect(intake.formLocked).toBe(false);
	});

	it('«Отмена» while the photos upload calls the order off: nothing is sent', async () => {
		intake.open();
		const photos = later<void>();
		const send = vi.fn(() => Promise.resolve(1));
		const pending = intake.submit(send, { ready: () => photos.promise });
		expect(intake.callOff()).toBe(true);
		await expect(pending).resolves.toBeNull();
		expect(intake.submitting).toBe(false);
		expect(intake.uploading).toBe(false);
		expect(intake.formLocked).toBe(false);
		photos.resolve();
		await Promise.resolve();
		expect(send).not.toHaveBeenCalled();
		// Once the order is on its way it cannot be called back.
		const req = later<number>();
		const sent = intake.submit(() => req.promise);
		expect(intake.callOff()).toBe(false);
		req.resolve(2);
		await expect(sent).resolves.toMatchObject({ ok: true, value: 2 });
	});

	it('a ready that says no stops the order: nothing is sent, the sheet can be used again', async () => {
		intake.open();
		const send = vi.fn(() => Promise.resolve(1));
		await expect(intake.submit(send, { ready: async () => false })).resolves.toBeNull();
		expect(send).not.toHaveBeenCalled();
		expect(intake.submitting).toBe(false);
		expect(intake.uploading).toBe(false);
		expect(intake.formLocked).toBe(false);
		expect(intake.isOpen).toBe(true);
		// «Принять» again with what is shown goes.
		await expect(intake.submit(send, { ready: async () => true })).resolves.toMatchObject({
			ok: true,
			value: 1
		});
		expect(send).toHaveBeenCalledOnce();
	});
});

describe('the documents of an accepted order', () => {
	it('are what the switches of the intake say: the act first, then the sticker', () => {
		expect(intakeDocuments({ act: true, sticker: true })).toEqual(['intake_act', 'sticker']);
		expect(intakeDocuments({ act: true, sticker: false })).toEqual(['intake_act']);
		expect(intakeDocuments({ act: false, sticker: true })).toEqual(['sticker']);
		expect(intakeDocuments({ act: false, sticker: false })).toEqual([]);
	});
});

describe('how the documents of an accepted order print', () => {
	it('the sheet went: as usual, in a browser the preview', () => {
		for (const direct of [true, false]) {
			platform.canPrintDirectly.mockReturnValue(direct);
			expect(intakePrintOptions('close')).toEqual({});
		}
	});

	it('another order is typed in the sheet: straight to a printer or not at all, never a preview over it', () => {
		platform.canPrintDirectly.mockReturnValue(true);
		expect(intakePrintOptions('other')).toEqual({ noPreview: true });
		platform.canPrintDirectly.mockReturnValue(false);
		expect(intakePrintOptions('other')).toBeNull();
	});
});

describe('the sheet an answer prints over', () => {
	it('an answer in time keeps the sheet as it decided', () => {
		expect(printSheet(true, 'close', false)).toBe('close');
		expect(printSheet(true, 'other', true)).toBe('other');
	});

	it('a late answer with a sheet open now is over a form that stays: no preview over it', () => {
		platform.canPrintDirectly.mockReturnValue(false);
		expect(intakePrintOptions(printSheet(false, 'close', true))).toBeNull();
		platform.canPrintDirectly.mockReturnValue(true);
		expect(intakePrintOptions(printSheet(false, 'close', true))).toEqual({ noPreview: true });
	});

	it('a late answer with no sheet open prints as usual', () => {
		expect(printSheet(false, 'close', false)).toBe('close');
	});
});

describe('an answer that comes late, printed through the way the sheet decides', () => {
	const toasts = () => toast.items.map((t) => [t.message, t.tone]);
	/** What the sheet does once the answer is in: the options of `printSheet`, then the documents. */
	const printAfter = (current: boolean, open: boolean) => {
		const options = intakePrintOptions(printSheet(current, 'close', open));
		return options === null
			? Promise.resolve()
			: printDocuments(intakeDocuments({ act: true, sticker: false }), { orderId: 5 }, options);
	};

	beforeEach(() => {
		platform.canPrintDirectly.mockReturnValue(true);
		platform.listPrinters.mockResolvedValue([{ name: 'Акты HP', is_default: true }]);
		platform.printFormat.mockResolvedValue('pdf');
		platform.printPdf.mockResolvedValue(undefined);
		documents.documentPdf.mockResolvedValue({
			blob: new Blob(['%PDF'], { type: 'application/pdf' }),
			filename: 'akt-priema-5.pdf'
		});
	});

	it('the form stays open: the act goes to the printer and no preview opens over it', async () => {
		intake.open();
		await printAfter(false, intake.isOpen);
		expect(platform.printPdf).toHaveBeenCalledOnce();
		expect(preview.current).toBeNull();
		expect(toasts()).toEqual([['Отправлено на Акты HP', 'success']]);
	});

	it('the form stays open and there is no printer: a notice, still no preview', async () => {
		platform.listPrinters.mockResolvedValue([]);
		intake.open();
		await printAfter(false, intake.isOpen);
		expect(preview.current).toBeNull();
		expect(toasts()).toEqual([['Принтеры не найдены', 'danger']]);
	});

	it('a list of printers that could not be read is a failure, not «Принтеры не найдены»', async () => {
		platform.listPrinters.mockRejectedValue(new Error('lpstat не ответил'));
		intake.open();
		await printAfter(false, intake.isOpen);
		expect(platform.printPdf).not.toHaveBeenCalled();
		expect(preview.current).toBeNull();
		const messages = toasts().map(([message]) => message);
		expect(messages).toHaveLength(1);
		expect(messages[0]).toContain('lpstat не ответил');
		expect(messages[0]).not.toContain('Принтеры не найдены');
	});
});
