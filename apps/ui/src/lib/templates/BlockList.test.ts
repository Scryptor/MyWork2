import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import BlockList from './BlockListHarness.test-helper.svelte';
import { createTemplateEditor, type TemplateEditorApi } from './editor.svelte.js';

const blockCode = vi.hoisted(() => vi.fn());
vi.mock('#lib/api/document-templates.js', async (original) => ({
	...(await original<typeof import('#lib/api/document-templates.js')>()),
	blockCode
}));

// The list of blocks moved with the arrows of a handle: the row moves at once, the template after a
// pause. The rows keep their keys while they move, so the focus stays with the block it was on and the
// next arrow moves that block again.

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});
afterEach(() => {
	vi.useRealTimers();
	toast.clear();
	blockCode.mockReset();
});

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

describe('BlockList: «Свой Typst-код»', () => {
	const barcode: Block = {
		copies: 'both',
		body: {
			type: 'barcode',
			height_mm: 12,
			digits: true,
			align: 'center',
			fill: false,
			stretch: false
		}
	};

	/**
	 * The menu of a block, opened from the keyboard: the one its button controls (jsdom does not finish
	 * the closing of the one before).
	 */
	async function openMenu(label: string) {
		const button = screen.getByRole('button', { name: `Действия: ${label}` });
		button.focus();
		await userEvent.keyboard('{Enter}');
		return waitFor(() => {
			const menu = document.getElementById(button.getAttribute('aria-controls') ?? '');
			expect(menu).toHaveAttribute('role', 'menu');
			return menu!;
		});
	}
	const itemsOf = (menu: HTMLElement) =>
		[...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].map((i) => i.textContent?.trim());
	/** Chosen from the keyboard, as a click would. */
	async function choose(menu: HTMLElement, label: string) {
		const item = [...menu.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
			(i) => i.textContent?.trim() === label
		)!;
		item.focus();
		await userEvent.keyboard('{Enter}');
	}

	it('a text block offers it, a barcode does not', async () => {
		const editor = editorOf([text('Шапка'), barcode]);
		render(BlockList, { editor });
		expect(itemsOf(await openMenu('Текст · Шапка'))).toContain('Свой Typst-код');
		await userEvent.keyboard('{Escape}');
		const menu = await openMenu('Штрихкод');
		expect(itemsOf(menu)).toContain('Дублировать');
		expect(itemsOf(menu)).not.toContain('Свой Typst-код');
		editor.dispose();
	});

	it('choosing it makes the block a code block of the code the server gave; one undo brings it back', async () => {
		blockCode.mockResolvedValue({ code: '#{ "x" }' });
		const footer: Block = { ...text('Подвал'), copies: 'client' };
		const editor = editorOf([text('Шапка'), footer]);
		editor.select([1]);
		const before = editor.template;
		render(BlockList, { editor });
		await choose(await openMenu('Текст · Подвал'), 'Свой Typst-код');
		await waitFor(() =>
			expect(editor.template.blocks[1]).toEqual({
				copies: 'client',
				body: { type: 'typst_code', code: '#{ "x" }' }
			})
		);
		expect(blockCode).toHaveBeenCalledWith('warranty_intake_act', before, [1]);
		expect(editor.template.blocks[0]).toEqual(text('Шапка'));
		expect(editor.selected).toEqual([1]);
		editor.undo();
		expect(editor.template.blocks[1]).toEqual(footer);
		expect(editor.canUndo).toBe(false);
		editor.dispose();
	});

	it('a block changed while the code was on its way is not replaced by the code of what it was', async () => {
		let answer: (v: { code: string }) => void = () => {};
		blockCode.mockReturnValue(new Promise((r) => (answer = r)));
		const editor = editorOf([text('Шапка'), text('Подвал')]);
		render(BlockList, { editor });
		await choose(await openMenu('Текст · Подвал'), 'Свой Typst-код');
		await waitFor(() => expect(blockCode).toHaveBeenCalled());
		// Another block changed meanwhile: the code still goes in, over the template as it is now.
		const now = editor.template;
		editor.edit({ ...now, blocks: [text('Новая шапка'), now.blocks[1]!] });
		answer({ code: '#{ "x" }' });
		await waitFor(() => expect(editor.template.blocks[1]!.body.type).toBe('typst_code'));
		expect(editor.template.blocks[0]).toEqual(text('Новая шапка'));

		// The block itself changed: its code is no more what it prints.
		blockCode.mockReturnValue(new Promise((r) => (answer = r)));
		await choose(await openMenu('Текст · Новая шапка'), 'Свой Typst-код');
		await waitFor(() => expect(blockCode).toHaveBeenCalledTimes(2));
		const later = editor.template;
		editor.edit({ ...later, blocks: [text('Шапка 2'), later.blocks[1]!] });
		answer({ code: '#{ "y" }' });
		await new Promise((r) => setTimeout(r, 0));
		expect(editor.template.blocks[0]).toEqual(text('Шапка 2'));
		editor.dispose();
	});

	it('a block the server does not turn into code stays as it is, the reason is told', async () => {
		blockCode.mockRejectedValue(
			new ApiError(422, 'validation', 'Проверьте введённые данные', {
				code: 'validation',
				message: 'Проверьте введённые данные',
				fields: [
					{
						field: 'path',
						code: 'code_unavailable',
						message: 'Этот блок нельзя превратить в Typst-код'
					}
				],
				current_version: null,
				permission: null,
				retry_after_secs: null,
				diagnostics: []
			})
		);
		const editor = editorOf([text('Шапка')]);
		render(BlockList, { editor });
		await choose(await openMenu('Текст · Шапка'), 'Свой Typst-код');
		await waitFor(() =>
			expect(toast.items.map((t) => t.message)).toContain('Этот блок нельзя превратить в Typst-код')
		);
		expect(editor.template.blocks).toEqual([text('Шапка')]);
		expect(editor.canUndo).toBe(false);
		editor.dispose();
	});
});
