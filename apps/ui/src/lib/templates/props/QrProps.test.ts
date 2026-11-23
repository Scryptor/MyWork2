import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import QrProps from './QrProps.svelte';

const settings = vi.hoisted(() => ({
	firm_telegram: '' as string,
	firm_google_maps: '',
	firm_yandex_maps: ''
}));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { settings } }));

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});
beforeEach(() => {
	settings.firm_telegram = '';
});

const qr = (link: string): Block => ({
	copies: 'both',
	body: { type: 'qr', link, size_mm: 20, caption: 'Мы в Telegram', align: 'center' }
});

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
		screen.getByRole('combobox', { name: 'Ссылка' }).focus();
		await userEvent.keyboard('{Enter}');
		const options = await screen.findAllByRole('option', { hidden: true });
		await userEvent.click(options.find((o) => o.textContent?.trim() === 'Своя ссылка')!);
		const field = await screen.findByRole('textbox', { name: 'Ссылка' });
		expect(field).toHaveValue('https://');
		expect(body().link).toBe('https://');
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
		settings.firm_telegram = 'https://t.me/firm';
		setup('{фирма.телеграм}');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});

	it('a link of its own shows no hint', () => {
		setup('https://a.b');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});
});
