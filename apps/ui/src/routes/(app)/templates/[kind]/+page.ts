import { error } from '@sveltejs/kit';
import type { DocumentKind } from '$api/DocumentKind';
import { getTemplate } from '#lib/api/document-templates.js';
import { failLoad } from '#lib/app/load-error.js';
import { ru } from '#lib/i18n/ru.js';
import type { PageLoad } from './$types';

/** The documents with a template to edit (`DocumentKind::editable`): all but the calibration label. */
const EDITABLE: readonly DocumentKind[] = [
	'intake_act',
	'issue_act',
	'warranty_intake_act',
	'warranty_issue_act',
	'blank_intake_act',
	'sticker'
];

// The template editor (plan 7.1): the template of the document in the address. Reading it needs only a
// session; the screen itself checks the right to change it.
export const load: PageLoad = async ({ params, parent }) => {
	const kind = EDITABLE.find((k) => k === params.kind);
	if (kind === undefined) error(404, ru.templates.notFound);
	// The session first: the program's layout signs in or sets up before anything is asked for.
	await parent();
	try {
		return { template: await getTemplate(kind) };
	} catch (e) {
		failLoad(e);
	}
};
