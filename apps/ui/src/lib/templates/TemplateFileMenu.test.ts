import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import TemplateFileMenu from './TemplateFileMenu.svelte';
import type { TemplateEditor } from './editor.svelte.js';

const exportFile = vi.hoisted(() => vi.fn());
vi.mock('#lib/api/document-templates.js', async (original) => ({
	...(await original<typeof import('#lib/api/document-templates.js')>()),
	exportTemplateFile: exportFile,
	legacyTemplate: async () => null
}));

afterEach(() => {
	toast.clear();
	exportFile.mockReset();
});

describe('TemplateFileMenu: «Сохранить в файл»', () => {
	it('when the edits cannot be saved first it says «Не сохранено» and offers to try again', async () => {
		const flush = vi.fn<() => Promise<boolean>>().mockResolvedValue(false);
		const editor = { kind: 'intake_act', flush } as unknown as TemplateEditor;
		render(TemplateFileMenu, { editor, onmisfit: () => {} });
		await userEvent.click(screen.getByRole('button', { name: 'Файл' }));
		await userEvent.click(await screen.findByText('Сохранить в файл…'));
		const notice = toast.items.find((t) => t.message === 'Не сохранено');
		expect(notice).toBeDefined();
		expect(notice!.action?.label).toBe('Повторить');
		expect(exportFile).not.toHaveBeenCalled();
		// «Повторить» saves the edits again and goes on with the file.
		flush.mockResolvedValue(true);
		exportFile.mockResolvedValue({ blob: new Blob(['{}']), filename: 'x.json' });
		await notice!.action!.run();
		expect(flush).toHaveBeenCalledTimes(2);
		expect(exportFile).toHaveBeenCalled();
	});
});
