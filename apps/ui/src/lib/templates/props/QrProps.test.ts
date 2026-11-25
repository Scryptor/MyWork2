import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import QrProps from './QrProps.svelte';

const settings = vi.hoisted(() => ({
	firm_links: [{ id: 'telegram', name: 'Telegram', url: '', enabled: true }]
}));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { settings } }));

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});
beforeEach(() => {
	settings.firm_links[0]!.url = '';
});

const qr = (link: string): Block => ({
	copies: 'both',
	body: { type: 'qr', link, size_mm: 20, caption: 'Мы в Telegram', align: 'center' }
});

/** Picks `label` in «Откуда ссылка», opened from the keyboard: jsdom does not place the list. */
async function choose(label: string) {
	screen.getByRole('combobox', { name: 'Откуда ссылка' }).focus();
	await userEvent.keyboard('{Enter}');
	const options = await screen.findAllByRole('option', { hidden: true });
	await userEvent.click(options.find((o) => o.textContent?.trim() === label)!);
}

function setup(link: string) {
	const editor = editorOf(templateOf('intake_act', [qr(link)]));
	render(QrProps, { editor, path: [0] });
	const body = () => {
		const b = editor.template.blocks[0]!.body;
		if (b.type !== 'qr') throw new Error('not a qr');
		return b;
	};
	return { editor, body };
}

describe('QrProps', () => {
	it('«Своя ссылка» shows the link field and starts it from https://', async () => {
		const { body } = setup('{фирма.телеграм}');
		expect(screen.queryByRole('textbox', { name: 'Ссылка' })).toBeNull();
		// Opened from the keyboard: jsdom does not place the list, so its options are hidden.
		screen.getByRole('combobox', { name: 'Откуда ссылка' }).focus();
		await userEvent.keyboard('{Enter}');
		const options = await screen.findAllByRole('option', { hidden: true });
		await userEvent.click(options.find((o) => o.textContent?.trim() === 'Своя ссылка')!);
		const field = await screen.findByRole('textbox', { name: 'Ссылка' });
		expect(field).toHaveValue('https://');
		expect(body().link).toBe('https://');
	});

	it('the list and the field are named apart', () => {
		setup('https://a.b');
		expect(screen.getByRole('combobox', { name: 'Откуда ссылка' })).toBeInTheDocument();
		expect(screen.queryByRole('combobox', { name: 'Ссылка' })).toBeNull();
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toBeInTheDocument();
	});

	it('the field takes no more than the model', () => {
		setup('https://a.b');
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toHaveAttribute('maxlength', '500');
	});

	it('a link typed into the firm one stays its own: the field does not go', async () => {
		const { body } = setup('https://a.b');
		const field = screen.getByRole('textbox', { name: 'Ссылка' });
		await userEvent.clear(field);
		await userEvent.type(field, '{{фирма.телеграм}');
		expect(body().link).toBe('{фирма.телеграм}');
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toBe(field);
		expect(field).toHaveFocus();
		await userEvent.type(field, '?start=1');
		expect(body().link).toBe('{фирма.телеграм}?start=1');
	});

	it('«Своя ссылка» chosen again keeps the link typed', async () => {
		const { body } = setup('https://a.b');
		const field = screen.getByRole('textbox', { name: 'Ссылка' });
		await userEvent.clear(field);
		await userEvent.type(field, '{{фирма.телеграм}');
		await choose('Своя ссылка');
		expect(body().link).toBe('{фирма.телеграм}');
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toHaveValue('{фирма.телеграм}');
	});

	it('an undo back to a firm link shows it as the firm one', async () => {
		const { editor, body } = setup('{фирма.телеграм}');
		await choose('Своя ссылка');
		expect(body().link).toBe('https://');
		editor.undo();
		await tick();
		expect(body().link).toBe('{фирма.телеграм}');
		expect(screen.queryByRole('textbox', { name: 'Ссылка' })).toBeNull();
		expect(screen.getByRole('combobox', { name: 'Откуда ссылка' })).toHaveTextContent(
			'Telegram фирмы'
		);
	});

	it('a preset chosen in the list hides the field', async () => {
		const { body } = setup('https://a.b');
		await choose('Google Карты фирмы');
		expect(body().link).toBe('{фирма.гугл_карты}');
		expect(screen.queryByRole('textbox', { name: 'Ссылка' })).toBeNull();
	});

	it('a link of its own is typed into the field', async () => {
		const { body } = setup('https://a.b');
		const field = screen.getByRole('textbox', { name: 'Ссылка' });
		await userEvent.type(field, '/c');
		expect(body().link).toBe('https://a.b/c');
	});

	it('an edit of the size writes size_mm', async () => {
		const { body } = setup('{фирма.телеграм}');
		const field = screen.getByRole('textbox', { name: 'Размер, мм' });
		await userEvent.clear(field);
		await userEvent.type(field, '35');
		expect(body().size_mm).toBe(35);
	});

	it('the caption and the alignment are edited too', async () => {
		const { body } = setup('{фирма.телеграм}');
		const field = screen.getByRole('textbox', { name: 'Подпись под кодом' });
		await userEvent.clear(field);
		await userEvent.type(field, 'Привет');
		await userEvent.click(screen.getByRole('radio', { name: 'Справа' }));
		expect(body().caption).toBe('Привет');
		expect(body().align).toBe('right');
	});

	it('an empty setting of the preset shows a hint with a link to the settings', () => {
		setup('{фирма.телеграм}');
		expect(screen.getByText(/Ссылка не заполнена — код не печатается/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Документы и реквизиты' })).toHaveAttribute(
			'href',
			expect.stringContaining('settings/documents')
		);
	});

	it('a filled setting shows no hint', () => {
		settings.firm_links[0]!.url = 'https://t.me/firm';
		setup('{фирма.телеграм}');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});

	it('a link of its own shows no hint', () => {
		setup('https://a.b');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});
});
