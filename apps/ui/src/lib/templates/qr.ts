import type { BlockBody } from '$api/BlockBody';
import type { FirmLink } from '$api/FirmLink';
import { normalizeSearch } from '#lib/format/search.js';
import { ru } from '#lib/i18n/ru.js';

export type QrBody = Extract<BlockBody, { type: 'qr' }>;

/** The substitution of a link of the list: `{ссылка.<id>}`. */
export function linkSubstitution(id: string): string {
	return `{ссылка.${id}}`;
}

/** The names of the three links that were fields before the list of links, and their ids. */
const LEGACY_NAMES: Record<string, string> = {
	'фирма.телеграм': 'telegram',
	'фирма.гугл_карты': 'google_maps',
	'фирма.яндекс_карты': 'yandex_maps'
};

const PREFIX = 'ссылка.';

/** `record[key]` of the record's own keys only: `constructor` and the like of `Object.prototype` are not in it. */
function own(record: Record<string, string>, key: string): string | undefined {
	return Object.hasOwn(record, key) ? record[key] : undefined;
}

/**
 * The id of the link of a block that is one substitution and spaces around it, as the server reads it:
 * spaces around do not count (`qr_name`), the name in the braces is compared by `normalize_search`
 * (`link_by_placeholder`). Any well-formed id counts, the list having it or not. Null for any other link.
 */
export function qrLinkId(link: string): string | null {
	const name = /^\s*\{([^{}]*)\}\s*$/.exec(link)?.[1];
	if (name === undefined) return null;
	const wanted = normalizeSearch(name);
	const legacy = own(LEGACY_NAMES, wanted);
	if (legacy !== undefined) return legacy;
	if (!wanted.startsWith(PREFIX)) return null;
	const id = wanted.slice(PREFIX.length);
	return /^[a-z0-9_]{1,40}$/.test(id) ? id : null;
}

/** The caption a block starts from for `link`: the default text of a built-in link, else its name. */
export function linkCaption(link: FirmLink): string {
	return own(ru.templates.defaults.qrCaptions, link.id) ?? link.name;
}

/** The name of a link by its id when the list does not have it: of a built-in or an old one, else the id. */
export function linkName(id: string): string {
	return own(ru.templates.qrLinkNames, id) ?? id;
}

/**
 * A link of the list sets its substitution and, when the caption is empty or another link's, its caption;
 * «своя ссылка» starts from `https://` when there was a link of the list and leaves the caption alone.
 */
export function chooseLink(
	body: QrBody,
	choice: string | 'custom',
	links: readonly FirmLink[]
): QrBody {
	if (choice === 'custom') {
		return qrLinkId(body.link) === null ? body : { ...body, link: 'https://' };
	}
	const chosen = links.find((l) => l.id === choice);
	const known = new Set([
		...Object.values(ru.templates.defaults.qrCaptions),
		...links.flatMap((l) => [l.name, linkCaption(l)])
	]);
	const own = body.caption !== '' && !known.has(body.caption);
	return {
		...body,
		link: linkSubstitution(choice),
		caption: own ? body.caption : chosen ? linkCaption(chosen) : body.caption
	};
}
