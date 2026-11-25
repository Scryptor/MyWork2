import { render, screen } from '@testing-library/svelte';
import { describe, expect, it } from 'vitest';
import type { Block } from '$api/Block';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import { createTemplateEditor, type TemplateEditorApi } from '../editor.svelte.js';
import CopiesField from './CopiesField.svelte';

// «Экземпляры» of a block hide in a one-copy layout, unless the block is already for one copy only:
// then the field stays, so the choice can be undone.

const block = (copies: Block['copies']): Block => ({
	copies,
	body: { type: 'spacer', mm: 2 }
});

function editorOf(layout: Template['layout'], copies: Block['copies']) {
	const template: Template = {
		kind: 'warranty_intake_act',
		page: {
			size: 'a4',
			landscape: false,
			margins_mm: { top: 10, right: 10, bottom: 10, left: 10 }
		},
		layout,
		font_size_pt: 9,
		blocks: [block(copies)]
	};
	const initial: DocumentTemplate = {
		id: 3,
		kind: 'warranty_intake_act',
		template,
		standard: template,
		custom: false,
		version: 0,
		updated_at: null,
		presets: []
	};
	const api: TemplateEditorApi = {
		getTemplate: async () => initial,
		saveTemplate: async (_kind, t, version) => ({ ...initial, template: t, version: version + 1 })
	};
	return createTemplateEditor(initial, { api });
}

describe('CopiesField', () => {
	it('is shown in a two-copy layout', () => {
		render(CopiesField, { editor: editorOf('two_per_a4', 'both'), path: [0] });
		expect(screen.getByText('Экземпляры')).toBeInTheDocument();
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});

	it('is hidden in the one-copy layout while the block is in both copies', () => {
		render(CopiesField, { editor: editorOf('one_per_page', 'both'), path: [0] });
		expect(screen.queryByText('Экземпляры')).toBeNull();
	});

	it('stays in the one-copy layout for a block of one copy, with a hint that it is not printed', () => {
		render(CopiesField, { editor: editorOf('one_per_page', 'service'), path: [0] });
		expect(screen.getByText('Экземпляры')).toBeInTheDocument();
		expect(screen.getByText(/не печатается/)).toBeInTheDocument();
	});
});
