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
import { canPrintDirectly, listPrinters, printFormat, printPages, printPdf } from './platform.js';
import { printSettings } from './settings.svelte.js';

/** The pages of the preview: enough for a screen. */
export const PREVIEW_DPI = 110;

/**
 * Acts on A4 are made at this resolution: the barcode fitted to its dots, the pages of Windows rendered at
 * it (it divides 600 and 1200). The resolution of «Печать» is the label printer's, as its offset is.
 */
export const ACT_DPI = 300;

/** Printed on the label printer in the label's size; the rest are acts on A4. */
export const isLabel = (kind: DocumentKind): boolean =>
	kind === 'sticker' || kind === 'calibration';

/** Of an order: everything but the blank act and the calibration label (`DocumentKind::for_order`). */
export const forOrder = (kind: DocumentKind): boolean =>
	kind !== 'blank_intake_act' && kind !== 'calibration';

export const documentName = (kind: DocumentKind): string => ru.print.documents[kind];

/** The title of a document: of the order, a sample of a document of an order, or the document itself. */
export function documentTitle(kind: DocumentKind, target: DocumentTarget): string {
	const name = documentName(kind);
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

/** A document for the preview, and the others of the same order it can switch to. */
export interface PreviewRequest {
	kind: DocumentKind;
	target: DocumentTarget;
	kinds: readonly DocumentKind[];
}

const sameRequest = (a: PreviewRequest, b: PreviewRequest) =>
	a.kind === b.kind && a.target.orderId === b.target.orderId;

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

/** «Сохранить PDF»: the file under the name the server gave it; the desktop program keeps it in Downloads. */
export async function savePdf(kind: DocumentKind, target: DocumentTarget): Promise<void> {
	const { blob, filename } = await documentPdf(kind, target, documentOptions(kind));
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

/** No printer to print on: none in the system, or none chosen and no default one. */
class NoPrinter extends Error {}

async function printerName(chosen: string | null): Promise<string> {
	const printers = await listPrinters();
	const name = chosen ?? printers.find((p) => p.is_default)?.name;
	if (printers.length === 0 || name === undefined) throw new NoPrinter();
	return name;
}

/** The document to the printer in the format of this system: a PDF to CUPS, pages of PNG to Windows. */
async function send(kind: DocumentKind, target: DocumentTarget, printer: string): Promise<void> {
	const { copies, mediaMm } = printTarget(kind);
	const opts = documentOptions(kind);
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

function failureText(e: unknown): string {
	const text = e instanceof ApiError ? errorMessage(e) : e instanceof Error ? e.message : '';
	return text ? ru.print.failedWith(text) : ru.print.failed;
}

/**
 * The documents on their way to a printer (`kind:orderId`). Each press of Ctrl/⌘P or of the menu is a job,
 * and nothing shows until the printer has it: a second press then is dropped, not a second sheet.
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
}

/**
 * Prints a document on this computer and says where it went. In a browser, or without a printer, the
 * preview opens instead (with `noPreview` and no printer: a notice); a failure is a notice with its text
 * and «Предпросмотр». The same document of the same order asked again while it is being sent is dropped.
 * Never throws.
 */
export async function printDocument(
	kind: DocumentKind,
	target: DocumentTarget,
	opts: PrintOptions = {}
): Promise<void> {
	if (!canPrintDirectly()) {
		openPreview(kind, target);
		return;
	}
	const job = `${kind}:${target.orderId ?? ''}`;
	if (sending.has(job)) return;
	sending.add(job);
	try {
		const printer = await printerName(printTarget(kind).printer);
		await send(kind, target, printer);
		toast.show({ message: ru.print.sent(printer), tone: 'success' });
	} catch (e) {
		if (noPrinter(e)) {
			if (opts.noPreview) {
				toast.show({
					message: ru.print.noPrinterChosen,
					tone: 'danger',
					action: { label: ru.print.printSettings, run: () => goto(resolve('settings/print')) }
				});
			} else {
				openPreview(kind, target);
			}
			return;
		}
		toast.show({
			message: failureText(e),
			tone: 'danger',
			action: { label: ru.print.preview, run: () => openPreview(kind, target) }
		});
	} finally {
		sending.delete(job);
	}
}

/** Several documents of one order (the act and the sticker of an intake), one after another. */
export async function printDocuments(
	kinds: readonly DocumentKind[],
	target: DocumentTarget,
	opts: PrintOptions = {}
): Promise<void> {
	for (const kind of kinds) await printDocument(kind, target, opts);
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
