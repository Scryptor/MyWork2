import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import LegacyTemplateDialog from './LegacyTemplateDialog.svelte';

const TEXT = 'Акт № {заказ.номер}\nКлиент: {клиент.фио}';

describe('LegacyTemplateDialog', () => {
	it('shows the text with its line breaks', () => {
		render(LegacyTemplateDialog, { open: true, text: TEXT, oninsert: vi.fn() });
		const shown = screen.getByLabelText('Шаблон из MyWork 2', { selector: 'pre' });
		expect(shown.textContent).toBe(TEXT);
	});

	it('«Вставить текстом» hands the text over', async () => {
		const oninsert = vi.fn();
		render(LegacyTemplateDialog, { open: true, text: TEXT, oninsert });
		await userEvent.click(screen.getByRole('button', { name: 'Вставить текстом' }));
		expect(oninsert).toHaveBeenCalledExactlyOnceWith(TEXT);
	});

	it('«Закрыть» inserts nothing', async () => {
		const oninsert = vi.fn();
		render(LegacyTemplateDialog, { open: true, text: TEXT, oninsert });
		// The dialog has its own «✕» named the same: the footer button is the last one.
		const close = screen.getAllByRole('button', { name: 'Закрыть' }).at(-1)!;
		await userEvent.click(close);
		expect(oninsert).not.toHaveBeenCalled();
		await waitFor(() =>
			expect(screen.queryByRole('button', { name: 'Вставить текстом' })).toBeNull()
		);
	});
});
