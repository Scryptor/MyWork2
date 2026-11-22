import type { DocumentFormat } from '$api/DocumentFormat';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import type { DocumentQuery } from '$api/DocumentQuery';
import { request, requestFile } from './client.js';

/** How a document is built: the label and its offset, the printer's resolution, the raster's (PNG only). */
export interface DocumentOptions {
	/** Dots per inch of the PNG raster; the server's default is 110 (a preview on the screen). */
	dpi?: number;
	/** Dots per inch of the printer: the barcode is fitted to its dots. */
	printerDpi?: number;
	/** The label, width × height, mm. */
	labelMm?: [number, number];
	/** The content moved right and down, mm. */
	offsetMm?: [number, number];
}

/** A PDF has no raster: its options have no `dpi` (the server refuses one). */
export type PdfOptions = Omit<DocumentOptions, 'dpi'> & { dpi?: never };

/** An order's document, or without `orderId` the document itself (blank act, calibration) or a sample. */
export interface DocumentTarget {
	orderId?: number;
}

const path = (kind: DocumentKind, target: DocumentTarget) =>
	target.orderId === undefined
		? `/documents/${kind}`
		: `/orders/${target.orderId}/documents/${kind}`;

/** The parameters of the request; what is not given is not sent, a PDF never gets `dpi`. */
export function documentQuery(format: DocumentFormat, opts: DocumentOptions): DocumentQuery {
	const q: DocumentQuery = { format };
	if (format === 'png' && opts.dpi !== undefined) q.dpi = opts.dpi;
	if (opts.printerDpi !== undefined) q.printer_dpi = opts.printerDpi;
	if (opts.labelMm) [q.label_w_mm, q.label_h_mm] = opts.labelMm;
	if (opts.offsetMm) [q.offset_x_mm, q.offset_y_mm] = opts.offsetMm;
	return q;
}

/** A PDF of the document and the name the server gave it (`akt-priema-<номер>.pdf`). */
export interface DocumentPdf {
	blob: Blob;
	filename: string;
}

/** The name of a PDF that came without one: plain, not made up from the order. */
const UNNAMED_PDF = 'document.pdf';

export async function documentPdf(
	kind: DocumentKind,
	target: DocumentTarget,
	opts: PdfOptions = {}
): Promise<DocumentPdf> {
	const file = await requestFile(path(kind, target), { query: documentQuery('pdf', opts) });
	return { blob: file.blob, filename: file.filename ?? UNNAMED_PDF };
}

/** The pages as PNG; `signal` calls the request off (a preview closed before the answer). */
export const documentPages = (
	kind: DocumentKind,
	target: DocumentTarget,
	opts: DocumentOptions = {},
	signal?: AbortSignal
): Promise<DocumentPages> =>
	request<DocumentPages>('GET', path(kind, target), {
		query: documentQuery('png', opts),
		signal
	});
