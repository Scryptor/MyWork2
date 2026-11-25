import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import InsertField from './InsertField.svelte';

const api = vi.hoisted(() => ({
	documentFields: vi.fn(async (kind?: string) =>
		kind === 'cell_sticker'
			? [{ key: 'cell.code', placeholder: 'ячейка.код', label: 'Код ячейки' }]
			: []
	)
}));
vi.mock('#lib/api/document-templates.js', () => ({ documentFields: api.documentFields }));

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

describe('InsertField', () => {
	it('asks for the fields of its document', async () => {
		render(InsertField, {
			props: { kind: 'cell_sticker', target: null, value: '', onchange: () => {} }
		});
		await userEvent.click(screen.getByRole('button', { name: 'Вставить поле' }));
		await waitFor(() => expect(api.documentFields).toHaveBeenCalledWith('cell_sticker'));
		await userEvent.type(screen.getByRole('combobox', { name: 'Поле' }), 'код');
		expect(await screen.findByText('Код ячейки')).toBeTruthy();
	});
});
