import { describe, expect, it, vi } from 'vitest';
import type { Template } from '$api/Template';
import { ApiError } from '#lib/api/client.js';
import { readTemplateFile } from './file.js';

// A `.mwtemplate` file the user picked: what is not JSON or has the wrong format is «не файл шаблона»,
// a newer version is its own mistake, a model the document does not take lists the server's texts.

const template = { kind: 'warranty_intake_act' } as Template;
const invalid = (...fields: [string, string][]) =>
	new ApiError(422, 'validation', 'Ошибка', {
		code: 'validation',
		message: 'Ошибка',
		fields: fields.map(([field, message]) => ({ field, code: 'invalid', message })),
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

describe('readTemplateFile', () => {
	it('sends the parsed file and gives the template back', async () => {
		const send = vi.fn(async () => template);
		const r = await readTemplateFile('warranty_intake_act', '{"format":"mwtemplate"}', send);
		expect(r).toEqual({ ok: true, template });
		expect(send).toHaveBeenCalledWith('warranty_intake_act', { format: 'mwtemplate' });
	});

	it('text that is not JSON is not a template file, and nothing is sent', async () => {
		const send = vi.fn(async () => template);
		expect(await readTemplateFile('warranty_intake_act', 'not json', send)).toEqual({
			ok: false,
			problem: 'not_template'
		});
		expect(send).not.toHaveBeenCalled();
	});

	it('the field `format` is not a template file', async () => {
		const r = await readTemplateFile('warranty_intake_act', '{}', async () => {
			throw invalid(['format', 'x']);
		});
		expect(r).toEqual({ ok: false, problem: 'not_template' });
	});

	it('the field `format_version` is a newer program', async () => {
		const r = await readTemplateFile('warranty_intake_act', '{}', async () => {
			throw invalid(['format_version', 'x']);
		});
		expect(r).toEqual({ ok: false, problem: 'newer' });
	});

	it('other fields are listed with the server texts', async () => {
		const r = await readTemplateFile('warranty_intake_act', '{}', async () => {
			throw invalid(['blocks[0].mm', 'Отступ слишком большой'], ['file', 'Файл повреждён']);
		});
		expect(r).toEqual({
			ok: false,
			problem: 'misfit',
			messages: ['Отступ слишком большой', 'Файл повреждён']
		});
	});

	it('another failure is not a problem of the file: it is thrown', async () => {
		const down = new ApiError(0, 'network', 'нет связи', null);
		await expect(
			readTemplateFile('warranty_intake_act', '{}', async () => {
				throw down;
			})
		).rejects.toBe(down);
	});
});
