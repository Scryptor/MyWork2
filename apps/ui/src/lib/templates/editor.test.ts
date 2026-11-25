import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import { ApiError } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';
import {
	blockLabel,
	createTemplateEditor,
	templateErrors,
	type TemplateEditorApi
} from './editor.svelte.js';
import { insertBlock, insertTextBlock, replaceBlock } from './ops.js';

const text = (s: string): Block => ({
	copies: 'both',
	body: {
		type: 'text',
		text: s,
		size_pt: null,
		bold: false,
		align: 'left',
		one_line: false,
		frame: false,
		numbered: false
	}
});

const STANDARD: Template = {
	kind: 'warranty_intake_act',
	page: { size: 'a4', landscape: false, margins_mm: { top: 10, right: 10, bottom: 10, left: 10 } },
	layout: 'two_per_a4',
	font_size_pt: 9,
	blocks: [text('Акт приёма по гарантии'), { copies: 'both', body: { type: 'separator' } }]
};

const conflict = () =>
	new ApiError(409, 'conflict', 'Запись изменена', {
		code: 'conflict',
		message: 'Запись изменена',
		fields: [],
		current_version: 0,
		permission: null,
		retry_after_secs: null,
		diagnostics: []
	});

/**
 * A server of one template, as mw-server keeps it: a write checks the version, a model equal to the
 * standard one makes the template standard, a write that changes nothing makes no version.
 */
function fakeServer(template: Template = STANDARD) {
	let current: DocumentTemplate = {
		id: 3,
		kind: 'warranty_intake_act',
		template,
		standard: STANDARD,
		custom: JSON.stringify(template) !== JSON.stringify(STANDARD),
		version: 0,
		updated_at: null
	};
	let gate: Promise<void> = Promise.resolve();
	let open = () => {};
	const api = {
		getTemplate: vi.fn(async () => current),
		saveTemplate: vi.fn(async (_kind: string, t: Template, version: number) => {
			await gate;
			await Promise.resolve();
			if (JSON.stringify(t) === JSON.stringify(current.template)) return current;
			if (version !== current.version) throw conflict();
			current = {
				...current,
				template: structuredClone(t),
				custom: JSON.stringify(t) !== JSON.stringify(STANDARD),
				version: current.version + 1
			};
			return current;
		})
	} satisfies TemplateEditorApi;
	return {
		api,
		get current() {
			return current;
		},
		/** Someone at another computer saves the template. */
		elsewhere(t: Template) {
			current = { ...current, template: t, custom: true, version: current.version + 1 };
		},
		hold() {
			gate = new Promise((r) => (open = r));
		},
		release() {
			open();
			gate = Promise.resolve();
		}
	};
}

const settle = async () => {
	for (let i = 0; i < 20; i++) await Promise.resolve();
};

const withText = (t: Template, s: string) => insertBlock(t, [t.blocks.length], text(s));

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('createTemplateEditor', () => {
	it('the text of an old act goes in as one step: one undo takes it out', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		const { template } = insertTextBlock(ed.template, ed.selected, 'Старый акт');
		ed.edit(template);
		expect(ed.template.blocks.at(-1)).toEqual(text('Старый акт'));
		ed.undo();
		expect(ed.template).toEqual(STANDARD);
		expect(ed.canUndo).toBe(false);
		ed.dispose();
	});

	it('an edit can be undone; the undo brings the model back and saves it', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		expect([ed.canUndo, ed.canRedo, ed.isStandard]).toEqual([false, false, true]);

		const edited = withText(ed.template, 'Своя строка');
		ed.edit(edited);
		expect(ed.canUndo).toBe(true);
		expect(ed.template).toEqual(edited);
		expect(ed.isStandard).toBe(false);
		await settle();
		expect(server.api.saveTemplate).toHaveBeenLastCalledWith('warranty_intake_act', edited, 0);
		expect(server.current.custom).toBe(true);

		ed.undo();
		expect(ed.template).toEqual(STANDARD);
		expect([ed.canUndo, ed.canRedo]).toEqual([false, true]);
		await settle();
		expect(server.api.saveTemplate).toHaveBeenLastCalledWith('warranty_intake_act', STANDARD, 1);
		expect(server.current.custom).toBe(false);

		ed.redo();
		expect(ed.template).toEqual(edited);
		await settle();
		expect(server.current.template).toEqual(edited);
		ed.dispose();
	});

	it('«Сбросить к стандартному» is an edit: the template is standard, and it can be undone', async () => {
		const own = withText(STANDARD, 'Своя строка');
		const server = fakeServer(own);
		const ed = createTemplateEditor(server.current, { api: server.api });
		expect(ed.isStandard).toBe(false);

		ed.resetToStandard();
		expect(ed.isStandard).toBe(true);
		expect(ed.template).toEqual(STANDARD);
		await settle();
		expect(server.current.custom).toBe(false);

		ed.undo();
		expect(ed.isStandard).toBe(false);
		expect(ed.template).toEqual(own);
		await settle();
		expect(server.current.template).toEqual(own);
		ed.dispose();
	});

	it('an edit undone before it is written sends nothing', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api, delayMs: 600 });
		// Typing waits for the pause; undone within it, nothing is left to send.
		ed.edit(replaceBlock(ed.template, [0], text('Акт приёма по гарантии №')), 'blocks.0.text');
		ed.undo();
		expect(ed.template).toEqual(STANDARD);
		expect(ed.status.saveState).toBe('idle');
		vi.advanceTimersByTime(1000);
		await settle();
		expect(await ed.flush()).toBe(true);
		expect(server.api.saveTemplate).not.toHaveBeenCalled();
		ed.dispose();
	});

	it('an edit equal to the model makes no step and no write', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		// An operation on a path that is not there gives an equal copy.
		ed.edit(replaceBlock(ed.template, [9], text('нет такого')));
		expect(ed.canUndo).toBe(false);
		await settle();
		expect(server.api.saveTemplate).not.toHaveBeenCalled();
		ed.dispose();
	});

	it('text typed in one field within a second is one step to undo', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		ed.edit(replaceBlock(ed.template, [0], text('А')), 'blocks.0.text');
		vi.advanceTimersByTime(300);
		ed.edit(replaceBlock(ed.template, [0], text('Ак')), 'blocks.0.text');
		ed.undo();
		expect(ed.template).toEqual(STANDARD);
		expect(ed.canUndo).toBe(false);
		ed.dispose();
	});

	it('an undo while the edit is on its way is saved once the answer comes', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		server.hold();
		ed.edit(withText(ed.template, 'Своя строка'));
		await settle();
		expect(server.api.saveTemplate).toHaveBeenCalledTimes(1);
		// Back to what the server had before the write: the write still lands, so the undo must follow.
		ed.undo();
		server.release();
		await settle();
		await ed.flush();
		expect(server.current.template).toEqual(STANDARD);
		expect(ed.template).toEqual(STANDARD);
		ed.dispose();
	});

	it('a failed write undone back to the saved template leaves no error in the save state', async () => {
		const server = fakeServer();
		const refused = new ApiError(422, 'validation', 'Проверьте введённые данные', {
			code: 'validation',
			message: 'Проверьте введённые данные',
			fields: [{ field: 'blocks.2.text', code: 'too_long', message: 'Слишком длинный текст' }],
			current_version: null,
			permission: null,
			retry_after_secs: null,
			diagnostics: []
		});
		server.api.saveTemplate.mockRejectedValueOnce(refused);
		const ed = createTemplateEditor(server.current, { api: server.api });
		ed.edit(withText(ed.template, 'Своя строка'));
		await settle();
		expect(ed.status.saveState).toBe('error');
		expect(ed.status.lost).toBe('в шаблоне ошибка, она показана над превью');

		ed.undo();
		expect(ed.unsaved).toBe(false);
		expect(ed.status.saveState).not.toBe('error');
		expect(ed.status.lost).toBeNull();
		expect(ed.status.autosave.error).toBeNull();
		ed.dispose();
	});

	it('a value the server keeps in its own form is not sent again and again', async () => {
		const server = fakeServer();
		// The server keeps sizes as f32: 9.123456789 comes back a little different.
		const save = server.api.saveTemplate.getMockImplementation()!;
		server.api.saveTemplate.mockImplementation(async (kind, t, version) =>
			save(kind, { ...t, font_size_pt: Math.fround(t.font_size_pt) }, version)
		);
		const ed = createTemplateEditor(server.current, { api: server.api });
		ed.edit({ ...ed.template, font_size_pt: 9.123456789 });
		await settle();
		await settle();
		expect(server.api.saveTemplate).toHaveBeenCalledTimes(1);
		expect(server.current.version).toBe(1);
		ed.dispose();
	});

	it('a template changed elsewhere: «Взять новое» shows theirs, and undo brings own back', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		const theirs = withText(STANDARD, 'Чужая строка');
		server.elsewhere(theirs);
		const mine = withText(ed.template, 'Своя строка');
		ed.edit(mine);
		await settle();
		expect(ed.status.saveState).toBe('conflict');
		expect(ed.conflict?.theirs).toEqual(theirs);
		expect(ed.template).toEqual(mine);

		ed.takeTheirs();
		expect(ed.conflict).toBeNull();
		expect(ed.template).toEqual(theirs);
		await settle();
		expect(server.current.template).toEqual(theirs);

		ed.undo();
		expect(ed.template).toEqual(mine);
		await settle();
		await ed.flush();
		expect(server.current.template).toEqual(mine);
		ed.dispose();
	});

	it('«Оставить моё» writes own model over the newer version', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		server.elsewhere(withText(STANDARD, 'Чужая строка'));
		const mine = withText(ed.template, 'Своя строка');
		ed.edit(mine);
		await settle();
		expect(ed.status.saveState).toBe('conflict');
		ed.keepMine();
		await settle();
		await ed.flush();
		expect(server.current.template).toEqual(mine);
		expect(ed.template).toEqual(mine);
		ed.dispose();
	});

	it('an undo while the record is read again after a 409 does not write over the newer version', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		const theirs = withText(STANDARD, 'Чужая строка');
		server.elsewhere(theirs);
		// The re-reading after the conflict waits until the test lets it go.
		let answer: (r: DocumentTemplate) => void = () => {};
		server.api.getTemplate.mockImplementationOnce(
			() => new Promise((resolve) => (answer = resolve))
		);

		ed.edit(withText(ed.template, 'Своя строка'));
		await settle();
		expect(server.api.saveTemplate).toHaveBeenCalledTimes(1);
		// The own edit is taken back (the model is the saved one again) while the record is being read.
		ed.undo();
		expect(ed.template).toEqual(STANDARD);
		answer(server.current);
		await settle();
		await settle();

		// The record read is taken as it is: no write over the other computer's version.
		expect(server.api.saveTemplate).toHaveBeenCalledTimes(1);
		expect(server.current.template).toEqual(theirs);
		expect(ed.template).toEqual(theirs);
		ed.dispose();
	});

	it('an undo while a write is on its way: the write lands, then the model shown goes again', async () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		server.hold();
		ed.edit(withText(ed.template, 'Своя строка'));
		await settle();
		ed.undo();
		server.release();
		await settle();
		await settle();
		expect(server.current.template).toEqual(STANDARD);
		expect(ed.template).toEqual(STANDARD);
		ed.dispose();
	});

	it('the selection falls back to «Страница» when its block is gone', () => {
		const server = fakeServer();
		const ed = createTemplateEditor(server.current, { api: server.api });
		expect(ed.selected).toBe('page');
		ed.edit(withText(ed.template, 'Своя строка'));
		ed.select([2]);
		expect(ed.selected).toEqual([2]);
		ed.undo();
		expect(ed.selected).toBe('page');
		ed.dispose();
	});
});

describe('blockLabel', () => {
	it('tells QR codes apart by the firm link or the start of their own', () => {
		const qr = (link: string): Block => ({
			copies: 'both',
			body: { type: 'qr', link, size_mm: 20, caption: '', align: 'center' }
		});
		expect(blockLabel(qr('{ссылка.telegram}'))).toBe('QR-код · Telegram');
		expect(blockLabel(qr(' {Фирма.Яндекс_Карты} '))).toBe('QR-код · Яндекс Карты');
		expect(blockLabel(qr('{фирма.гугл_карты}'))).toBe('QR-код · Google Карты');
		// The name from the list, when the caller has it.
		expect(
			blockLabel(qr('{ссылка.link_site}'), [
				{ id: 'link_site', name: 'Наш сайт', url: '', enabled: true }
			])
		).toBe('QR-код · Наш сайт');
		expect(blockLabel(qr('{ссылка.link_gone}'))).toBe('QR-код · link_gone');
		expect(blockLabel(qr('https://example.com/o/{заказ.номер}'))).toBe(
			'QR-код · https://example.com/o/{заказ.номер}'
		);
		expect(blockLabel(qr(''))).toBe('QR-код');
	});

	it('names the type and the start of the text', () => {
		expect(blockLabel(text('Акт приёма № {заказ.номер}'))).toBe(
			'Текст · Акт приёма № {заказ.номер}'
		);
		expect(
			blockLabel(text('Очень длинный текст договора, который не поместится в строку списка'))
		).toBe('Текст · Очень длинный текст договора, который…');
		expect(blockLabel(text('  \n  '))).toBe('Текст');
		expect(blockLabel({ copies: 'both', body: { type: 'separator' } })).toBe('Разделитель');
		expect(blockLabel({ copies: 'both', body: { type: 'spacer', mm: 4 } })).toBe('Отступ · 4 мм');
		expect(blockLabel({ copies: 'both', body: { type: 'spacer', mm: 4.5 } })).toBe(
			'Отступ · 4,5 мм'
		);
		expect(
			blockLabel({
				copies: 'both',
				body: { type: 'typst_code', code: '// пояснение\n\n#v("order.number")' }
			})
		).toBe('Typst-код · #v("order.number")');
		expect(blockLabel({ copies: 'both', body: { type: 'typst_code', code: '' } })).toBe(
			'Typst-код'
		);
		expect(
			blockLabel({ copies: 'both', body: { type: 'typst_code', code: '// только\n  \n' } })
		).toBe('Typst-код');
		// One way to write millimetres: `formatMm`, the texts add the unit themselves.
		expect('mm' in ru.templates).toBe(false);
	});
});

describe('templateErrors', () => {
	it('lists the mistakes of a refused model with what each is about', () => {
		const e = new ApiError(422, 'validation', 'Проверьте введённые данные', {
			code: 'validation',
			message: 'Проверьте введённые данные',
			fields: [
				{ field: 'blocks.0.text', code: 'too_long', message: 'Слишком длинный текст' },
				{ field: 'page.margins_mm', code: 'too_large', message: 'Поля велики' },
				{ field: 'blocks', code: 'too_many', message: 'Слишком много элементов' }
			],
			current_version: null,
			permission: null,
			retry_after_secs: null,
			diagnostics: []
		});
		expect(templateErrors(e, STANDARD)).toEqual([
			'Текст · Акт приёма по гарантии: Слишком длинный текст',
			'Страница: Поля велики',
			'Слишком много элементов'
		]);
		expect(templateErrors(new ApiError(0, 'network', 'Нет связи', null), STANDARD)).toEqual([]);
	});

	it('a code error is a line per diagnostic: the block, the line and the column, then the hints', () => {
		const template: Template = {
			...STANDARD,
			blocks: [
				text('Акт'),
				{ copies: 'both', body: { type: 'typst_code', code: '#v("order.number")\n#foo' } }
			]
		};
		const e = new ApiError(422, 'validation', 'Ошибка в своём Typst-коде', {
			code: 'validation',
			message: 'Ошибка в своём Typst-коде',
			fields: [
				{ field: 'blocks.1.code', code: 'typst_error', message: 'Ошибка в своём Typst-коде' },
				{ field: 'template', code: 'typst_error', message: 'Ошибка в своём Typst-коде' },
				{ field: 'blocks.0.text', code: 'too_long', message: 'Слишком длинный текст' }
			],
			current_version: null,
			permission: null,
			retry_after_secs: null,
			diagnostics: [
				{
					place: { block: [1], line: 2, column: 3 },
					message: 'unknown variable: foo',
					hints: ['check the spelling', 'or define it']
				},
				{ place: null, message: 'maximum function call depth exceeded', hints: [] },
				// A block that is gone meanwhile: its type is what is left to name it.
				{ place: { block: [7], line: 1, column: 1 }, message: 'expected expression', hints: [] }
			]
		});
		expect(templateErrors(e, template)).toEqual([
			'Typst-код · #v("order.number") · строка 2, знак 3: unknown variable: foo — check the spelling — or define it',
			'maximum function call depth exceeded',
			'Typst-код · строка 1, знак 1: expected expression',
			// The fields of the code errors repeat the lines above; the others stay.
			'Текст · Акт: Слишком длинный текст'
		]);
	});
});
