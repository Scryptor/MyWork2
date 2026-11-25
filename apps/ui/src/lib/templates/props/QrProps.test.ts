import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import QrProps from './QrProps.svelte';

const settings = vi.hoisted(() => ({
	firm_links: [
		{ id: 'yandex_maps', name: 'Яндекс Карты', url: '', enabled: true },
		{ id: 'telegram', name: 'Telegram', url: '', enabled: true },
		{ id: 'link_site', name: 'Сайт', url: 'https://example.com', enabled: true },
		{ id: 'link_off', name: 'Старый сайт', url: 'https://example.org', enabled: false }
	]
}));
vi.mock('#lib/app/context.svelte.js', () => ({ app: { settings } }));

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});
beforeEach(() => {
	settings.firm_links[1]!.url = '';
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
		const { body } = setup('{ссылка.telegram}');
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
		await userEvent.type(field, '{{ссылка.telegram}');
		expect(body().link).toBe('{ссылка.telegram}');
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toBe(field);
		expect(field).toHaveFocus();
		await userEvent.type(field, '?start=1');
		expect(body().link).toBe('{ссылка.telegram}?start=1');
	});

	it('«Своя ссылка» chosen again keeps the link typed', async () => {
		const { body } = setup('https://a.b');
		const field = screen.getByRole('textbox', { name: 'Ссылка' });
		await userEvent.clear(field);
		await userEvent.type(field, '{{ссылка.telegram}');
		await choose('Своя ссылка');
		expect(body().link).toBe('{ссылка.telegram}');
		expect(screen.getByRole('textbox', { name: 'Ссылка' })).toHaveValue('{ссылка.telegram}');
	});

	it('an undo back to a firm link shows it as the firm one', async () => {
		const { editor, body } = setup('{ссылка.telegram}');
		await choose('Своя ссылка');
		expect(body().link).toBe('https://');
		editor.undo();
		await tick();
		expect(body().link).toBe('{ссылка.telegram}');
		expect(screen.queryByRole('textbox', { name: 'Ссылка' })).toBeNull();
		expect(screen.getByRole('combobox', { name: 'Откуда ссылка' })).toHaveTextContent('Telegram');
	});

	it('a link chosen in the list hides the field', async () => {
		const { body } = setup('https://a.b');
		await choose('Сайт');
		expect(body().link).toBe('{ссылка.link_site}');
		expect(screen.queryByRole('textbox', { name: 'Ссылка' })).toBeNull();
	});

	it('lists the links of the settings in their order, then «Своя ссылка»', async () => {
		setup('{ссылка.telegram}');
		screen.getByRole('combobox', { name: 'Откуда ссылка' }).focus();
		await userEvent.keyboard('{Enter}');
		const options = await screen.findAllByRole('option', { hidden: true });
		expect(options.map((o) => o.textContent?.trim())).toEqual([
			'Яндекс Карты',
			'Telegram',
			'Сайт',
			'Старый сайт (выключена)',
			'Своя ссылка'
		]);
	});

	it('a link of an old block that the list lacks is shown by its name, with a hint what to do', () => {
		setup('{фирма.гугл_карты}');
		expect(screen.getByRole('combobox', { name: 'Откуда ссылка' })).toHaveTextContent(
			'Google Карты'
		);
		const settingsLink = screen.getByRole('link', { name: 'Документы и реквизиты' });
		expect(settingsLink.parentElement).toHaveTextContent(
			'Ссылки «Google Карты» нет в списке — выберите другую ссылку или добавьте её в «Документы и реквизиты»',
			{ normalizeWhitespace: true }
		);
		expect(screen.queryByText(/google_maps/)).toBeNull();
	});

	it('a gone link of its own, without a known name, is called by its id', () => {
		setup('{ссылка.link_gone}');
		expect(screen.getByRole('combobox', { name: 'Откуда ссылка' })).toHaveTextContent('link_gone');
		expect(screen.getByText(/Ссылки «link_gone» нет в списке/)).toBeInTheDocument();
	});

	it('a link that is switched off shows a hint with a link to the settings', () => {
		setup('{ссылка.link_off}');
		expect(screen.getByText(/Ссылка выключена — QR-код не печатается/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Документы и реквизиты' })).toHaveAttribute(
			'href',
			expect.stringContaining('settings/documents')
		);
	});

	it('a link of its own is typed into the field', async () => {
		const { body } = setup('https://a.b');
		const field = screen.getByRole('textbox', { name: 'Ссылка' });
		await userEvent.type(field, '/c');
		expect(body().link).toBe('https://a.b/c');
	});

	it('an edit of the size writes size_mm', async () => {
		const { body } = setup('{ссылка.telegram}');
		const field = screen.getByRole('textbox', { name: 'Размер, мм' });
		await userEvent.clear(field);
		await userEvent.type(field, '35');
		expect(body().size_mm).toBe(35);
	});

	it('the caption and the alignment are edited too', async () => {
		const { body } = setup('{ссылка.telegram}');
		const field = screen.getByRole('textbox', { name: 'Подпись под кодом' });
		await userEvent.clear(field);
		await userEvent.type(field, 'Привет');
		await userEvent.click(screen.getByRole('radio', { name: 'Справа' }));
		expect(body().caption).toBe('Привет');
		expect(body().align).toBe('right');
	});

	it('an empty address of the link shows a hint with a link to the settings', () => {
		setup('{ссылка.telegram}');
		expect(screen.getByText(/Ссылка не заполнена — код не печатается/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Документы и реквизиты' })).toHaveAttribute(
			'href',
			expect.stringContaining('settings/documents')
		);
	});

	it('a filled setting shows no hint', () => {
		settings.firm_links[1]!.url = 'https://t.me/firm';
		setup('{ссылка.telegram}');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});

	it('a link of its own shows no hint', () => {
		setup('https://a.b');
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});
});
