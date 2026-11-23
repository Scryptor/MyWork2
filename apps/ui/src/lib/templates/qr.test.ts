import { describe, expect, it } from 'vitest';
import { choosePreset, qrPreset, type QrBody } from './qr.js';

const body = (link: string, caption: string): QrBody => ({
	type: 'qr',
	link,
	size_mm: 20,
	caption,
	align: 'center'
});

describe('qrPreset', () => {
	it('knows a link of one firm substitution, whatever the spaces, case and «ё»', () => {
		expect(qrPreset('{фирма.телеграм}')).toBe('telegram');
		expect(qrPreset(' {Фирма.Яндекс_Карты} ')).toBe('yandex_maps');
		expect(qrPreset('{фирма.гугл_карты}')).toBe('google_maps');
	});

	it('is null for a link of its own or with more than the substitution', () => {
		expect(qrPreset('https://t.me/x')).toBeNull();
		expect(qrPreset('{фирма.телеграм}/x')).toBeNull();
		expect(qrPreset('')).toBeNull();
	});
});

describe('choosePreset', () => {
	it('a preset sets its link and its caption over an empty one', () => {
		expect(choosePreset(body('https://x', ''), 'google_maps')).toEqual(
			body('{фирма.гугл_карты}', 'Мы на Google Картах')
		);
	});

	it('a preset replaces the caption of another preset', () => {
		expect(choosePreset(body('{фирма.телеграм}', 'Мы в Telegram'), 'yandex_maps')).toEqual(
			body('{фирма.яндекс_карты}', 'Мы на Яндекс Картах')
		);
	});

	it('a preset keeps a caption written by hand', () => {
		expect(choosePreset(body('{фирма.телеграм}', 'Пишите нам'), 'yandex_maps')).toEqual(
			body('{фирма.яндекс_карты}', 'Пишите нам')
		);
	});

	it('«своя ссылка» starts from https:// after a preset and keeps the caption', () => {
		expect(choosePreset(body('{фирма.телеграм}', 'Мы в Telegram'), 'custom')).toEqual(
			body('https://', 'Мы в Telegram')
		);
	});

	it('«своя ссылка» leaves a link of its own as it is', () => {
		const b = body('https://a.b/c', 'x');
		expect(choosePreset(b, 'custom')).toBe(b);
	});
});
