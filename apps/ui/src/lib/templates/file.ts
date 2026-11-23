import type { DocumentKind } from '$api/DocumentKind';
import type { Template } from '$api/Template';
import { ApiError } from '#lib/api/client.js';
import { importTemplateFile } from '#lib/api/document-templates.js';
import { templateErrors } from './editor.svelte.js';

/** What a `.mwtemplate` file the user picked came to. */
export type FileRead =
	| { ok: true; template: Template }
	| { ok: false; problem: 'not_template' | 'newer' }
	| { ok: false; problem: 'misfit'; messages: string[] };

/**
 * Checks the text of a picked file: it must be JSON, and the server must take it as a template of this
 * document. Mistakes of the file are the answer; anything else (no connection) is thrown.
 */
export async function readTemplateFile(
	kind: DocumentKind,
	text: string,
	send: (kind: DocumentKind, file: unknown) => Promise<Template> = importTemplateFile
): Promise<FileRead> {
	let parsed: unknown;
	try {
		parsed = JSON.parse(text);
	} catch {
		return { ok: false, problem: 'not_template' };
	}
	try {
		return { ok: true, template: await send(kind, parsed) };
	} catch (e) {
		if (!(e instanceof ApiError) || e.code !== 'validation') throw e;
		const fields = e.body?.fields ?? [];
		if (fields.some((f) => f.field === 'format')) return { ok: false, problem: 'not_template' };
		if (fields.some((f) => f.field === 'format_version')) return { ok: false, problem: 'newer' };
		return { ok: false, problem: 'misfit', messages: placedMessages(e, parsed) };
	}
}

/** The server's texts, each with the block or the page it is about, read off the picked file. */
function placedMessages(e: ApiError, parsed: unknown): string[] {
	const model = (parsed as { template?: { blocks?: unknown } } | null)?.template;
	const template = (Array.isArray(model?.blocks) ? model : { blocks: [] }) as Template;
	try {
		return templateErrors(e, template);
	} catch {
		return (e.body?.fields ?? []).map((f) => f.message || e.message);
	}
}
