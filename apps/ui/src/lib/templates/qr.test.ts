import { describe, expect, it } from 'vitest';
import type { FirmLink } from '$api/FirmLink';
import { ru } from '#lib/i18n/ru.js';
import { chooseLink, linkCaption, qrLinkId, type QrBody } from './qr.js';

const body = (link: string, caption: string): QrBody => ({
	type: 'qr',
	link,
	size_mm: 20,
	caption,
	align: 'center'
});

const link = (id: string, name: string, enabled = true): FirmLink => ({
	id,
	name,
	url: '',
	enabled
});
const links = [
	link('yandex_maps', 'Яндекс Карты'),
	link('2gis', '2ГИС'),
	link('telegram', 'Telegram'),
	link('max', 'MAX'),
	link('link_site', 'Сайт')
];

describe('qrLinkId', () => {
	it('knows a link of one substitution, whatever the spaces, case and «ё»', () => {
		expect(qrLinkId('{ссылка.2gis}')).toBe('2gis');
		expect(qrLinkId(' {Ссылка.Telegram} ')).toBe('telegram');
		expect(qrLinkId(' {Фирма.Телеграм} ')).toBe('telegram');
		expect(qrLinkId('{фирма.яндекс_карты}')).toBe('yandex_maps');
		expect(qrLinkId('{фирма.гугл_карты}')).toBe('google_maps');
	});

	it('reads the name in the braces as the server does: spaces inside, NFC', () => {
		expect(qrLinkId('{ ссылка.telegram }')).toBe('telegram');
		expect(qrLinkId('{\tФирма.Гугл_Карты\n}')).toBe('google_maps');
		// «ё» as «е» and a combining diaeresis: NFC makes it «ё», then it is «е».
		expect(qrLinkId('{фирма.телёграм}')).toBe('telegram');
		expect(qrLinkId('{фирма. телеграм}')).toBeNull();
		expect(qrLinkId('{ ссылка.telegram }x')).toBeNull();
	});

	it('takes any well-formed id, in the list or not', () => {
		expect(qrLinkId('{ссылка.link_gone}')).toBe('link_gone');
		expect(qrLinkId('{ссылка.Bad-Id}')).toBeNull();
		expect(qrLinkId('{ссылка.}')).toBeNull();
	});

	it('is null for a link of its own or with more than the substitution', () => {
		expect(qrLinkId('https://x')).toBeNull();
		expect(qrLinkId('{ссылка.telegram}/x')).toBeNull();
		expect(qrLinkId('{заказ.номер}')).toBeNull();
		expect(qrLinkId('')).toBeNull();
	});
});

describe('linkCaption', () => {
	it('takes the captions of the built-in links and Google Maps from the default texts', () => {
		const captions = ru.templates.defaults.qrCaptions;
		expect(linkCaption(links[0]!)).toBe(captions.yandex_maps);
		expect(linkCaption(links[1]!)).toBe('Мы в 2ГИС');
		expect(linkCaption(links[2]!)).toBe('Мы в Telegram');
		expect(linkCaption(links[3]!)).toBe('Мы в MAX');
		expect(linkCaption(link('google_maps', 'Google Карты'))).toBe('Мы на Google Картах');
	});

	it('is the name of an own link', () => {
		expect(linkCaption(links[4]!)).toBe('Сайт');
	});
});

describe('chooseLink', () => {
	it('sets the substitution and a caption over an empty one', () => {
		expect(chooseLink(body('https://x', ''), '2gis', links)).toEqual(
			body('{ссылка.2gis}', 'Мы в 2ГИС')
		);
	});

	it('replaces the caption of another link, old ones included', () => {
		expect(chooseLink(body('{ссылка.telegram}', 'Мы в Telegram'), 'max', links)).toEqual(
			body('{ссылка.max}', 'Мы в MAX')
		);
		expect(chooseLink(body('{фирма.гугл_карты}', 'Мы на Google Картах'), 'max', links)).toEqual(
			body('{ссылка.max}', 'Мы в MAX')
		);
		expect(chooseLink(body('{ссылка.link_site}', 'Сайт'), 'telegram', links)).toEqual(
			body('{ссылка.telegram}', 'Мы в Telegram')
		);
	});

	it('keeps a caption written by hand', () => {
		expect(chooseLink(body('{ссылка.telegram}', 'Пишите нам'), 'max', links)).toEqual(
			body('{ссылка.max}', 'Пишите нам')
		);
	});

	it('«своя ссылка» starts from https:// after a link of the list and keeps the caption', () => {
		expect(chooseLink(body('{ссылка.telegram}', 'Мы в Telegram'), 'custom', links)).toEqual(
			body('https://', 'Мы в Telegram')
		);
	});

	it('«своя ссылка» leaves a link of its own as it is', () => {
		const b = body('https://a.b/c', 'x');
		expect(chooseLink(b, 'custom', links)).toBe(b);
	});
});
