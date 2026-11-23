import { ru } from '#lib/i18n/ru.js';
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
const documents = vi.hoisted(() => ({
	documentPdf: vi.fn(),
	documentPages: vi.fn(),
	documentBatchPdf: vi.fn()
}));
vi.mock('#lib/api/documents.js', () => documents);
const navigation = vi.hoisted(() => ({ goto: vi.fn(async () => {}) }));
vi.mock('$app/navigation', () => navigation);
vi.mock('$app/paths', () => ({ resolve: (p: string) => `/${p}` }));

import {
	ACT_DPI,
	actDpiHelp,
	documentOptions,
	issueKind,
	menuDocuments,
	openPreview,
	preview,
	printDocument,
	printDocuments,
	batch,
	printBatch,
	stopBatch,
	printShortcut,
	printTarget,
	savePdf,
	savePdfBatch,
	showBatchResult,
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
	const ACTS = [
		'intake_act',
		'issue_act',
		'warranty_intake_act',
		'warranty_issue_act',
		'blank_intake_act'
	] as const;
	const LABELS = ['sticker', 'calibration'] as const;

	beforeEach(() =>
		printSettings.set({
			actPrinter: 'Акты HP',
			labelPrinter: 'Xprinter XP-365B',
			actCopies: 2,
			labelCopies: 3,
			labelMm: [40, 30],
			offsetMm: [1.5, -1],
			dpi: 600
		})
	);

	it('acts go to the act printer on A4 at 300 dpi, whatever the label printer’s resolution', () => {
		expect(ACT_DPI).toBe(300);
		for (const dpi of [203, 600] as const) {
			printSettings.set({ dpi });
			for (const kind of ACTS) {
				expect(printTarget(kind)).toEqual({ printer: 'Акты HP', copies: 2, mediaMm: null });
				expect(documentOptions(kind)).toEqual({ printerDpi: 300 });
			}
		}
	});

	it('the sticker and the calibration go to the label printer in its size, offset and resolution', () => {
		for (const dpi of [203, 600] as const) {
			printSettings.set({ dpi });
			for (const kind of LABELS) {
				expect(printTarget(kind)).toEqual({
					printer: 'Xprinter XP-365B',
					copies: 3,
					mediaMm: [40, 30]
				});
				expect(documentOptions(kind)).toEqual({
					printerDpi: dpi,
					labelMm: [40, 30],
					offsetMm: [1.5, -1]
				});
			}
		}
	});

	it('prints a PDF on that printer and says where it went', async () => {
		await printDocument('sticker', { orderId: 42 });
		expect(documents.documentPdf).toHaveBeenCalledWith(
			'sticker',
			{ orderId: 42 },
			{ printerDpi: 600, labelMm: [40, 30], offsetMm: [1.5, -1] }
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

	it('a PDF larger than one piece of the base64 encoder goes whole', async () => {
		// 0x8000 bytes a piece: three pieces and a bit, every byte value.
		const bytes = Uint8Array.from({ length: 3 * 0x8000 + 17 }, (_, i) => (i * 7 + 3) % 256);
		documents.documentPdf.mockResolvedValue({
			blob: new Blob([bytes], { type: 'application/pdf' }),
			filename: 'akt-priema-42.pdf'
		});
		await printDocument('intake_act', { orderId: 42 });
		const sent = platform.printPdf.mock.calls[0]![0].pdfBase64 as string;
		expect(Uint8Array.from(atob(sent), (c) => c.charCodeAt(0))).toEqual(bytes);
	});

	it('Windows: an act in pages of PNG at 300 dpi, its barcode fitted to the same dots', async () => {
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

	it('Windows: a label in pages of PNG at the label printer’s resolution', async () => {
		platform.printFormat.mockResolvedValue('png');
		documents.documentPages.mockResolvedValue({ ...PAGES, dpi: 600 });
		await printDocument('sticker', { orderId: 42 });
		expect(documents.documentPages).toHaveBeenCalledWith(
			'sticker',
			{ orderId: 42 },
			{ printerDpi: 600, labelMm: [40, 30], offsetMm: [1.5, -1], dpi: 600 }
		);
		expect(platform.printPages).toHaveBeenCalledWith(
			expect.objectContaining({ printer: 'Xprinter XP-365B', dpi: 600, copies: 3 })
		);
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

	it('asked from the preview with no printer in the system: «Принтеры не найдены»', async () => {
		platform.listPrinters.mockResolvedValue([]);
		printSettings.set({ actPrinter: 'Акты HP' });
		await printDocument('intake_act', { orderId: 7 }, { noPreview: true });
		expect(preview.current).toBeNull();
		expect(toasts()).toEqual([['Принтеры не найдены', 'danger', 'Настройки печати']]);
	});

	it('asked from the preview (no preview): a notice with «Настройки печати», the preview does not come back', async () => {
		platform.listPrinters.mockResolvedValue([{ name: 'Акты HP', is_default: false }]);
		await printDocument('intake_act', { orderId: 7 }, { noPreview: true });
		expect(preview.current).toBeNull();
		expect(platform.printPdf).not.toHaveBeenCalled();
		expect(toasts()).toEqual([
			['Принтер не выбран, а принтера по умолчанию в системе нет', 'danger', 'Настройки печати']
		]);
		await toast.items[0]!.action!.run();
		expect(navigation.goto).toHaveBeenCalledWith('/settings/print');
		expect(preview.current).toBeNull();
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

	it('over a form that stays open: without a printer a notice, never a preview over it', async () => {
		platform.listPrinters.mockResolvedValue([{ name: 'Акты HP', is_default: false }]);
		await printDocuments(['intake_act', 'sticker'], { orderId: 9 }, { noPreview: true });
		expect(preview.current).toBeNull();
		expect(platform.printPdf).not.toHaveBeenCalled();
		expect(toasts()).toEqual([
			['Принтер не выбран, а принтера по умолчанию в системе нет', 'danger', 'Настройки печати']
		]);
	});

	it('over a form that stays open: a computer without printers is one notice «Принтеры не найдены», not two', async () => {
		platform.listPrinters.mockResolvedValue([]);
		await printDocuments(['intake_act', 'sticker'], { orderId: 9 }, { noPreview: true });
		expect(preview.current).toBeNull();
		expect(toasts()).toEqual([['Принтеры не найдены', 'danger', 'Настройки печати']]);
	});

	it('over a form that stays open: different reasons for the act and the sticker are told once each', async () => {
		platform.listPrinters.mockResolvedValue([{ name: 'Акты HP', is_default: false }]);
		printSettings.set({ actPrinter: 'Акты HP' });
		await printDocuments(['intake_act', 'sticker'], { orderId: 9 }, { noPreview: true });
		expect(platform.printPdf).toHaveBeenCalledOnce();
		expect(toasts()).toEqual([
			['Отправлено на Акты HP', 'success', undefined],
			['Принтер не выбран, а принтера по умолчанию в системе нет', 'danger', 'Настройки печати']
		]);
	});

	it('over a form that stays open: with a printer each goes to it', async () => {
		await printDocuments(['intake_act', 'sticker'], { orderId: 9 }, { noPreview: true });
		expect(platform.printPdf.mock.calls.map(([a]) => a.printer)).toEqual(['Акты HP', 'Акты HP']);
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
			{ printerDpi: 300 }
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

describe('the act of an issue', () => {
	it('is the one after warranty when the order is issued from «Принят по гарантии»', () => {
		expect(issueKind('Принят по гарантии')).toBe('warranty_issue_act');
	});

	it('finds that status as the server does: case and spaces do not matter (normalize_search)', () => {
		for (const name of ['ПРИНЯТ ПО ГАРАНТИИ', 'принят по Гарантии', '  Принят  по\tгарантии ']) {
			expect(issueKind(name), name).toBe('warranty_issue_act');
		}
	});

	it('is the ordinary issue act from any other status, or an unknown one', () => {
		for (const name of ['Готов', 'Принят', 'В работе', 'Принят по гарантии (старый)', '']) {
			expect(issueKind(name), name).toBe('issue_act');
		}
		expect(issueKind(undefined)).toBe('issue_act');
	});
});

describe('the sentence about acts under the printer resolution', () => {
	it('Windows prints their pages at 300 dpi; CUPS and a browser send a PDF with the barcode on that grid', () => {
		expect(actDpiHelp('png')).toBe(
			'Акты печатаются в 300 точек на дюйм. Эта настройка на них не влияет.'
		);
		for (const format of ['pdf', null] as const)
			expect(actDpiHelp(format)).toBe(
				'Акты уходят на принтер в PDF; штрихкод выровнен по сетке 300 точек на дюйм. Эта настройка на них не влияет.'
			);
	});
});

describe('the sentence about acts while the format of the program is not known', () => {
	it('in the program, without an answer: no word about a PDF', () => {
		expect(actDpiHelp(null, true)).toBe(ru.help.print.dpiActsNeutral);
		expect(actDpiHelp(null, true)).not.toContain('PDF');
		// The answer wins; a browser prints through a PDF.
		expect(actDpiHelp('png', true)).toBe(ru.help.print.dpiActsPng);
		expect(actDpiHelp('pdf', true)).toBe(ru.help.print.dpiActsPdf);
		expect(actDpiHelp(null, false)).toBe(ru.help.print.dpiActsPdf);
	});
});

describe('printing a batch of orders', () => {
	const ORDERS = [
		{ id: 11, number: '№ 11' },
		{ id: 12, number: '№ 12' },
		{ id: 13, number: '№ 13' }
	];
	const sentIds = () => documents.documentPdf.mock.calls.map((c) => c[1].orderId);

	it('printBatch sends each order once, in order', async () => {
		const progress: [number, number][] = [];
		const r = await printBatch('sticker', ORDERS, { onprogress: (d, t) => progress.push([d, t]) });
		expect(sentIds()).toEqual([11, 12, 13]);
		expect(platform.printPdf).toHaveBeenCalledTimes(3);
		expect(r).toEqual({ sent: 3, failed: [], stopped: false, printer: 'Акты HP' });
		expect(progress).toEqual([
			[1, 3],
			[2, 3],
			[3, 3]
		]);
	});

	it('printBatch goes on after a failure', async () => {
		platform.printPdf
			.mockResolvedValueOnce(undefined)
			.mockRejectedValueOnce(new Error('Бумага закончилась'))
			.mockResolvedValueOnce(undefined);
		const r = await printBatch('intake_act', ORDERS, {});
		expect(r.sent).toBe(2);
		expect(r.failed).toEqual([
			{ number: '№ 12', text: 'Не удалось напечатать. Бумага закончилась' }
		]);
		expect(r.stopped).toBe(false);
		expect(sentIds()).toEqual([11, 12, 13]);
	});

	it('printBatch stops after the current order', async () => {
		const stop = new AbortController();
		const r = await printBatch('intake_act', ORDERS, {
			signal: stop.signal,
			onprogress: () => stop.abort()
		});
		expect(r).toEqual({ sent: 1, failed: [], stopped: true, printer: 'Акты HP' });
		expect(sentIds()).toEqual([11]);
	});

	it('printBatch without a printer tells it once', async () => {
		platform.listPrinters.mockResolvedValue([]);
		const r = await printBatch('intake_act', ORDERS, {});
		expect(documents.documentPdf).not.toHaveBeenCalled();
		expect(platform.printPdf).not.toHaveBeenCalled();
		expect(r).toEqual({ sent: 0, failed: [], stopped: false, printer: null });
		expect(toasts()).toEqual([['Принтеры не найдены', 'danger', 'Настройки печати']]);
	});

	it('a second printBatch while one runs is dropped', async () => {
		const first = printBatch('intake_act', ORDERS, {});
		const second = await printBatch('intake_act', ORDERS, {});
		expect(second).toEqual({ sent: 0, failed: [], stopped: false, printer: null });
		expect((await first).sent).toBe(3);
		expect(sentIds()).toEqual([11, 12, 13]);
		// Once it is over the next one runs.
		expect((await printBatch('intake_act', ORDERS, {})).sent).toBe(3);
	});

	it('the running batch is in module state: its progress shows, stopBatch stops after the current order', async () => {
		expect(batch.progress).toBeNull();
		const seen: ({ done: number; total: number } | null)[] = [];
		const r = await printBatch('intake_act', ORDERS, {
			onprogress: () => {
				seen.push(batch.progress);
				stopBatch();
			}
		});
		expect(seen).toEqual([{ done: 1, total: 3 }]);
		expect(r).toMatchObject({ sent: 1, stopped: true });
		expect(sentIds()).toEqual([11]);
		expect(batch.progress).toBeNull();
		// Nothing running: stopping does nothing, and the next batch is not affected.
		stopBatch();
		expect((await printBatch('intake_act', ORDERS, {})).sent).toBe(3);
	});

	it('the notices: how many were sent, and the numbers of the failed (five, then «и ещё N»)', () => {
		// At most three notices are on the screen at once: each batch is looked at by itself.
		const shown = (r: Parameters<typeof showBatchResult>[0]) => {
			toast.clear();
			showBatchResult(r);
			return toasts();
		};
		const failed = Array.from({ length: 7 }, (_, i) => ({ number: `№ ${i + 1}`, text: '' }));
		expect(shown({ sent: 12, failed: [], stopped: false, printer: 'Акты HP' })).toEqual([
			['Отправлено на «Акты HP»: 12', 'success', undefined]
		]);
		expect(
			shown({ sent: 3, failed: failed.slice(0, 2), stopped: false, printer: 'Акты HP' })
		).toEqual([
			['Отправлено на «Акты HP»: 3', 'success', undefined],
			['Не напечатаны: № 1, № 2', 'danger', undefined]
		]);
		expect(shown({ sent: 0, failed, stopped: false, printer: 'Акты HP' })).toEqual([
			['Не напечатаны: № 1, № 2, № 3, № 4, № 5 и ещё 2', 'danger', undefined]
		]);
		expect(shown({ sent: 0, failed: [], stopped: false, printer: null })).toEqual([]);
	});

	it('a batch stopped after a failure tells both: how many went to the printer, and which did not', async () => {
		platform.printPdf
			.mockResolvedValueOnce(undefined)
			.mockRejectedValueOnce(new Error('Бумага закончилась'))
			.mockResolvedValueOnce(undefined);
		let done = 0;
		const r = await printBatch('intake_act', [...ORDERS, { id: 14, number: '№ 14' }], {
			onprogress: () => {
				if (++done === 3) stopBatch();
			}
		});
		expect(r).toMatchObject({ sent: 2, stopped: true, printer: 'Акты HP' });
		expect(sentIds()).toEqual([11, 12, 13]);
		showBatchResult(r);
		expect(toasts()).toEqual([
			['Отправлено на «Акты HP»: 2', 'success', undefined],
			['Не напечатаны: № 12', 'danger', undefined]
		]);
	});

	it('«Сохранить PDF» of a batch downloads the file under the name the server gave it', async () => {
		documents.documentBatchPdf.mockResolvedValue({
			blob: new Blob(['%PDF']),
			filename: 'naklejka-3.pdf'
		});
		URL.createObjectURL = () => 'blob:pdf';
		URL.revokeObjectURL = () => {};
		const names: string[] = [];
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(function (
			this: HTMLAnchorElement
		) {
			names.push(this.download);
		});
		await savePdfBatch('sticker', [11, 12, 13]);
		expect(documents.documentBatchPdf).toHaveBeenCalledWith(
			'sticker',
			[11, 12, 13],
			documentOptions('sticker')
		);
		expect(names).toEqual(['naklejka-3.pdf']);
		click.mockRestore();
	});
});
