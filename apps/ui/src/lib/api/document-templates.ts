import type { DocumentField } from '$api/DocumentField';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { DocumentTemplateInfo } from '$api/DocumentTemplateInfo';
import type { Template } from '$api/Template';
import type { TemplatePreview } from '$api/TemplatePreview';
import type { TemplateSave } from '$api/TemplateSave';
import { request, requestFile } from './client.js';

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
