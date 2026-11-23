import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
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
const fields = (rows: FieldRow[]): Block => ({ copies: 'both', body: { type: 'fields', rows } });

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
});
