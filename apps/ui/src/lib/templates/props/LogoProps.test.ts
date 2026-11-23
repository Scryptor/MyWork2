import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { Template } from '$api/Template';
import { createTemplateEditor, type TemplateEditorApi } from '../editor.svelte.js';
import LogoProps from './LogoProps.svelte';

const api = vi.hoisted(() => ({ getLogoInfo: vi.fn() }));
vi.mock('#lib/api/logo.js', () => api);

const template: Template = {
	kind: 'intake_act',
	page: {
		size: 'a4',
		landscape: false,
		margins_mm: { top: 10, right: 10, bottom: 10, left: 10 }
	},
	layout: 'one',
	font_size_pt: 9,
	blocks: [{ copies: 'both', body: { type: 'logo', width_mm: 40, height_mm: 15, align: 'left' } }]
} as unknown as Template;

function editor() {
	const initial: DocumentTemplate = {
		id: 1,
		kind: 'intake_act',
		template,
		standard: template,
		custom: false,
		version: 0,
		updated_at: null
	};
	const tapi: TemplateEditorApi = {
		getTemplate: async () => initial,
		saveTemplate: async (_k, t, v) => ({ ...initial, template: t, version: v + 1 })
	};
	return createTemplateEditor(initial, { api: tapi });
}

const logoBody = (e: ReturnType<typeof editor>) => {
	const b = e.template.blocks[0]!.body;
	if (b.type !== 'logo') throw new Error('not a logo');
	return b;
};

beforeEach(() => api.getLogoInfo.mockReset());

describe('LogoProps', () => {
	it('an edit of the width is an edit of the template', async () => {
		api.getLogoInfo.mockResolvedValue({ width: 4, height: 2, size: 70 });
		const e = editor();
		render(LogoProps, { editor: e, path: [0] });
		const field = screen.getByRole('textbox', { name: 'Ширина, мм' });
		await userEvent.clear(field);
		await userEvent.type(field, '55');
		expect(logoBody(e).width_mm).toBe(55);
		expect(logoBody(e).height_mm).toBe(15);
	});

	it('the height and the alignment are edited too', async () => {
		api.getLogoInfo.mockResolvedValue({ width: 4, height: 2, size: 70 });
		const e = editor();
		render(LogoProps, { editor: e, path: [0] });
		const field = screen.getByRole('textbox', { name: 'Высота, мм' });
		await userEvent.clear(field);
		await userEvent.type(field, '20');
		await userEvent.click(screen.getByRole('radio', { name: 'Справа' }));
		expect(logoBody(e).height_mm).toBe(20);
		expect(logoBody(e).align).toBe('right');
	});

	it('without a logo a hint with a link to «Документы и реквизиты» is shown', async () => {
		api.getLogoInfo.mockResolvedValue(null);
		render(LogoProps, { editor: editor(), path: [0] });
		expect(await screen.findByText(/Логотип не загружен — блок не печатается/)).toBeInTheDocument();
		expect(screen.getByRole('link', { name: 'Документы и реквизиты' })).toHaveAttribute(
			'href',
			expect.stringContaining('settings/documents')
		);
	});

	it('with a logo there is no hint', async () => {
		api.getLogoInfo.mockResolvedValue({ width: 4, height: 2, size: 70 });
		render(LogoProps, { editor: editor(), path: [0] });
		await waitFor(() => expect(api.getLogoInfo).toHaveBeenCalled());
		await Promise.resolve();
		expect(screen.queryByText(/не печатается/)).toBeNull();
	});
});
