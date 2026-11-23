import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import BlockList from './BlockListHarness.test-helper.svelte';
import { createTemplateEditor, type TemplateEditorApi } from './editor.svelte.js';

// The list of blocks moved with the arrows of a handle: the row moves at once, the template after a
// pause. The rows keep their keys while they move, so the focus stays with the block it was on and the
// next arrow moves that block again.

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});
afterEach(() => vi.useRealTimers());

const text = (s: string): Block => ({
	copies: 'both',
	body: { type: 'text', text: s, size_pt: null, bold: false, align: 'left', one_line: false }
});

function editorOf(blocks: Block[]) {
	const template: Template = {
		kind: 'warranty_intake_act',
		page: {
			size: 'a4',
			landscape: false,
			margins_mm: { top: 10, right: 10, bottom: 10, left: 10 }
		},
		layout: 'two_per_a4',
		font_size_pt: 9,
		blocks
	};
	const initial: DocumentTemplate = {
		id: 3,
		kind: 'warranty_intake_act',
		template,
		standard: template,
		custom: false,
		version: 0,
		updated_at: null
	};
	const api: TemplateEditorApi = {
		getTemplate: async () => initial,
		saveTemplate: async (_kind, t, version) => ({ ...initial, template: t, version: version + 1 })
	};
	return createTemplateEditor(initial, { api });
}

/** The texts of the blocks of a list (the top level or a column) of the template. */
const textsOf = (blocks: Block[]) =>
	blocks.map((b) => (b.body.type === 'text' ? b.body.text : b.body.type));
const handle = (name: string) =>
	screen.getByRole('button', { name: `Переместить „Текст · ${name}“` });
/** The order of the text blocks on screen, by their handles. */
const shown = () =>
	screen
		.getAllByRole('button', { name: /^Переместить „Текст · / })
		.map((b) => /· (.*)“$/.exec(b.getAttribute('aria-label') ?? '')![1]);

function setup(blocks: Block[]) {
	vi.useFakeTimers();
	const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
	const editor = editorOf(blocks);
	render(BlockList, { editor });
	return { user, editor };
}

describe('BlockList: moving a block with the arrows', () => {
	it('two arrows with a pause between them move the same block, and the focus stays on it', async () => {
		const { user, editor } = setup([text('Первый'), text('Второй'), text('Третий')]);
		handle('Третий').focus();
		await user.keyboard('{ArrowUp}');
		await vi.advanceTimersByTimeAsync(400);
		expect(textsOf(editor.template.blocks)).toEqual(['Первый', 'Третий', 'Второй']);
		expect(handle('Третий')).toHaveFocus();

		await user.keyboard('{ArrowUp}');
		await vi.advanceTimersByTimeAsync(400);
		expect(textsOf(editor.template.blocks)).toEqual(['Третий', 'Первый', 'Второй']);
		expect(handle('Третий')).toHaveFocus();
		expect(shown()).toEqual(['Третий', 'Первый', 'Второй']);
		editor.dispose();
	});

	it('in a column too: two arrows with a pause move the same block of the column', async () => {
		const columns: Block = {
			copies: 'both',
			body: { type: 'columns', gap_mm: 4, columns: [[text('а'), text('б'), text('в')], []] }
		};
		const { user, editor } = setup([text('Шапка'), columns]);
		handle('в').focus();
		await user.keyboard('{ArrowUp}');
		await vi.advanceTimersByTimeAsync(400);
		await user.keyboard('{ArrowUp}');
		await vi.advanceTimersByTimeAsync(400);
		const body = editor.template.blocks[1]!.body;
		expect(body.type === 'columns' && textsOf(body.columns[0]!)).toEqual(['в', 'а', 'б']);
		expect(handle('в')).toHaveFocus();
		expect(shown()).toEqual(['Шапка', 'в', 'а', 'б']);
		editor.dispose();
	});

	it('Ctrl+Z while an arrow move waits: the block the arrows moved is the one that moves', async () => {
		const { user, editor } = setup([text('Первый'), text('Второй'), text('Третий')]);
		handle('Третий').focus();
		await user.keyboard('{ArrowUp}');
		await vi.advanceTimersByTimeAsync(400);
		expect(textsOf(editor.template.blocks)).toEqual(['Первый', 'Третий', 'Второй']);

		await user.keyboard('{ArrowUp}');
		// The first move is undone while the second one waits for the pause.
		editor.undo();
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Третий', 'Первый', 'Второй']);
		await vi.advanceTimersByTimeAsync(400);
		expect(textsOf(editor.template.blocks)).toEqual(['Третий', 'Первый', 'Второй']);
		editor.dispose();
	});
});

describe('BlockList: the rows follow a template changed from outside', () => {
	it('an edit, an undo and a redo of the template (another length, then the same) show in the rows', async () => {
		const { editor } = setup([text('Первый'), text('Второй')]);
		const t = editor.template;
		// A block added from outside (the file menu, «Взять новое»): the rows follow.
		editor.edit({ ...t, blocks: [...t.blocks, text('Третий')] });
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Первый', 'Второй', 'Третий']);
		// The same length, another order: the rows show the new order, their handles are those blocks.
		const now = editor.template;
		editor.edit({ ...now, blocks: [now.blocks[2]!, now.blocks[0]!, now.blocks[1]!] });
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Третий', 'Первый', 'Второй']);
		editor.undo();
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Первый', 'Второй', 'Третий']);
		editor.undo();
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Первый', 'Второй']);
		editor.redo();
		await vi.advanceTimersByTimeAsync(0);
		expect(shown()).toEqual(['Первый', 'Второй', 'Третий']);
		editor.dispose();
	});
});
