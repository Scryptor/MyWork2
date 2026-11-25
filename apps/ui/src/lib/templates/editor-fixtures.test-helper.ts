// An editor over a template for component tests: the server is a function that answers with what it got.
import type { Block } from '$api/Block';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import { isLabel } from '#lib/print/print.svelte.js';
import { createTemplateEditor, type TemplateEditorApi } from './editor.svelte.js';

export function templateOf(
	kind: DocumentKind,
	blocks: Block[],
	margins = { top: 3, right: 3, bottom: 3, left: 3 }
): Template {
	return {
		kind,
		page: { size: 'a4', landscape: false, margins_mm: margins },
		layout: isLabel(kind) ? 'one_per_page' : 'two_per_a4',
		font_size_pt: 9,
		blocks
	};
}

export function editorOf(template: Template, api?: Partial<TemplateEditorApi>) {
	const initial: DocumentTemplate = {
		id: 1,
		kind: template.kind,
		template,
		standard: template,
		custom: false,
		version: 0,
		updated_at: null,
		presets: []
	};
	return createTemplateEditor(initial, {
		api: {
			getTemplate: async () => initial,
			saveTemplate: async (_k, t, v) => ({ ...initial, template: t, version: v + 1 }),
			...api
		}
	});
}
