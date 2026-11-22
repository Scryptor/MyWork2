import { mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { leaveDesktop } from '#lib/platform/fake-desktop.js';
import {
	canPrintDirectly,
	listPrinters,
	printFormat,
	printPages,
	printPdf,
	type PrinterInfo
} from './platform.js';

// Printing on this computer goes through the commands of the desktop program; their arguments are
// camelCase on this side. «Принтер по умолчанию» (null) becomes the system's default printer.

afterEach(leaveDesktop);

/** The desktop program with these printers: records every command and its arguments. */
function desktop(printers: PrinterInfo[], fail?: { command: string; error: string }) {
	const calls: [string, unknown][] = [];
	mockIPC((cmd, args) => {
		calls.push([cmd, args]);
		if (fail && cmd === fail.command) throw fail.error;
		if (cmd === 'list_printers') return printers;
		if (cmd === 'print_format') return 'pdf';
		return null;
	});
	return calls;
}

const PRINTERS: PrinterInfo[] = [
	{ name: 'Акты HP', is_default: false },
	{ name: 'Наклейки Xprinter', is_default: true }
];

describe('in a browser', () => {
	it('there is no direct printing, and a command says why instead of failing on the bridge', async () => {
		expect(canPrintDirectly()).toBe(false);
		await expect(listPrinters()).rejects.toThrow(
			'Печать на принтер — только в программе для компьютера'
		);
		await expect(
			printPdf({ printer: 'Акты HP', pdfBase64: 'JVBERg==', copies: 1, mediaMm: null })
		).rejects.toThrow('Печать на принтер — только в программе для компьютера');
	});
});

describe('in the desktop program', () => {
	it('lists the printers and the format the system prints', async () => {
		const calls = desktop(PRINTERS);
		expect(canPrintDirectly()).toBe(true);
		expect(await listPrinters()).toEqual(PRINTERS);
		expect(await printFormat()).toBe('pdf');
		expect(calls.map(([cmd]) => cmd)).toEqual(['list_printers', 'print_format']);
	});

	it('prints a PDF on the chosen printer with the camelCase arguments of the command', async () => {
		const calls = desktop(PRINTERS);
		await printPdf({ printer: 'Акты HP', pdfBase64: 'JVBERg==', copies: 2, mediaMm: [58, 40] });
		expect(calls).toEqual([
			['print_pdf', { printer: 'Акты HP', pdfBase64: 'JVBERg==', copies: 2, mediaMm: [58, 40] }]
		]);
	});

	it('prints pages with their resolution and sheet size', async () => {
		const calls = desktop(PRINTERS);
		await printPages({
			printer: 'Наклейки Xprinter',
			pagesBase64: ['iVBORw0KGgo='],
			dpi: 203,
			widthMm: 58,
			heightMm: 40,
			copies: 1
		});
		expect(calls).toEqual([
			[
				'print_pages',
				{
					printer: 'Наклейки Xprinter',
					pagesBase64: ['iVBORw0KGgo='],
					dpi: 203,
					widthMm: 58,
					heightMm: 40,
					copies: 1
				}
			]
		]);
	});

	it('«Принтер по умолчанию» is the system’s default printer, by its name', async () => {
		const calls = desktop(PRINTERS);
		await printPdf({ printer: null, pdfBase64: 'JVBERg==', copies: 1, mediaMm: null });
		expect(calls).toEqual([
			['list_printers', {}],
			[
				'print_pdf',
				{ printer: 'Наклейки Xprinter', pdfBase64: 'JVBERg==', copies: 1, mediaMm: null }
			]
		]);
	});

	it('without a default printer in the system nothing is printed, and it says so', async () => {
		const calls = desktop([{ name: 'Акты HP', is_default: false }]);
		await expect(
			printPages({
				printer: null,
				pagesBase64: ['iVBORw0KGgo='],
				dpi: 203,
				widthMm: 58,
				heightMm: 40,
				copies: 1
			})
		).rejects.toThrow('Принтер по умолчанию не задан');
		expect(calls.map(([cmd]) => cmd)).toEqual(['list_printers']);
	});

	it('the error of the program (a Russian string) becomes an Error with that text', async () => {
		desktop(PRINTERS, { command: 'print_pdf', error: 'Принтер «Акты HP» не найден' });
		const failure = printPdf({
			printer: 'Акты HP',
			pdfBase64: 'JVBERg==',
			copies: 1,
			mediaMm: null
		});
		await expect(failure).rejects.toBeInstanceOf(Error);
		await expect(failure).rejects.toThrow('Принтер «Акты HP» не найден');
	});
});
