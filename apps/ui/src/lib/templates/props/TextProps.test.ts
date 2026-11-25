import { cleanup, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import type { Block } from '$api/Block';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import TextProps from './TextProps.svelte';

const text = (patch: Record<string, unknown> = {}): Block => ({
	copies: 'both',
	body: {
		type: 'text',
		text: 'а',
		size_pt: null,
		bold: false,
		align: 'left',
		one_line: false,
		frame: false,
		numbered: false,
		...patch
	}
});

function setup(patch?: Record<string, unknown>) {
	const editor = editorOf(templateOf('intake_act', [text(patch)]));
	render(TextProps, { editor, path: [0] });
	const body = () => {
		const b = editor.template.blocks[0]!.body;
		if (b.type !== 'text') throw new Error('not text');
		return b;
	};
	return { editor, body };
}

describe('TextProps: the frame and the numbered list', () => {
	it('«В рамке» switches the frame', async () => {
		const { body } = setup();
		await userEvent.click(screen.getByRole('switch', { name: 'В рамке' }));
		expect(body().frame).toBe(true);
	});

	it('«Нумерованный список» switches the numbering', async () => {
		const { body } = setup();
		await userEvent.click(screen.getByRole('switch', { name: 'Нумерованный список' }));
		expect(body().numbered).toBe(true);
	});

	it('the numbering is off beside «В одну строку», with the reason', () => {
		setup({ one_line: true });
		expect(screen.getByRole('switch', { name: /Нумерованный список/ })).toBeDisabled();
		expect(screen.getByText('Не действует для текста в одну строку')).toBeInTheDocument();
	});

	it('the numbering is on and has no reason when the text wraps', () => {
		setup();
		expect(screen.getByRole('switch', { name: /Нумерованный список/ })).toBeEnabled();
		expect(screen.queryByText('Не действует для текста в одну строку')).toBeNull();
	});
});

describe('TextProps: the alignment of a numbered list', () => {
	const radios = () => screen.getAllByRole('radio');

	it('is off for a numbered list, with the reason: the list is always on the left', () => {
		setup({ numbered: true, align: 'center' });
		expect(screen.getByRole('radiogroup', { name: 'Выравнивание' })).toHaveAccessibleDescription(
			'Нумерованный список всегда по левому краю'
		);
		for (const radio of radios()) expect(radio).toBeDisabled();
		// It shows how the list is printed; the block keeps its own alignment.
		expect(screen.getByRole('radio', { name: 'Слева' })).toBeChecked();
	});

	it('works for a plain text and for a numbered one in one line (numbering is off there)', async () => {
		const { body } = setup({ numbered: true, one_line: true });
		expect(screen.queryByText('Нумерованный список всегда по левому краю')).toBeNull();
		await userEvent.click(screen.getByRole('radio', { name: 'По центру' }));
		expect(body().align).toBe('center');
		cleanup();
		setup();
		for (const radio of radios()) expect(radio).toBeEnabled();
		expect(screen.queryByText('Нумерованный список всегда по левому краю')).toBeNull();
	});
});
