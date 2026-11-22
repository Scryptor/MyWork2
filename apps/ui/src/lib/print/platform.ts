// Printing on this computer: the commands of the desktop program (`apps/desktop/src-tauri/src/print`).
// The module of the bridge is loaded only in the program; a browser never downloads it and prints
// through a PDF instead.

import type { InvokeArgs } from '@tauri-apps/api/core';
import { ru } from '#lib/i18n/ru.js';
import { isTauri } from '#lib/platform/tauri.js';

/** A printer of this computer, as the program names it. */
export interface PrinterInfo {
	name: string;
	is_default: boolean;
}

/** What printing takes: a PDF (CUPS on macOS and Linux) or pages of PNG (GDI on Windows). */
export type PrintFormat = 'pdf' | 'png';

/** The program prints by itself; a browser only through a PDF. */
export function canPrintDirectly(): boolean {
	return isTauri();
}

/** A command of the program; its error (a Russian string) becomes an `Error` with that text. */
async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
	if (!isTauri()) throw new Error(ru.print.onlyInProgram);
	const { invoke } = await import('@tauri-apps/api/core');
	try {
		return await invoke<T>(command, args);
	} catch (e) {
		const message = typeof e === 'string' ? e : e instanceof Error ? e.message : ru.print.failed;
		throw new Error(message, { cause: e });
	}
}

export const listPrinters = () => call<PrinterInfo[]>('list_printers');

export const printFormat = () => call<PrintFormat>('print_format');

/** The name to print on: the chosen printer, or for null the system's default one. */
async function resolvePrinter(printer: string | null): Promise<string> {
	if (printer !== null) return printer;
	const found = (await listPrinters()).find((p) => p.is_default);
	if (!found) throw new Error(ru.print.noDefaultPrinter);
	return found.name;
}

/** A PDF to CUPS; `mediaMm` is the label (width, height), null for an act. */
export async function printPdf(opts: {
	printer: string | null;
	pdfBase64: string;
	copies: number;
	mediaMm: [number, number] | null;
}): Promise<void> {
	const printer = await resolvePrinter(opts.printer);
	await call<void>('print_pdf', {
		printer,
		pdfBase64: opts.pdfBase64,
		copies: opts.copies,
		mediaMm: opts.mediaMm
	});
}

/** Pages of PNG to GDI: `dpi` dots per inch, a sheet of `widthMm` × `heightMm`. */
export async function printPages(opts: {
	printer: string | null;
	pagesBase64: string[];
	dpi: number;
	widthMm: number;
	heightMm: number;
	copies: number;
}): Promise<void> {
	const printer = await resolvePrinter(opts.printer);
	await call<void>('print_pages', {
		printer,
		pagesBase64: opts.pagesBase64,
		dpi: opts.dpi,
		widthMm: opts.widthMm,
		heightMm: opts.heightMm,
		copies: opts.copies
	});
}
