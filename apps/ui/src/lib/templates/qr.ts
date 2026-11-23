import type { BlockBody } from '$api/BlockBody';

export type QrBody = Extract<BlockBody, { type: 'qr' }>;
export type QrPreset = 'telegram' | 'google_maps' | 'yandex_maps';

/** The firm's links a «QR-код» block starts from: the substitution, the setting it comes from, the caption. */
export const QR_PRESETS: Record<
	QrPreset,
	{
		link: string;
		setting: 'firm_telegram' | 'firm_google_maps' | 'firm_yandex_maps';
		caption: string;
	}
> = {
	telegram: { link: '{фирма.телеграм}', setting: 'firm_telegram', caption: 'Мы в Telegram' },
	google_maps: {
		link: '{фирма.гугл_карты}',
		setting: 'firm_google_maps',
		caption: 'Мы на Google Картах'
	},
	yandex_maps: {
		link: '{фирма.яндекс_карты}',
		setting: 'firm_yandex_maps',
		caption: 'Мы на Яндекс Картах'
	}
};

const PRESETS = Object.keys(QR_PRESETS) as QrPreset[];

/** The server's rule (`qr_name`): spaces at the ends do not count, nor the case, nor «ё» against «е». */
const normal = (s: string) => s.trim().toLowerCase().replaceAll('ё', 'е');

/** The preset whose substitution is the whole link, or null for a link of its own. */
export function qrPreset(link: string): QrPreset | null {
	const key = normal(link);
	return PRESETS.find((p) => normal(QR_PRESETS[p].link) === key) ?? null;
}

/**
 * A preset sets its link and, when the caption is empty or another preset's, its caption; «своя ссылка»
 * starts from `https://` when there was a preset and leaves the caption alone.
 */
export function choosePreset(body: QrBody, preset: QrPreset | 'custom'): QrBody {
	if (preset === 'custom') {
		return qrPreset(body.link) === null ? body : { ...body, link: 'https://' };
	}
	const own = body.caption !== '' && !PRESETS.some((p) => QR_PRESETS[p].caption === body.caption);
	return {
		...body,
		link: QR_PRESETS[preset].link,
		caption: own ? body.caption : QR_PRESETS[preset].caption
	};
}
