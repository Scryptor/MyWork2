import type { BlockCode } from '$api/BlockCode';
import type { BlockCodeRequest } from '$api/BlockCodeRequest';
import type { DocumentField } from '$api/DocumentField';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { DocumentTemplateInfo } from '$api/DocumentTemplateInfo';
import type { LegacyTemplate } from '$api/LegacyTemplate';
import type { Template } from '$api/Template';
import type { TemplatePreview } from '$api/TemplatePreview';
import type { TemplateSave } from '$api/TemplateSave';
import type { BlockPath } from '#lib/templates/ops.js';
import { ApiError, request, requestFile } from './client.js';

const base = (kind: DocumentKind) => `/document-templates/${kind}`;

export const listTemplates = (): Promise<DocumentTemplateInfo[]> =>
	request('GET', '/document-templates');

export const getTemplate = (kind: DocumentKind): Promise<DocumentTemplate> =>
	request('GET', base(kind));

/** Saves on top of the edit `version` the template is based on (a stale one is a 409). */
export const saveTemplate = (
	kind: DocumentKind,
	template: Template,
	version: number
): Promise<DocumentTemplate> => {
	const body: TemplateSave = { template, version };
	return request('PUT', base(kind), { body });
};

/** A draft rendered on the server as PNG pages; `signal` calls a superseded request off. */
export const previewTemplate = (
	kind: DocumentKind,
	body: TemplatePreview,
	signal?: AbortSignal
): Promise<DocumentPages> => request('POST', `${base(kind)}/preview`, { body, signal });

/** The Typst code that prints what the block at `path` prints; the template may be unsaved. */
export const blockCode = (
	kind: DocumentKind,
	template: Template,
	path: BlockPath
): Promise<BlockCode> => {
	const body: BlockCodeRequest = { template, path: [...path] };
	return request('POST', `${base(kind)}/block-code`, { body });
};

/** The template as a file to save, with the name the server gave it. */
export async function exportTemplateFile(
	kind: DocumentKind
): Promise<{ blob: Blob; filename: string }> {
	const file = await requestFile(`${base(kind)}/file`);
	return { blob: file.blob, filename: file.filename ?? `${kind}.mwtemplate` };
}

/** Checks a parsed file on the server and returns its template; nothing is saved. */
export const importTemplateFile = (kind: DocumentKind, file: unknown): Promise<Template> =>
	request('POST', `${base(kind)}/file`, { body: file });

export const documentFields = (): Promise<DocumentField[]> => request('GET', '/document-fields');

/** The text of the old MyWork 2 act of the kind, or null when the import left none (a 404). */
export async function legacyTemplate(kind: DocumentKind): Promise<LegacyTemplate | null> {
	try {
		return await request<LegacyTemplate>('GET', `${base(kind)}/legacy`);
	} catch (e) {
		if (e instanceof ApiError && e.status === 404) return null;
		throw e;
	}
}
