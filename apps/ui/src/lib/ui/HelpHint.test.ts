import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import Harness from './HelpHintHarness.test-helper.svelte';

describe('HelpHint', () => {
	it('is a button named after the title', () => {
		render(Harness, { kind: 'hint', text: 'Знак после сумм.' });
		expect(screen.getByRole('button', { name: 'Что такое «Валюта»' })).toBeInTheDocument();
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('Enter opens a dialog with the title and the text; Esc closes it and returns focus', async () => {
		render(Harness, { kind: 'hint', text: 'Знак после сумм.' });
		const button = screen.getByRole('button', { name: 'Что такое «Валюта»' });
		button.focus();
		await userEvent.keyboard('{Enter}');
		const dialog = await screen.findByRole('dialog', { name: 'Валюта' });
		expect(dialog).toHaveTextContent('Знак после сумм.');
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		await waitFor(() => expect(button).toHaveFocus());
	});

	it('«Понятно» closes the dialog', async () => {
		render(Harness, { kind: 'hint', text: 'Знак после сумм.' });
		await userEvent.click(screen.getByRole('button', { name: 'Что такое «Валюта»' }));
		await userEvent.click(await screen.findByRole('button', { name: 'Понятно' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});

	describe('Ctrl/⌘+Enter while open', () => {
		afterEach(() => {
			vi.restoreAllMocks();
		});

		async function openInDialog() {
			const onsubmit = vi.fn();
			render(Harness, { kind: 'dialog', text: 'Что сломалось', onsubmit });
			await userEvent.click(await screen.findByRole('button', { name: 'Что такое «Жалоба»' }));
			expect(await screen.findByRole('dialog', { name: 'Жалоба' })).toBeInTheDocument();
			return onsubmit;
		}

		it('Ctrl+Enter closes the hint, not the dialog underneath; then it is the dialog’s again', async () => {
			const onsubmit = await openInDialog();
			await userEvent.keyboard('{Control>}{Enter}{/Control}');
			await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Жалоба' })).toBeNull());
			expect(onsubmit).not.toHaveBeenCalled();
			expect(screen.getByRole('dialog', { name: 'Принять' })).toBeInTheDocument();
			await userEvent.keyboard('{Control>}{Enter}{/Control}');
			expect(onsubmit).toHaveBeenCalledTimes(1);
		});

		it('⌘+Enter on a Mac closes the hint, not the dialog underneath', async () => {
			vi.spyOn(navigator, 'platform', 'get').mockReturnValue('MacIntel');
			const onsubmit = await openInDialog();
			await userEvent.keyboard('{Meta>}{Enter}{/Meta}');
			await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Жалоба' })).toBeNull());
			expect(onsubmit).not.toHaveBeenCalled();
		});
	});

	it('two paragraphs are two <p>', async () => {
		render(Harness, { kind: 'hint', text: ['Первое.', 'Второе.'] });
		await userEvent.click(screen.getByRole('button', { name: 'Что такое «Валюта»' }));
		const dialog = await screen.findByRole('dialog');
		const paragraphs = [...dialog.querySelectorAll('p')].map((p) => p.textContent);
		expect(paragraphs).toEqual(['Первое.', 'Второе.']);
	});
});

describe('help prop', () => {
	it('Field: the icon is next to the label, outside <label>, and does not focus the field', async () => {
		render(Harness, { kind: 'field', text: 'Как в паспорте' });
		const button = screen.getByRole('button', { name: 'Что такое «Имя»' });
		expect(button.closest('label')).toBeNull();
		await userEvent.click(button);
		expect(await screen.findByRole('dialog', { name: 'Имя' })).toHaveTextContent('Как в паспорте');
		expect(document.getElementById('x')).not.toHaveFocus();
	});

	it('Field without help has no icon', () => {
		render(Harness, { kind: 'field' });
		expect(screen.getAllByRole('button')).toHaveLength(1);
	});

	it('Switch: the icon is outside <label> and does not toggle the switch', async () => {
		render(Harness, { kind: 'switch', text: 'Что делает переключатель' });
		const button = screen.getByRole('button', { name: 'Что такое «Включено»' });
		expect(button.closest('label')).toBeNull();
		await userEvent.click(button);
		expect(await screen.findByRole('dialog', { name: 'Включено' })).toHaveTextContent(
			'Что делает переключатель'
		);
		expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
	});

	it('Section: the icon follows the heading', () => {
		render(Harness, { kind: 'section', text: ['а', 'б'] });
		expect(
			screen.getByRole('button', { name: 'Что такое «Требуют внимания»' })
		).toBeInTheDocument();
	});

	it('ChipGroup: the icon ends the row of chips and does not press a chip', async () => {
		render(Harness, { kind: 'chips', text: 'Что сдано с устройством' });
		const button = screen.getByRole('button', { name: 'Что такое «Комплектность»' });
		await userEvent.click(button);
		expect(await screen.findByRole('dialog', { name: 'Комплектность' })).toHaveTextContent(
			'Что сдано с устройством'
		);
		expect(screen.getByRole('button', { name: 'Чехол' })).toHaveAttribute('aria-pressed', 'false');
	});
});
