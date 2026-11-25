import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeAll, describe, expect, it } from 'vitest';
import type { Block } from '$api/Block';
import type { FieldRow } from '$api/FieldRow';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import FieldsProps from './FieldsPropsHarness.test-helper.svelte';

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

const row = (label: string, value = ''): FieldRow => ({
	label,
	value,
	hide_if_empty: false,
	bold: false
});
const fields = (rows: FieldRow[]): Block => ({
	copies: 'both',
	body: { type: 'fields', rows, grid: false }
});

function setup(rows: FieldRow[]) {
	const editor = editorOf(templateOf('intake_act', [fields(rows)]));
	render(FieldsProps, { editor, path: [0] });
	const body = () => {
		const b = editor.template.blocks[0]!.body;
		if (b.type !== 'fields') throw new Error('not fields');
		return b;
	};
	return { editor, rows: () => body().rows };
}

describe('FieldsProps', () => {
	it('the value of a row is a multi-line field', async () => {
		const { rows } = setup([row('Клиент', 'а')]);
		const value = screen.getByRole('textbox', { name: 'Значение' });
		expect(value.tagName).toBe('TEXTAREA');
		await userEvent.type(value, '{Enter}б');
		expect(rows()[0]!.value).toBe('а\nб');
	});

	it('«Добавить строку» is off at 50 rows, with the reason beside it', () => {
		const fifty = Array.from({ length: 50 }, (_, i) => row(`Поле ${i}`));
		setup(fifty);
		expect(screen.getByRole('button', { name: 'Добавить строку' })).toBeDisabled();
		expect(screen.getByText('Не больше 50 строк')).toBeInTheDocument();
	});

	it('under 50 rows a row is added, no hint is shown', async () => {
		const { rows } = setup([row('Клиент')]);
		expect(screen.queryByText('Не больше 50 строк')).toBeNull();
		await userEvent.click(screen.getByRole('button', { name: 'Добавить строку' }));
		expect(rows()).toHaveLength(2);
	});

	it('an undo of an added row takes it off the screen', async () => {
		const { editor, rows } = setup([row('Клиент')]);
		await userEvent.click(screen.getByRole('button', { name: 'Добавить строку' }));
		expect(screen.getAllByRole('group')).toHaveLength(2);
		editor.undo();
		expect(rows()).toHaveLength(1);
		await Promise.resolve();
		expect(
			screen.getAllByRole('group').filter((g) => g.getAttribute('aria-label')?.startsWith('Строка'))
		).toHaveLength(1);
	});

	/** The titles of the rows on screen, in order. */
	const titles = () =>
		screen.getAllByRole('textbox', { name: 'Название' }).map((e) => (e as HTMLInputElement).value);
	const setRows = (editor: ReturnType<typeof setup>['editor'], list: FieldRow[]) => {
		const t = editor.template;
		editor.edit({ ...t, blocks: [fields(list)] });
	};

	it('rows changed from outside (an edit, an undo, a redo) show on screen, whatever the length', async () => {
		const { editor } = setup([row('А'), row('Б')]);
		expect(titles()).toEqual(['А', 'Б']);
		setRows(editor, [row('А'), row('Б'), row('В')]);
		await tick();
		expect(titles()).toEqual(['А', 'Б', 'В']);
		// The same length with other rows, as «Взять новое» brings them.
		setRows(editor, [row('Г'), row('Д'), row('Е')]);
		await tick();
		expect(titles()).toEqual(['Г', 'Д', 'Е']);
		editor.undo();
		await tick();
		expect(titles()).toEqual(['А', 'Б', 'В']);
		editor.undo();
		await tick();
		expect(titles()).toEqual(['А', 'Б']);
		editor.redo();
		await tick();
		expect(titles()).toEqual(['А', 'Б', 'В']);
	});

	it('the rows are not made again once they are on screen (no temporary keys)', async () => {
		setup([row('А'), row('Б')]);
		const first = screen
			.getAllByRole('group')
			.filter((g) => g.getAttribute('aria-label')?.startsWith('Строка'));
		expect(first).toHaveLength(2);
		await tick();
		await tick();
		expect(first.every((g) => g.isConnected)).toBe(true);
	});

	it('after the list was changed from outside the rows can still be moved with the arrows', async () => {
		const { editor, rows } = setup([row('А'), row('Б')]);
		setRows(editor, [row('А'), row('Б'), row('В')]);
		await tick();
		screen.getByRole('button', { name: 'Переместить „В“' }).focus();
		await userEvent.keyboard('{ArrowUp}');
		await new Promise((r) => setTimeout(r, 500));
		expect(rows().map((r) => r.label)).toEqual(['А', 'В', 'Б']);
	});

	it('«Сетка» switches the grid of the table', async () => {
		const { editor } = setup([row('Клиент')]);
		const grid = () => {
			const b = editor.template.blocks[0]!.body;
			return b.type === 'fields' && b.grid;
		};
		expect(grid()).toBe(false);
		await userEvent.click(screen.getByRole('switch', { name: 'Сетка' }));
		expect(grid()).toBe(true);
		await userEvent.click(screen.getByRole('switch', { name: 'Сетка' }));
		expect(grid()).toBe(false);
	});
});
