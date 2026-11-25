import { afterEach, describe, expect, it, vi } from 'vitest';
import { ru } from '#lib/i18n/ru.js';
import { documentBatchPdf, documentPages, documentPdf, documentQuery } from './documents.js';

// The documents of the server: an order's (`/orders/{id}/documents/{kind}`) or without an order
// (`/documents/{kind}`: the blank act, the calibration label, the sample of the rest). The options of
// the screen become the parameters of the request.

function stubFetch(response: () => Response) {
	const fn = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async () => response());
	vi.stubGlobal('fetch', fn);
	return fn;
}

/** The address of the only request, as a path and its parameters. */
function sent(fn: ReturnType<typeof stubFetch>) {
	expect(fn).toHaveBeenCalledOnce();
	const url = new URL(fn.mock.calls[0]![0], 'http://127.0.0.1');
	return { path: url.pathname, params: Object.fromEntries(url.searchParams) };
}

afterEach(() => {
	vi.unstubAllGlobals();
});

describe('documentQuery', () => {
	it('names every option as the API does', () => {
		expect(
			documentQuery('png', {
				dpi: 300,
				printerDpi: 203,
				labelMm: [58, 40],
				offsetMm: [1.5, -2]
			})
		).toEqual({
			format: 'png',
			dpi: 300,
			printer_dpi: 203,
			label_w_mm: 58,
			label_h_mm: 40,
			offset_x_mm: 1.5,
			offset_y_mm: -2
		});
	});

	it('sends nothing that was not given: the server has its defaults', () => {
		expect(documentQuery('png', {})).toEqual({ format: 'png' });
		expect(documentQuery('pdf', { printerDpi: 600 })).toEqual({ format: 'pdf', printer_dpi: 600 });
	});

	it('sends the chosen pages; none chosen (null) is the whole document, no parameter', () => {
		expect(documentQuery('pdf', { pages: '1,3-4' })).toEqual({ format: 'pdf', pages: '1,3-4' });
		expect(documentQuery('png', { dpi: 300, pages: '2' })).toEqual({
			format: 'png',
			dpi: 300,
			pages: '2'
		});
		expect(documentQuery('pdf', { pages: null })).toEqual({ format: 'pdf' });
	});

	it('refuses an empty selection: nothing checked is not the whole document', () => {
		expect(() => documentQuery('pdf', { pages: '' })).toThrow(ru.print.noPagesChecked);
		expect(() => documentQuery('png', { dpi: 300, pages: '' })).toThrow(ru.print.noPagesChecked);
	});

	it('a PDF never gets the raster resolution, even when it is passed', () => {
		const opts = { dpi: 300, printerDpi: 203, labelMm: [40, 30] as [number, number] };
		expect(documentQuery('pdf', opts)).toEqual({
			format: 'pdf',
			printer_dpi: 203,
			label_w_mm: 40,
			label_h_mm: 30
		});
	});
});

describe('documentPdf', () => {
	it('fetches the order’s document as a PDF with the printer’s resolution, without dpi', async () => {
		const fn = stubFetch(
			() =>
				new Response('%PDF-1.7', { status: 200, headers: { 'Content-Type': 'application/pdf' } })
		);
		// A caller that gets around the type still sends no `dpi`: the server would refuse it with 400.
		const { blob } = await documentPdf('intake_act', { orderId: 7 }, {
			printerDpi: 203,
			offsetMm: [0.5, 0],
			dpi: 300
		} as never);
		expect(blob.type).toBe('application/pdf');
		expect(await blob.text()).toBe('%PDF-1.7');
		expect(sent(fn)).toEqual({
			path: '/api/v1/orders/7/documents/intake_act',
			params: { format: 'pdf', printer_dpi: '203', offset_x_mm: '0.5', offset_y_mm: '0' }
		});
	});

	it('is named as the server names it: «Сохранить PDF» keeps that name', async () => {
		stubFetch(
			() =>
				new Response('%PDF', {
					status: 200,
					headers: { 'Content-Disposition': 'inline; filename="akt-priema-42.pdf"' }
				})
		);
		expect((await documentPdf('intake_act', { orderId: 42 })).filename).toBe('akt-priema-42.pdf');
		// No name in the answer: a plain one, not one made up from the order.
		stubFetch(() => new Response('%PDF', { status: 200 }));
		expect((await documentPdf('intake_act', { orderId: 42 })).filename).toBe('document.pdf');
	});

	it('a name that would climb out of Downloads is cut to the file, a bare «..» is the plain name', async () => {
		stubFetch(
			() =>
				new Response('%PDF', {
					status: 200,
					headers: { 'Content-Disposition': 'attachment; filename="../../x.pdf"' }
				})
		);
		expect((await documentPdf('intake_act', { orderId: 42 })).filename).toBe('x.pdf');
		stubFetch(
			() =>
				new Response('%PDF', {
					status: 200,
					headers: { 'Content-Disposition': "attachment; filename*=UTF-8''%2E%2E" }
				})
		);
		expect((await documentPdf('intake_act', { orderId: 42 })).filename).toBe('document.pdf');
	});

	it('only the chosen pages', async () => {
		const fn = stubFetch(() => new Response('%PDF', { status: 200 }));
		await documentPdf('intake_act', { orderId: 7 }, { printerDpi: 300, pages: '1' });
		expect(sent(fn)).toEqual({
			path: '/api/v1/orders/7/documents/intake_act',
			params: { format: 'pdf', printer_dpi: '300', pages: '1' }
		});
	});

	it('nothing checked asks the server nothing', async () => {
		const fn = stubFetch(() => new Response('%PDF', { status: 200 }));
		await expect(documentPdf('intake_act', { orderId: 7 }, { pages: '' })).rejects.toThrow(
			ru.print.noPagesChecked
		);
		await expect(documentPages('intake_act', { orderId: 7 }, { pages: '' })).rejects.toThrow(
			ru.print.noPagesChecked
		);
		expect(fn).not.toHaveBeenCalled();
	});

	it('without an order: the document itself (blank act) or the sample', async () => {
		const fn = stubFetch(() => new Response('%PDF', { status: 200 }));
		await documentPdf('blank_intake_act', {});
		expect(sent(fn)).toEqual({
			path: '/api/v1/documents/blank_intake_act',
			params: { format: 'pdf' }
		});
	});

	it('a refusal of the server is an ApiError with its message', async () => {
		stubFetch(
			() =>
				new Response(
					JSON.stringify({ code: 'not_found', message: 'Заказ не найден', fields: [] }),
					{ status: 404 }
				)
		);
		await expect(documentPdf('issue_act', { orderId: 1 })).rejects.toMatchObject({
			name: 'ApiError',
			status: 404,
			message: 'Заказ не найден'
		});
	});
});

describe('documentPages', () => {
	it('fetches the pages as PNG with the raster and the printer’s resolution, the label size', async () => {
		const pages = { dpi: 203, width_mm: 58, height_mm: 40, pages: ['iVBORw0KGgo='] };
		const fn = stubFetch(() => new Response(JSON.stringify(pages), { status: 200 }));
		expect(
			await documentPages(
				'sticker',
				{ orderId: 12 },
				{ dpi: 203, printerDpi: 203, labelMm: [58, 40] }
			)
		).toEqual(pages);
		expect(sent(fn)).toEqual({
			path: '/api/v1/orders/12/documents/sticker',
			params: {
				format: 'png',
				dpi: '203',
				printer_dpi: '203',
				label_w_mm: '58',
				label_h_mm: '40'
			}
		});
	});

	it('a preview closed before the answer calls its request off', async () => {
		const fn = stubFetch(() => new Response('{}', { status: 200 }));
		const controller = new AbortController();
		await documentPages('intake_act', { orderId: 3 }, {}, controller.signal);
		expect(fn.mock.calls[0]![1]?.signal).toBe(controller.signal);
	});

	it('the sample of a document of an order is asked without an order', async () => {
		const fn = stubFetch(
			() =>
				new Response(JSON.stringify({ dpi: 110, width_mm: 210, height_mm: 297, pages: [] }), {
					status: 200
				})
		);
		await documentPages('issue_act', {});
		expect(sent(fn)).toEqual({ path: '/api/v1/documents/issue_act', params: { format: 'png' } });
	});
});

describe('documentBatchPdf', () => {
	it('posts the orders and the options as JSON and returns the file with its name', async () => {
		const fn = stubFetch(
			() =>
				new Response('%PDF', {
					headers: { 'Content-Disposition': 'inline; filename="naklejka-2.pdf"' }
				})
		);
		const file = await documentBatchPdf('sticker', [4, 5], {
			printerDpi: 203,
			labelMm: [58, 40],
			offsetMm: [1, -1]
		});
		expect(fn).toHaveBeenCalledOnce();
		const [url, init] = fn.mock.calls[0]!;
		expect(new URL(url, 'http://127.0.0.1').pathname).toBe('/api/v1/documents/sticker/batch');
		expect(init?.method).toBe('POST');
		expect(JSON.parse(init?.body as string)).toEqual({
			order_ids: [4, 5],
			printer_dpi: 203,
			label_w_mm: 58,
			label_h_mm: 40,
			offset_x_mm: 1,
			offset_y_mm: -1
		});
		expect(file.filename).toBe('naklejka-2.pdf');
	});
});

describe('cell and rack stickers', () => {
	it('cell and rack stickers have their paths', async () => {
		const fn = stubFetch(() => new Response('%PDF', { status: 200 }));
		await documentPdf('cell_sticker', { rackId: 3, shelf: 2, cell: 4 });
		expect(sent(fn).path).toBe('/api/v1/stock/racks/3/cells/2/4/sticker');
		expect(new URL(fn.mock.calls[0]![0], 'http://127.0.0.1').searchParams.get('format')).toBe(
			'pdf'
		);
		fn.mockClear();
		await documentPdf('cell_sticker', { rackId: 3 });
		expect(sent(fn).path).toBe('/api/v1/stock/racks/3/stickers');
	});
});
