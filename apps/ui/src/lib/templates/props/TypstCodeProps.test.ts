import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import type { CodeDiagnostic } from '$api/CodeDiagnostic';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import TypstCodeProps from './TypstCodePropsHarness.test-helper.svelte';

const api = vi.hoisted(() => ({
	documentFields: vi.fn(async () => [
		{ key: 'client.name', placeholder: 'клиент.имя', label: 'Клиент' },
		{ key: 'order.number', placeholder: 'заказ.номер', label: 'Номер заказа' }
	])
}));
vi.mock('#lib/api/document-templates.js', async (original) => ({
	...(await original<typeof import('#lib/api/document-templates.js')>()),
	documentFields: api.documentFields
}));

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

vi.mock('#lib/app/context.svelte.js', () => ({
	app: {
		settings: {
			firm_links: [
				{ id: 'telegram', name: 'Telegram', url: 'https://t.example/x', enabled: true },
				{ id: 'off', name: 'Выкл', url: 'https://off.example', enabled: false }
			]
		}
	}
}));

const code = (s: string): Block => ({ copies: 'both', body: { type: 'typst_code', code: s } });
const columns = (a: Block[], b: Block[]): Block => ({
	copies: 'both',
	body: { type: 'columns', columns: [a, b], gap_mm: 6 }
});

function setup(blocks: Block[], path: number[]) {
	const editor = editorOf(templateOf('intake_act', blocks));
	render(TypstCodeProps, { editor, path });
	const codeAt = () => {
		const b =
			path.length === 1
				? editor.template.blocks[path[0]!]!.body
				: (() => {
						const c = editor.template.blocks[path[0]!]!.body;
						if (c.type !== 'columns') throw new Error('not columns');
						return c.columns[path[1]!]![path[2]!]!.body;
					})();
		if (b.type !== 'typst_code') throw new Error('not code');
		return b.code;
	};
	return { editor, codeAt, field: () => screen.getByRole('textbox', { name: 'Typst-код' }) };
}

const diagnostic = (block: number[] | null, message: string, hints: string[] = []) =>
	({
		place: block && { block, line: 2, column: 3 },
		message,
		hints
	}) satisfies CodeDiagnostic;

describe('TypstCodeProps', () => {
	it('what is typed is the code of the block, one step to undo', async () => {
		const { editor, codeAt, field } = setup([code('#v("order.number")\n')], [0]);
		expect(field()).toHaveValue('#v("order.number")\n');
		await userEvent.type(field(), '#copy');
		expect(codeAt()).toBe('#v("order.number")\n#copy');
		editor.undo();
		expect(codeAt()).toBe('#v("order.number")\n');
	});

	it('the field takes as much code as the model: 20 000 characters', () => {
		const { field } = setup([code('')], [0]);
		expect(field()).toHaveAttribute('maxlength', '20000');
		expect(field()).toHaveAccessibleDescription(/v\("order.number"\) и v1\(…\)/);
	});

	it('the errors of this block show under the field with their line; those of others do not', async () => {
		const { editor } = setup([code('a\nb #foo'), code('c')], [0]);
		editor.diagnostics = [
			diagnostic([0], 'unknown variable: foo', ['check the spelling']),
			diagnostic([1], 'another block'),
			diagnostic(null, 'no place')
		];
		await tick();
		expect(
			screen.getByText('Строка 2, знак 3: unknown variable: foo — check the spelling')
		).toBeInTheDocument();
		expect(screen.queryByText(/another block/)).toBeNull();
		expect(screen.queryByText(/no place/)).toBeNull();
		// The next preview without errors takes them away.
		editor.diagnostics = [];
		await tick();
		expect(screen.queryByText(/unknown variable/)).toBeNull();
	});

	it('in a column the path is matched whole: [0, 1, 0] is not [0]', async () => {
		const { editor } = setup([columns([], [code('x\ny')])], [0, 1, 0]);
		editor.diagnostics = [diagnostic([0], 'the columns'), diagnostic([0, 1, 0], 'this code')];
		await tick();
		expect(screen.getByText('Строка 2, знак 3: this code')).toBeInTheDocument();
		expect(screen.queryByText(/the columns/)).toBeNull();
	});

	it('after «Экземпляры» comes the row of badges, with the QR of an enabled link', () => {
		setup([code('')], [0]);
		const copies = screen.getByText('Экземпляры');
		const badge = screen.getByRole('button', { name: 'Вставить: Номер заказа' });
		expect(copies.compareDocumentPosition(badge) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
		expect(screen.getByRole('button', { name: 'Вставить: QR-код Telegram' })).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /QR-код Выкл/ })).toBeNull();
	});

	it('a badge puts the markup at the caret, the caret after it and the focus in the editor', async () => {
		const { codeAt, field } = setup([code('Заказ ')], [0]);
		const box = field() as HTMLTextAreaElement;
		box.focus();
		box.setSelectionRange(6, 6);
		await userEvent.click(screen.getByRole('button', { name: 'Вставить: Номер заказа' }));
		expect(codeAt()).toBe('Заказ #v("order.number")');
		await waitFor(() => expect(box).toHaveFocus());
		expect(box.selectionStart).toBe(24);
		expect(box.selectionEnd).toBe(24);
	});

	it('«Только в экземпляре …» leaves the caret inside its brackets', async () => {
		const { codeAt, field } = setup([code('Текст ')], [0]);
		const box = field() as HTMLTextAreaElement;
		box.focus();
		box.setSelectionRange(6, 6);
		await userEvent.click(
			screen.getByRole('button', { name: 'Вставить: Только в экземпляре клиента' })
		);
		const inserted = 'Текст #if copy == "client" [ ]';
		expect(codeAt()).toBe(inserted);
		await waitFor(() => expect(box).toHaveFocus());
		// Right after `[`: what is typed goes into the client copy.
		await waitFor(() => expect(box.selectionStart).toBe(inserted.indexOf('[') + 1));
		expect(box.selectionEnd).toBe(box.selectionStart);
		box.setSelectionRange(0, 0);
		await userEvent.click(
			screen.getByRole('button', { name: 'Вставить: Только в экземпляре сервиса' })
		);
		const both = '#if copy == "service" [ ]' + inserted;
		expect(codeAt()).toBe(both);
		await waitFor(() => expect(box.selectionStart).toBe(both.indexOf('[') + 1));
	});

	it('in code mode a badge puts the code without #', async () => {
		const { codeAt, field } = setup([code('#{ ')], [0]);
		const box = field() as HTMLTextAreaElement;
		box.focus();
		box.setSelectionRange(3, 3);
		await userEvent.click(screen.getByRole('button', { name: 'Вставить: Отступ' }));
		expect(codeAt()).toBe('#{ vspace(5mm)');
	});

	it('the last badge is «Другое поле…» and opens the search of fields', async () => {
		const { codeAt, field } = setup([code('ab\ncd')], [0]);
		const chips = screen.getAllByRole('button', { name: /^Вставить: / });
		expect(chips.at(-1)).toHaveAccessibleName('Вставить: Другое поле…');
		expect(screen.queryByRole('button', { name: 'Вставить поле' })).toBeNull();
		const box = field() as HTMLTextAreaElement;
		box.focus();
		box.setSelectionRange(2, 2);
		await userEvent.click(chips.at(-1)!);
		await waitFor(() => expect(api.documentFields).toHaveBeenCalled());
		await userEvent.type(screen.getByRole('combobox', { name: 'Поле' }), 'номер{Enter}');
		expect(codeAt()).toBe('ab#v("order.number")\ncd');
	});

	it('«Другое поле…» inside code mode puts v("key") without #', async () => {
		const { codeAt, field } = setup([code('#{\n\n}\n')], [0]);
		const box = field() as HTMLTextAreaElement;
		box.focus();
		box.setSelectionRange(3, 3);
		await userEvent.click(screen.getByRole('button', { name: 'Вставить: Другое поле…' }));
		await waitFor(() => expect(api.documentFields).toHaveBeenCalled());
		await userEvent.type(screen.getByRole('combobox', { name: 'Поле' }), 'номер{Enter}');
		expect(codeAt()).toBe('#{\nv("order.number")\n}\n');
	});
});
