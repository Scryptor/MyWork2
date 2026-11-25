import { render, screen, waitFor, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import { editorOf, templateOf } from './editor-fixtures.test-helper.js';
import TypstCodeProps from './props/TypstCodePropsHarness.test-helper.svelte';
import TypstHelp from './TypstHelp.svelte';

vi.mock('#lib/app/context.svelte.js', () => ({ app: { settings: { firm_links: [] } } }));
vi.mock('#lib/api/document-templates.js', async (original) => ({
	...(await original<typeof import('#lib/api/document-templates.js')>()),
	documentFields: vi.fn(async () => [])
}));

const code: Block = { copies: 'both', body: { type: 'typst_code', code: '' } };

const TITLES = [
	'Что это за блок',
	'Значение поля',
	'Условие',
	'Таблица',
	'QR-код',
	'Отступ',
	'Только в одном экземпляре',
	'Ограничения'
];

describe('TypstHelp', () => {
	it('the button of the code block opens the dialog with the eight sections in order', async () => {
		const editor = editorOf(templateOf('intake_act', [code]));
		render(TypstCodeProps, { editor, path: [0] });
		await userEvent.click(screen.getByRole('button', { name: 'Справка по коду' }));
		const dialog = await screen.findByRole('dialog', { name: 'Справка по Typst-коду' });
		const headings = within(dialog).getAllByRole('heading', { level: 3 });
		expect(headings.map((h) => h.textContent)).toEqual(TITLES);
	});

	it('shows the code of an example in a monospace <pre>', () => {
		render(TypstHelp, { open: true });
		const pre = screen.getByText('Заказ № #v("order.number") от #v("order.received")');
		expect(pre.tagName).toBe('PRE');
	});

	it('Esc closes it', async () => {
		render(TypstHelp, { open: true });
		await screen.findByRole('dialog', { name: 'Справка по Typst-коду' });
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});
});
