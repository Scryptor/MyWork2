import { render, screen } from '@testing-library/svelte';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Block } from '$api/Block';
import type { CodeDiagnostic } from '$api/CodeDiagnostic';
import type { DocumentPages } from '$api/DocumentPages';
import { ApiError } from '#lib/api/client.js';
import { editorOf, templateOf } from './editor-fixtures.test-helper.js';
import TemplatePreview from './TemplatePreview.svelte';

const previewTemplate = vi.hoisted(() => vi.fn());
vi.mock('#lib/api/document-templates.js', async (original) => ({
	...(await original<typeof import('#lib/api/document-templates.js')>()),
	previewTemplate
}));

afterEach(() => {
	vi.useRealTimers();
	previewTemplate.mockReset();
});

const code = (s: string): Block => ({ copies: 'both', body: { type: 'typst_code', code: s } });
const PAGES: DocumentPages = { dpi: 96, width_mm: 210, height_mm: 297, pages: [] };

const codeError = (diagnostics: CodeDiagnostic[]) =>
	new ApiError(422, 'validation', 'Ошибка в своём Typst-коде', {
		code: 'validation',
		message: 'Ошибка в своём Typst-коде',
		fields: [{ field: 'blocks.0.code', code: 'typst_error', message: 'Ошибка в своём Typst-коде' }],
		current_version: null,
		permission: null,
		retry_after_secs: null,
		diagnostics
	});

describe('TemplatePreview: the errors of own code', () => {
	it('a code error fills the diagnostics of the editor; a preview drawn or failed otherwise empties them', async () => {
		vi.useFakeTimers();
		const first = templateOf('intake_act', [code('#v("order.number")\n#foo')]);
		const editor = editorOf(first);
		expect(editor.diagnostics).toEqual([]);
		const diagnostics: CodeDiagnostic[] = [
			{ place: { block: [0], line: 2, column: 2 }, message: 'unknown variable: foo', hints: [] }
		];
		previewTemplate.mockRejectedValueOnce(codeError(diagnostics));
		const { rerender } = render(TemplatePreview, {
			kind: 'intake_act',
			template: first,
			orderId: null,
			ondiagnostics: (d: readonly CodeDiagnostic[]) => (editor.diagnostics = d)
		});
		await vi.advanceTimersByTimeAsync(400);
		expect(editor.diagnostics).toEqual(diagnostics);
		// Over the preview: the block, the line and the column, once.
		expect(
			screen.getByText('Typst-код · #v("order.number") · строка 2, знак 2: unknown variable: foo')
		).toBeInTheDocument();

		// The code fixed: the preview is drawn, no error is left to mark.
		previewTemplate.mockResolvedValueOnce(PAGES);
		await rerender({ template: templateOf('intake_act', [code('#v("order.number")')]) });
		await vi.advanceTimersByTimeAsync(400);
		expect(editor.diagnostics).toEqual([]);

		// Another error, then no connection: the marks do not stay on code they may no longer fit.
		previewTemplate.mockRejectedValueOnce(codeError(diagnostics));
		await rerender({ template: templateOf('intake_act', [code('#foo\n#foo')]) });
		await vi.advanceTimersByTimeAsync(400);
		expect(editor.diagnostics).toEqual(diagnostics);
		previewTemplate.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи', null));
		await rerender({ template: templateOf('intake_act', [code('#bar')]) });
		await vi.advanceTimersByTimeAsync(400);
		expect(editor.diagnostics).toEqual([]);
	});
});
