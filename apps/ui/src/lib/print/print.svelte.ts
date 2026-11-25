// Printing a document (plan 6): the desktop program prints on the printer of its kind (acts, labels) of
// this computer; a browser, or a computer without a printer, shows the preview — the pages and «Сохранить
// PDF». A failure is a notice with «Предпросмотр»: printing never throws at whoever asked for it (an
// intake, an issue: their order is saved already).

import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import { ApiError, errorMessage } from '#lib/api/client.js';
import {
	documentBatchPdf,
	documentPages,
	documentPdf,
	type DocumentTarget,
	type PdfOptions
} from '#lib/api/documents.js';
import { onSessionReset } from '#lib/app/session-reset.js';
import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
import { layers } from '#lib/design/layers.svelte.js';
import { normalizeSearch } from '#lib/format/search.js';
import { ru } from '#lib/i18n/ru.js';
import { downloadBlob } from '#lib/platform/download.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import {
	canPrintDirectly,
	listPrinters,
	printFormat,
	printPages,
	printPdf,
	type PrintFormat
} from './platform.js';
import { printSettings } from './settings.svelte.js';

/** The pages of the preview: enough for a screen. */
export const PREVIEW_DPI = 110;

/**
 * Acts on A4 are made at this resolution: the barcode fitted to its dots, the pages of Windows rendered at
 * it (it divides 600 and 1200). The resolution of «Печать» is the label printer's, as its offset is.
 */
export const ACT_DPI = 300;

/**
 * The sentence about acts under the label printer's resolution (`ru.help.print.dpi`): Windows prints their
 * pages at `ACT_DPI`; CUPS gets the vector PDF, and `ACT_DPI` is only the grid of the barcode. A browser
 * prints through a PDF too. In the program, until it has said what the system prints (and if it never
 * does), a neutral sentence: neither «PDF» nor 300 dpi is promised for what is not known.
 */
export const actDpiHelp = (format: PrintFormat | null, inProgram = false): string => {
	if (format === 'png') return ru.help.print.dpiActsPng;
	if (format === null && inProgram) return ru.help.print.dpiActsNeutral;
	return ru.help.print.dpiActsPdf;
};

/** Printed on the label printer in the label's size; the rest are acts on A4. */
export const isLabel = (kind: DocumentKind): boolean =>
	kind === 'sticker' || kind === 'calibration' || kind === 'cell_sticker';

/** Of an order: everything but the blank act and the calibration label (`DocumentKind::for_order`). */
export const forOrder = (kind: DocumentKind): boolean =>
	kind !== 'blank_intake_act' && kind !== 'calibration' && kind !== 'cell_sticker';

export const documentName = (kind: DocumentKind): string => ru.print.documents[kind];

/** The title of a document: of the order, a sample of a document of an order, or the document itself. */
export function documentTitle(kind: DocumentKind, target: DocumentTarget): string {
	const name = documentName(kind);
	if (kind === 'cell_sticker' && target.rackId !== undefined)
		return target.shelf !== undefined && target.cell !== undefined
			? ru.print.cellSticker(target.code ?? '')
			: ru.print.rackStickers(target.code ?? '');
	if (target.orderId !== undefined) return ru.print.ofOrder(name, target.orderId);
	return forOrder(kind) ? ru.print.sample(name) : name;
}

/** The printer of a document, its copies and, for a label, the size of the paper. */
export function printTarget(kind: DocumentKind): {
	printer: string | null;
	copies: number;
	mediaMm: [number, number] | null;
} {
	return isLabel(kind)
		? {
				printer: printSettings.labelPrinter,
				copies: printSettings.labelCopies,
				mediaMm: printSettings.labelMm
			}
		: { printer: printSettings.actPrinter, copies: printSettings.actCopies, mediaMm: null };
}

/** The resolution a document is printed at: of the label printer for a label, `ACT_DPI` for an act. */
export const printerDpi = (kind: DocumentKind): number =>
	isLabel(kind) ? printSettings.dpi : ACT_DPI;

/**
 * How the server makes it: the barcode fitted to the printer's dots (a preview too, so it shows what is
 * printed); a label in its size, moved by its offset. Acts on A4 get neither.
 */
export function documentOptions(kind: DocumentKind): PdfOptions {
	return isLabel(kind)
		? {
				printerDpi: printerDpi(kind),
				labelMm: printSettings.labelMm,
				offsetMm: printSettings.offsetMm
			}
		: { printerDpi: printerDpi(kind) };
}

/**
 * The status «Принять по гарантии» sets, found by its name as `Core::accept_warranty_case` finds it
 * (`crates/mw-core/src/orders/update.rs`: `normalize_search` of the name, mirrored by `normalizeSearch`).
 */
const WARRANTY_STATUS = normalizeSearch('Принят по гарантии');

/**
 * The act printed after «Выдать» (spec 6.4): the issue act after warranty when the order is issued from
 * «Принят по гарантии», the ordinary one from any other status — after «Вернуть в работу» too, though the
 * order keeps its first issue date then. `statusBefore`: the name of the status before this issue.
 */
export const issueKind = (statusBefore: string | undefined): DocumentKind =>
	statusBefore !== undefined && normalizeSearch(statusBefore) === WARRANTY_STATUS
		? 'warranty_issue_act'
		: 'issue_act';

/**
 * Ctrl/⌘P: an order not issued yet prints its intake act, an issued one its issue act. An order issued
 * again after a warranty case gets the ordinary one too: the order itself does not record a past warranty
 * case (only its history does), so the act after warranty is in «Печать ▾».
 */
export const shortcutKind = (issued: boolean): DocumentKind =>
	issued ? 'issue_act' : 'intake_act';

/** «Печать ▾» of an order: the acts and the sticker; the warranty acts once it has been issued. */
export function menuDocuments(wasIssued: boolean): DocumentKind[] {
	return [
		'intake_act',
		'issue_act',
		'sticker',
		...(wasIssued ? (['warranty_intake_act', 'warranty_issue_act'] as const) : [])
	];
}

/** The documents of an order a batch can be made of (the menu of the selection bar). */
export const BATCH_DOCUMENTS: readonly DocumentKind[] = [
	'intake_act',
	'issue_act',
	'warranty_intake_act',
	'warranty_issue_act',
	'sticker'
];

/** A document for the preview, and the others of the same order it can switch to. */
export interface PreviewRequest {
	kind: DocumentKind;
	target: DocumentTarget;
	kinds: readonly DocumentKind[];
}

/**
 * What a document is of, as a key: `o<order>`, `r<rack>.<shelf>.<cell>` for a cell, `r<rack>` for a whole
 * rack, empty for a document of nothing (the blank act, a sample). The queue of the preview and the jobs
 * being sent tell their documents apart by it.
 */
export function targetKey(t: DocumentTarget): string {
	if (t.orderId !== undefined) return `o${t.orderId}`;
	if (t.rackId === undefined) return '';
	return t.shelf !== undefined && t.cell !== undefined
		? `r${t.rackId}.${t.shelf}.${t.cell}`
		: `r${t.rackId}`;
}

const sameRequest = (a: PreviewRequest, b: PreviewRequest) =>
	a.kind === b.kind && targetKey(a.target) === targetKey(b.target);

/**
 * The preview of the program's frame: one at a time; the documents asked meanwhile (the sticker after the
 * act of an intake) wait and come one after another.
 */
class Preview {
	#queue = $state.raw<PreviewRequest[]>([]);

	get current(): PreviewRequest | null {
		return this.#queue[0] ?? null;
	}

	open(request: PreviewRequest): void {
		if (this.#queue.some((r) => sameRequest(r, request))) return;
		this.#queue = [...this.#queue, request];
	}

	/** The one shown goes; the next one waiting comes. */
	close(): void {
		this.#queue = this.#queue.slice(1);
	}

	reset(): void {
		this.#queue = [];
	}
}

export const preview = new Preview();

onSessionReset(() => preview.reset());

/** The preview of a document; `kinds` — the documents of the same order it can switch to. */
export function openPreview(
	kind: DocumentKind,
	target: DocumentTarget,
	kinds: readonly DocumentKind[] = [kind]
): void {
	preview.open({ kind, target, kinds: kinds.includes(kind) ? kinds : [kind, ...kinds] });
}

/** The pages of the preview, the barcode as it is printed. */
export function previewPages(
	kind: DocumentKind,
	target: DocumentTarget,
	signal?: AbortSignal
): Promise<DocumentPages> {
	return documentPages(kind, target, { ...documentOptions(kind), dpi: PREVIEW_DPI }, signal);
}

/** The options of a document with the pages chosen in the preview; none chosen — the whole document. */
const withPages = <T extends object>(opts: T, pages: string | undefined): T =>
	pages === undefined ? opts : { ...opts, pages };

/**
 * «Сохранить PDF»: the file under the name the server gave it; the desktop program keeps it in Downloads.
 * `pages`: only those («1,3-4», from the preview).
 */
export async function savePdf(
	kind: DocumentKind,
	target: DocumentTarget,
	pages?: string
): Promise<void> {
	const { blob, filename } = await documentPdf(
		kind,
		target,
		withPages(documentOptions(kind), pages)
	);
	downloadBlob(filename, blob);
}

/** «Сохранить PDF» of several orders: one file, a document of each; a failure (no such order) throws. */
export async function savePdfBatch(kind: DocumentKind, orderIds: number[]): Promise<void> {
	const { blob, filename } = await documentBatchPdf(kind, orderIds, documentOptions(kind));
	downloadBlob(filename, blob);
}

/** Bytes in base64, a piece at a time: `String.fromCharCode` takes only so many arguments. */
async function base64Of(blob: Blob): Promise<string> {
	const bytes = new Uint8Array(await blob.arrayBuffer());
	const CHUNK = 0x8000;
	let binary = '';
	for (let i = 0; i < bytes.length; i += CHUNK)
		binary += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
	return btoa(binary);
}

/** No printer to print on: none in the system (`found` false), or none chosen and no default one. */
class NoPrinter extends Error {
	constructor(readonly found: boolean) {
		super();
	}
}

async function printerName(chosen: string | null): Promise<string> {
	const printers = await listPrinters();
	if (printers.length === 0) throw new NoPrinter(false);
	const name = chosen ?? printers.find((p) => p.is_default)?.name;
	if (name === undefined) throw new NoPrinter(true);
	return name;
}

/**
 * The document to the printer in the format of this system: a PDF to CUPS, pages of PNG to Windows. The
 * server picks the pages of `selection` for both, so the commands of the program print what they get.
 */
async function send(
	kind: DocumentKind,
	target: DocumentTarget,
	printer: string,
	selection?: string
): Promise<void> {
	const { copies, mediaMm } = printTarget(kind);
	const opts = withPages(documentOptions(kind), selection);
	if ((await printFormat()) === 'png') {
		const pages = await documentPages(kind, target, { ...opts, dpi: printerDpi(kind) });
		await printPages({
			printer,
			pagesBase64: pages.pages,
			dpi: pages.dpi,
			widthMm: pages.width_mm,
			heightMm: pages.height_mm,
			copies
		});
		return;
	}
	const { blob } = await documentPdf(kind, target, opts);
	await printPdf({ printer, pdfBase64: await base64Of(blob), copies, mediaMm });
}

const noPrinter = (e: unknown) =>
	e instanceof NoPrinter || (e instanceof Error && e.message === ru.print.noDefaultPrinter);

/** Why there is no printer: none in the system, or none chosen and no default one. */
const noPrinterText = (e: unknown) =>
	e instanceof NoPrinter && !e.found ? ru.print.noPrintersFound : ru.print.noPrinterChosen;

/** The notice of a document asked over a form: no printer, and the way to «Настройки → Печать». */
const showNoPrinter = (message: string) =>
	toast.show({
		message,
		tone: 'danger',
		action: { label: ru.print.printSettings, run: () => goto(resolve('settings/print')) }
	});

function failureText(e: unknown): string {
	const text = e instanceof ApiError ? errorMessage(e) : e instanceof Error ? e.message : '';
	return text ? ru.print.failedWith(text) : ru.print.failed;
}

/**
 * The documents on their way to a printer (`kind:targetKey:pages`). Each press of Ctrl/⌘P or of the menu is
 * a job, and nothing shows until the printer has it: a second press then is dropped, not a second sheet.
 * Other pages of the same document (chosen in the preview) are another job.
 */
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- bookkeeping, nothing renders it
const sending = new Set<string>();

export interface PrintOptions {
	/**
	 * The preview does not open by itself: asked from the preview (its «Печать»), or over a form that stays
	 * open (the intake with another order typed). Without a printer a notice says why and leads to
	 * «Настройки → Печать». A browser has no printer: the caller does not ask then.
	 */
	noPreview?: boolean;
	/** Only these pages («1,3-4»), chosen in the preview; none — every page. */
	pages?: string;
}

/**
 * Prints a document on this computer and says where it went. In a browser, or without a printer, the
 * preview opens instead (with `noPreview` and no printer: a notice); a failure is a notice with its text
 * and «Предпросмотр». The same pages of the same document of the same order asked again while they are
 * being sent are dropped. Never throws.
 */
export async function printDocument(
	kind: DocumentKind,
	target: DocumentTarget,
	opts: PrintOptions = {}
): Promise<void> {
	const noPrinterNotice = await printOne(kind, target, opts);
	if (noPrinterNotice !== null) showNoPrinter(noPrinterNotice);
}

/** `printDocument` without the notice about a missing printer: its text is returned for the caller to show. */
async function printOne(
	kind: DocumentKind,
	target: DocumentTarget,
	opts: PrintOptions
): Promise<string | null> {
	if (!canPrintDirectly()) {
		openPreview(kind, target);
		return null;
	}
	const job = `${kind}:${targetKey(target)}:${opts.pages ?? ''}`;
	if (sending.has(job)) return null;
	sending.add(job);
	try {
		const printer = await printerName(printTarget(kind).printer);
		await send(kind, target, printer, opts.pages);
		toast.show({ message: ru.print.sent(printer), tone: 'success' });
	} catch (e) {
		if (noPrinter(e)) {
			if (opts.noPreview) return noPrinterText(e);
			openPreview(kind, target);
			return null;
		}
		toast.show({
			message: failureText(e),
			tone: 'danger',
			action: { label: ru.print.preview, run: () => openPreview(kind, target) }
		});
	} finally {
		sending.delete(job);
	}
	return null;
}

/**
 * Several documents of one order (the act and the sticker of an intake), one after another. Without a
 * printer over a form that stays open the notice is one: the same words are not told twice.
 */
export async function printDocuments(
	kinds: readonly DocumentKind[],
	target: DocumentTarget,
	opts: PrintOptions = {}
): Promise<void> {
	const notices: string[] = [];
	for (const kind of kinds) {
		const notice = await printOne(kind, target, opts);
		if (notice !== null && !notices.includes(notice)) notices.push(notice);
	}
	for (const notice of notices) showNoPrinter(notice);
}

/**
 * Ctrl/⌘P of an open order (the panel, the full card): `run` prints its act. Works in fields; over a
 * dialog or a menu the key does nothing, nor do the repeats of a held key. Either way it is taken: the
 * browser's printing of the page never opens there. While `when` says false the key goes to an older
 * registration (the card under the panel).
 */
export function printShortcut(run: () => void, when: () => boolean = () => true): () => void {
	return registerHotkey(
		'Mod+P',
		(e) => {
			if (!e.repeat && !layers.blocking) run();
		},
		{ allowInFields: true, allowWithOverlay: true, when }
	);
}

/** The most orders of one batch: the server takes no more (`order_ids/too_many`). */
export const BULK_PRINT_LIMIT = 100;

/** How many numbers of the failed a notice names; the rest are «и ещё N». */
const FAILED_SHOWN = 5;

/**
 * What a batch came to. `printer` is null when nothing was sent: no printer, or another batch was running
 * (that call is dropped and tells nothing).
 */
export interface BatchPrintResult {
	sent: number;
	failed: { number: string; text: string }[];
	stopped: boolean;
	printer: string | null;
}

/**
 * The batch being printed: how far it is, kept here (not in the list of orders) so the list shows it again
 * and can stop it after the user has left it and come back. Null while none runs.
 */
class RunningBatch {
	progress = $state.raw<{ done: number; total: number } | null>(null);
	stopper: AbortController | null = null;
}

export const batch = new RunningBatch();

/** Stops the batch running after the order being sent; nothing when none runs. */
export function stopBatch(): void {
	batch.stopper?.abort();
}

let batchRunning = false;

/**
 * The documents of several orders to the printer of their kind, one order after another (`send`). The
 * printer is chosen once; a failure of one order is noted and the rest go on. `signal` stops it after the
 * order being sent, never in the middle of one. One batch at a time: a second call is dropped (nothing
 * sent). Without a printer one notice says so. Never throws. `number` is how the order is named in a notice.
 */
export async function printBatch(
	kind: DocumentKind,
	orders: { id: number; number: string }[],
	opts: { onprogress?: (done: number, total: number) => void; signal?: AbortSignal } = {}
): Promise<BatchPrintResult> {
	const result: BatchPrintResult = { sent: 0, failed: [], stopped: false, printer: null };
	if (batchRunning) return result;
	batchRunning = true;
	const stopper = new AbortController();
	const stopped = () => stopper.signal.aborted || opts.signal?.aborted === true;
	batch.stopper = stopper;
	batch.progress = { done: 0, total: orders.length };
	try {
		let printer: string;
		try {
			printer = await printerName(printTarget(kind).printer);
		} catch (e) {
			if (noPrinter(e)) showNoPrinter(noPrinterText(e));
			else toast.show({ message: failureText(e), tone: 'danger' });
			return result;
		}
		result.printer = printer;
		for (const [i, order] of orders.entries()) {
			try {
				await send(kind, { orderId: order.id }, printer);
				result.sent++;
			} catch (e) {
				result.failed.push({ number: order.number, text: failureText(e) });
			}
			batch.progress = { done: i + 1, total: orders.length };
			try {
				opts.onprogress?.(i + 1, orders.length);
			} catch {
				// A broken screen does not stop the sheets.
			}
			if (stopped() && i + 1 < orders.length) {
				result.stopped = true;
				break;
			}
		}
		return result;
	} finally {
		batchRunning = false;
		batch.progress = null;
		batch.stopper = null;
	}
}

/**
 * The notices of a finished batch: where it went and how many, after failures and «Остановить» too (the
 * user knows what is in the printer's tray), and the numbers of those that did not print.
 */
export function showBatchResult(r: BatchPrintResult): void {
	if (r.printer !== null && r.sent > 0)
		toast.show({ message: ru.print.batchSent(r.printer, r.sent), tone: 'success' });
	if (r.failed.length > 0) {
		const shown = r.failed.slice(0, FAILED_SHOWN).map((f) => f.number);
		toast.show({
			message: ru.print.batchFailed(shown, r.failed.length - shown.length),
			tone: 'danger'
		});
	}
}
