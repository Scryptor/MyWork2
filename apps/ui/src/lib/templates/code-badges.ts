import type { FirmLink } from '$api/FirmLink';
import { ru } from '#lib/i18n/ru.js';
import { inTypstCode } from './insert.js';
import snippets from './typst-snippets.json';

/**
 * What a badge puts into the code: `markup` in markup, `code` (no `#`) inside `#{…}`, `if … {…}`;
 * `caret_back` — how far before the end of it the caret stays (inside the brackets of `[ ]`), 0 without it.
 */
export type CodeBadge = {
	id: string;
	label: string;
	markup: string;
	code: string;
	caret_back?: number;
};

/** The permanent badges, by `typst-snippets.json` (Rust renders each of them); the labels are in `ru.ts`. */
export function staticBadges(): CodeBadge[] {
	return snippets.badges.map((b) => ({ ...b, label: ru.templates.badges[b.id] ?? b.id }));
}

/** A «QR-код <имя>» badge of every enabled link of the firm, in the order of the list. */
export function linkBadges(links: readonly FirmLink[]): CodeBadge[] {
	return links
		.filter((l) => l.enabled)
		.map((l) => {
			const code = `qr-image("link.${l.id}", 20mm)`;
			return {
				id: `link.${l.id}`,
				label: ru.templates.badgeQr(l.name),
				markup: `#${code}`,
				code
			};
		});
}

/** The text of a badge after `before` (the code before the caret): without `#` in code mode. */
export function badgeText(badge: CodeBadge, before: string): string {
	return inTypstCode(before) ? badge.code : badge.markup;
}
