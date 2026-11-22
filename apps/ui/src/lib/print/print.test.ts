import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { __resetHotkeysForTests } from '#lib/design/hotkeys.svelte.js';
import { layers } from '#lib/design/layers.svelte.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { DEFAULT_PRINT_SETTINGS, printSettings } from './settings.svelte.js';

// Printing a document: in the desktop program straight to the printer of its kind (acts, labels), in a
// browser or without a printer the preview. The program and the server are replaced here.
const platform = vi.hoisted(() => ({
	canPrintDirectly: vi.fn<() => boolean>(),
	listPrinters: vi.fn(),
	printFormat: vi.fn(),
	printPdf: vi.fn(),
	printPages: vi.fn()
}));
vi.mock('#lib/print/platform.js', () => platform);
const documents = vi.hoisted(() => ({ documentPdf: vi.fn(), documentPages: vi.fn() }));
vi.mock('#lib/api/documents.js', () => documents);

import {
	documentOptions,
	menuDocuments,
	openPreview,
	preview,
	printDocument,
	printDocuments,
	printShortcut,
	printTarget,
	savePdf,
	shortcutKind
} from './print.svelte.js';

const PRINTERS = [
	{ name: 'Акты HP', is_default: true },
	{ name: 'Xprinter XP-365B', is_default: false }
];
const PAGES = { dpi: 300, width_mm: 58, height_mm: 40, pages: ['iVBORw0KGgo='] };

const settle = async () => {
	for (let i = 0; i < 20; i++) await Promise.resolve();
};

beforeEach(() => {
	platform.canPrintDirectly.mockReturnValue(true);
	platform.listPrinters.mockResolvedValue(PRINTERS);
	platform.printFormat.mockResolvedValue('pdf');
	platform.printPdf.mockResolvedValue(undefined);
	platform.printPages.mockResolvedValue(undefined);
	documents.documentPdf.mockResolvedValue({
		blob: new Blob(['%PDF'], { type: 'application/pdf' }),
		filename: 'akt-priema-42.pdf'
	});
	documents.documentPages.mockResolvedValue(PAGES);
});

afterEach(() => {
	vi.clearAllMocks();
	printSettings.set(DEFAULT_PRINT_SETTINGS);
	localStorage.clear();
	preview.reset();
	toast.clear();
	__resetHotkeysForTests();
});

const toasts = () => toast.items.map((t) => [t.message, t.tone, t.action?.label]);

describe('the printer of a document', () => {
	beforeEach(() =>
		printSettings.set({
			actPrinter: 'Акты HP',
			labelPrinter: 'Xprinter XP-365B',
			actCopies: 2,
			labelCopies: 3,
			labelMm: [40, 30],
			offsetMm: [1.5, -1],
			dpi: 300
		})
	);

	it('acts go to the act printer on A4, without the label and its offset', () => {
		for (const kind of [
			'intake_act',
			'issue_act',
			'warranty_intake_act',
			'warranty_issue_act',
			'blank_intake_act'
		] as const) {
			expect(printTarget(kind)).toEqual({ printer: 'Акты HP', copies: 2, mediaMm: null });
			expect(documentOptions(kind)).toEqual({ printerDpi: 300 });
		}
	});

	it('the sticker and the calibration go to the label printer in the label’s size and offset', () => {
		for (const kind of ['sticker', 'calibration'] as const) {
			expect(printTarget(kind)).toEqual({
				printer: 'Xprinter XP-365B',
				copies: 3,
				mediaMm: [40, 30]
			});
			expect(documentOptions(kind)).toEqual({
				printerDpi: 300,
				labelMm: [40, 30],
				offsetMm: [1.5, -1]
			});
		}
	});

	it('prints a PDF on that printer and says where it went', async () => {
		await printDocument('sticker', { orderId: 42 });
		expect(documents.documentPdf).toHaveBeenCalledWith(
			'sticker',
			{ orderId: 42 },
			{ printerDpi: 300, labelMm: [40, 30], offsetMm: [1.5, -1] }
		);
		expect(platform.printPdf).toHaveBeenCalledWith({
			printer: 'Xprinter XP-365B',
			pdfBase64: btoa('%PDF'),
			copies: 3,
			mediaMm: [40, 30]
		});
		expect(toasts()).toEqual([['Отправлено на Xprinter XP-365B', 'success', undefined]]);
		expect(preview.current).toBeNull();
	});

	it('Windows: pages of PNG in the printer’s resolution, the barcode fitted to the same dots', async () => {
		platform.printFormat.mockResolvedValue('png');
		await printDocument('intake_act', { orderId: 42 });
		expect(documents.documentPdf).not.toHaveBeenCalled();
		expect(documents.documentPages).toHaveBeenCalledWith(
			'intake_act',
			{ orderId: 42 },
			{ printerDpi: 300, dpi: 300 }
		);
		expect(platform.printPages).toHaveBeenCalledWith({
			printer: 'Акты HP',
			pagesBase64: PAGES.pages,
			dpi: 300,
			widthMm: 58,
			heightMm: 40,
			copies: 2
		});
		expect(toasts()).toEqual([['Отправлено на Акты HP', 'success', undefined]]);
	});

	it('«Принтер по умолчанию» is the system’s default printer, named in the notice', async () => {
		printSettings.set({ actPrinter: null });
		await printDocument('issue_act', { orderId: 5 });
		expect(platform.printPdf).toHaveBeenCalledWith(
			expect.objectContaining({ printer: 'Акты HP', mediaMm: null, copies: 2 })
		);
		expect(toasts()).toEqual([['Отправлено на Акты HP', 'success', undefined]]);
	});
});

describe('without a printer: the preview', () => {
	it('in a browser the preview opens, nothing is asked of the program or the server', async () => {
		platform.canPrintDirectly.mockReturnValue(false);
		await printDocument('intake_act', { orderId: 42 });
		expect(preview.current).toEqual({
			kind: 'intake_act',
			target: { orderId: 42 },
			kinds: ['intake_act']
		});
		expect(platform.listPrinters).not.toHaveBeenCalled();
		expect(documents.documentPdf).not.toHaveBeenCalled();
		expect(toast.items).toEqual([]);
	});

	it('a computer without printers opens the preview instead of an error', async () => {
		platform.listPrinters.mockResolvedValue([]);
		printSettings.set({ actPrinter: 'Акты HP' });
		await printDocument('issue_act', { orderId: 7 });
		expect(preview.current?.kind).toBe('issue_act');
		expect(platform.printPdf).not.toHaveBeenCalled();
		expect(toast.items).toEqual([]);
	});

	it('no default printer in the system: the preview too', async () => {
		platform.listPrinters.mockResolvedValue([{ name: 'Акты HP', is_default: false }]);
		await printDocument('sticker', { orderId: 7 });
		expect(preview.current?.kind).toBe('sticker');
		expect(platform.printPdf).not.toHaveBeenCalled();
	});

	it('the program saying «Принтер по умолчанию не задан» is the same case', async () => {
		printSettings.set({ actPrinter: 'Акты HP' });
		platform.printPdf.mockRejectedValue(new Error('Принтер по умолчанию не задан'));
		await printDocument('intake_act', { orderId: 7 });
		expect(preview.current?.kind).toBe('intake_act');
		expect(toast.items).toEqual([]);
	});
});

describe('a failure', () => {
	it('of the printer is told with its text, and «Предпросмотр» opens the document', async () => {
		printSettings.set({ actPrinter: 'Акты HP' });
		platform.printPdf.mockRejectedValue(new Error('Принтер «Акты HP» не найден'));
		await printDocument('intake_act', { orderId: 42 });
		expect(toasts()).toEqual([
			['Не удалось напечатать. Принтер «Акты HP» не найден', 'danger', 'Предпросмотр']
		]);
		expect(preview.current).toBeNull();
		await toast.items[0]!.action!.run();
		expect(preview.current).toEqual({
			kind: 'intake_act',
			target: { orderId: 42 },
			kinds: ['intake_act']
		});
	});

	it('of the server is told with its message', async () => {
		documents.documentPdf.mockRejectedValue(
			new ApiError(404, 'not_found', 'Заказ не найден', null)
		);
		await printDocument('issue_act', { orderId: 1 });
		expect(toasts()).toEqual([
			['Не удалось напечатать. Заказ не найден', 'danger', 'Предпросмотр']
		]);
	});

	it('never throws: what called it (an intake, an issue) is done already', async () => {
		platform.listPrinters.mockRejectedValue('lpstat: ошибка');
		await expect(printDocument('intake_act', { orderId: 1 })).resolves.toBeUndefined();
		expect(toasts()).toEqual([['Не удалось напечатать', 'danger', 'Предпросмотр']]);
	});
});

describe('one job at a time', () => {
	/** `printPdf` that answers every job sent to it when told to. */
	function slowPrinter() {
		const answers: (() => void)[] = [];
		platform.printPdf.mockImplementation(() => new Promise<void>((r) => answers.push(r)));
		return () => answers.forEach((answer) => answer());
	}

	it('a second press while the document is on its way to the printer sends nothing more', async () => {
		const done = slowPrinter();
		const first = printDocument('intake_act', { orderId: 42 });
		await vi.waitFor(() => expect(platform.printPdf).toHaveBeenCalledOnce());
		const second = printDocument('intake_act', { orderId: 42 });
		await settle();
		done();
		await Promise.all([first, second]);
		expect(platform.printPdf).toHaveBeenCalledOnce();
		expect(documents.documentPdf).toHaveBeenCalledOnce();
		expect(toasts()).toEqual([['Отправлено на Акты HP', 'success', undefined]]);
	});

	it('once it has gone the next press prints again; another document prints meanwhile', async () => {
		const done = slowPrinter();
		const act = printDocument('intake_act', { orderId: 42 });
		await vi.waitFor(() => expect(platform.printPdf).toHaveBeenCalledOnce());
		const sticker = printDocument('sticker', { orderId: 42 });
		const otherOrder = printDocument('intake_act', { orderId: 43 });
		await vi.waitFor(() => expect(platform.printPdf).toHaveBeenCalledTimes(3));
		done();
		await Promise.all([act, sticker, otherOrder]);
		platform.printPdf.mockResolvedValue(undefined);
		await printDocument('intake_act', { orderId: 42 });
		expect(platform.printPdf).toHaveBeenCalledTimes(4);
	});

	it('a failed job does not hold the document back', async () => {
		platform.printPdf.mockRejectedValueOnce(new Error('Принтер «Акты HP» не найден'));
		await printDocument('intake_act', { orderId: 42 });
		await printDocument('intake_act', { orderId: 42 });
		expect(platform.printPdf).toHaveBeenCalledTimes(2);
	});
});

describe('several documents', () => {
	it('print one after another; in a browser their previews follow one another', async () => {
		platform.canPrintDirectly.mockReturnValue(false);
		await printDocuments(['intake_act', 'sticker'], { orderId: 9 });
		expect(preview.current?.kind).toBe('intake_act');
		preview.close();
		expect(preview.current?.kind).toBe('sticker');
		preview.close();
		expect(preview.current).toBeNull();
	});

	it('the same document asked again while its preview is open is not queued twice', () => {
		openPreview('intake_act', { orderId: 9 });
		openPreview('intake_act', { orderId: 9 });
		preview.close();
		expect(preview.current).toBeNull();
	});
});

describe('«Сохранить PDF»', () => {
	it('saves the PDF under the name the server gave it', async () => {
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
		URL.createObjectURL = () => 'blob:pdf';
		URL.revokeObjectURL = () => {};
		const names: string[] = [];
		click.mockImplementation(function (this: HTMLAnchorElement) {
			names.push(this.download);
		});
		await savePdf('intake_act', { orderId: 42 });
		expect(documents.documentPdf).toHaveBeenCalledWith(
			'intake_act',
			{ orderId: 42 },
			{
				printerDpi: 203
			}
		);
		expect(names).toEqual(['akt-priema-42.pdf']);
		click.mockRestore();
	});
});

describe('the menu «Печать»', () => {
	it('has the acts and the sticker; the warranty acts only for an order that was issued', () => {
		expect(menuDocuments(false)).toEqual(['intake_act', 'issue_act', 'sticker']);
		expect(menuDocuments(true)).toEqual([
			'intake_act',
			'issue_act',
			'sticker',
			'warranty_intake_act',
			'warranty_issue_act'
		]);
	});
});

describe('Ctrl/⌘P', () => {
	const press = (repeat = false) => {
		const e = new KeyboardEvent('keydown', {
			key: 'p',
			code: 'KeyP',
			ctrlKey: true,
			repeat,
			bubbles: true,
			cancelable: true
		});
		document.body.dispatchEvent(e);
		return e;
	};

	it('prints the act of the order’s state: the intake act until it is issued, then the issue act', () => {
		expect(shortcutKind(false)).toBe('intake_act');
		expect(shortcutKind(true)).toBe('issue_act');
	});

	it('runs in fields too and takes the key: the browser prints no page', () => {
		const run = vi.fn();
		printShortcut(run);
		const input = document.createElement('input');
		document.body.append(input);
		input.focus();
		const e = new KeyboardEvent('keydown', {
			key: 'p',
			code: 'KeyP',
			ctrlKey: true,
			bubbles: true,
			cancelable: true
		});
		input.dispatchEvent(e);
		expect(run).toHaveBeenCalledOnce();
		expect(e.defaultPrevented).toBe(true);
		input.remove();
	});

	it('over a dialog or a menu does nothing, and the browser still prints no page', () => {
		const run = vi.fn();
		printShortcut(run);
		const close = layers.open('modal', () => {});
		expect(press().defaultPrevented).toBe(true);
		expect(run).not.toHaveBeenCalled();
		close();
		press();
		expect(run).toHaveBeenCalledOnce();
	});

	it('a held key prints once: its repeats are taken, nothing more is printed', () => {
		const run = vi.fn();
		printShortcut(run);
		press();
		const repeats = [press(true), press(true), press(true)];
		expect(run).toHaveBeenCalledOnce();
		expect(repeats.map((e) => e.defaultPrevented)).toEqual([true, true, true]);
	});

	it('while `when` says no, the key goes to an older registration (the card under the panel)', async () => {
		const card = vi.fn();
		const panel = vi.fn();
		let panelOpen = true;
		printShortcut(card, () => !panelOpen);
		printShortcut(panel, () => panelOpen);
		press();
		panelOpen = false;
		press();
		await settle();
		expect([panel.mock.calls.length, card.mock.calls.length]).toEqual([1, 1]);
	});
});
