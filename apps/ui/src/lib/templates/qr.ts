import type { BlockBody } from '$api/BlockBody';
import { normalizeSearch } from '#lib/format/search.js';
import { ru } from '#lib/i18n/ru.js';

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
	telegram: {
		link: '{фирма.телеграм}',
		setting: 'firm_telegram',
		caption: ru.templates.defaults.qrCaptions.telegram
	},
	google_maps: {
		link: '{фирма.гугл_карты}',
		setting: 'firm_google_maps',
		caption: ru.templates.defaults.qrCaptions.google_maps
	},
	yandex_maps: {
		link: '{фирма.яндекс_карты}',
		setting: 'firm_yandex_maps',
		caption: ru.templates.defaults.qrCaptions.yandex_maps
	}
};

const PRESETS = Object.keys(QR_PRESETS) as QrPreset[];

/**
 * The name of a link that is one substitution and spaces around it, as the server reads it: spaces
 * around do not count (`qr_name`), the name in the braces is compared by `normalize_search`
 * (`field_by_placeholder`). Null for any other link.
 */
function substitution(link: string): string | null {
	const name = /^\s*\{([^{}]*)\}\s*$/.exec(link)?.[1];
	return name === undefined ? null : normalizeSearch(name);
}

/** The preset whose substitution is the whole link, or null for a link of its own. */
export function qrPreset(link: string): QrPreset | null {
	const name = substitution(link);
	if (name === null) return null;
	return PRESETS.find((p) => substitution(QR_PRESETS[p].link) === name) ?? null;
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
