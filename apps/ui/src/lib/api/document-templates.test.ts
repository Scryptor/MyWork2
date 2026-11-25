import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Template } from '$api/Template';
import {
	documentFields,
	exportTemplateFile,
	getTemplate,
	importTemplateFile,
	listTemplates,
	previewTemplate,
	saveTemplate
} from './document-templates.js';

function stubFetch(response: () => Response) {
	const fn = vi.fn<(url: string, init?: RequestInit) => Promise<Response>>(async () => response());
	vi.stubGlobal('fetch', fn);
	return fn;
}
const json = (v: unknown) => new Response(JSON.stringify(v), { status: 200 });

/** Path, method and parsed body of the only request. */
function sent(fn: ReturnType<typeof stubFetch>) {
	expect(fn).toHaveBeenCalledOnce();
	const [url, init] = fn.mock.calls[0]!;
	return {
		path: new URL(url, 'http://127.0.0.1').pathname,
		method: init?.method,
		body: typeof init?.body === 'string' ? JSON.parse(init.body) : undefined
	};
}

const template = { kind: 'intake_act', blocks: [] } as unknown as Template;

afterEach(() => vi.unstubAllGlobals());

describe('document templates API', () => {
	it('lists and gets', async () => {
		let fn = stubFetch(() => json([]));
		await listTemplates();
		expect(sent(fn)).toMatchObject({ path: '/api/v1/document-templates', method: 'GET' });
		fn = stubFetch(() => json({}));
		await getTemplate('intake_act');
		expect(sent(fn)).toMatchObject({
			path: '/api/v1/document-templates/intake_act',
			method: 'GET'
		});
	});

	it('saves with the version', async () => {
		const fn = stubFetch(() => json({}));
		await saveTemplate('intake_act', template, 4);
		expect(sent(fn)).toEqual({
			path: '/api/v1/document-templates/intake_act',
			method: 'PUT',
			body: { template, version: 4 }
		});
	});

	it('previews with the body and the signal', async () => {
		const fn = stubFetch(() => json({ pages: [] }));
		const ctl = new AbortController();
		await previewTemplate('sticker', { template, order_id: 3 }, ctl.signal);
		expect(sent(fn)).toEqual({
			path: '/api/v1/document-templates/sticker/preview',
			method: 'POST',
			body: { template, order_id: 3 }
		});
		expect(fn.mock.calls[0]![1]?.signal).toBe(ctl.signal);
	});

	it('exports a file under the name of the server', async () => {
		const fn = stubFetch(
			() =>
				new Response('{}', {
					status: 200,
					headers: { 'Content-Disposition': 'attachment; filename="akt-priema.mwtemplate"' }
				})
		);
		const f = await exportTemplateFile('intake_act');
		expect(f.filename).toBe('akt-priema.mwtemplate');
		expect(sent(fn)).toMatchObject({
			path: '/api/v1/document-templates/intake_act/file',
			method: 'GET'
		});
		stubFetch(() => new Response('{}', { status: 200 }));
		expect((await exportTemplateFile('intake_act')).filename).toBe('intake_act.mwtemplate');
	});

	it('imports the parsed file as the body', async () => {
		const fn = stubFetch(() => json(template));
		const file = { format: 1, template };
		expect(await importTemplateFile('intake_act', file)).toEqual(template);
		expect(sent(fn)).toEqual({
			path: '/api/v1/document-templates/intake_act/file',
			method: 'POST',
			body: file
		});
	});

	it('lists the fields', async () => {
		const fn = stubFetch(() => json([{ placeholder: '{заказ.номер}', label: 'Номер' }]));
		expect(await documentFields()).toHaveLength(1);
		expect(sent(fn)).toMatchObject({ path: '/api/v1/document-fields', method: 'GET' });
	});

	it('asks for the fields of a kind', async () => {
		const fn = stubFetch(() => json([]));
		await documentFields('cell_sticker');
		expect(sent(fn)).toMatchObject({ path: '/api/v1/document-fields' });
		expect(fn.mock.calls[0]![0]).toContain('kind=cell_sticker');
	});
});
