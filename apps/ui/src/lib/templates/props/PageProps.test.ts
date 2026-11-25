import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeAll, describe, expect, it } from 'vitest';
import { printSettings } from '#lib/print/settings.svelte.js';
import { editorOf, templateOf } from '../editor-fixtures.test-helper.js';
import PageProps from './PageProps.svelte';

// The server does not know the label of this computer (it is kept locally), so the page of a sticker
// warns when the margins leave less than the least content area (`MIN_CONTENT_MM` of the model).

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

const WARNING = /На наклейке этого рабочего места \(.*\) поля не оставляют места для содержимого/;
afterEach(() => printSettings.set({ labelMm: [58, 40] }));

describe('PageProps: the margins of a sticker', () => {
	it('warns when the margins leave less than 10 mm on the label of this computer', () => {
		printSettings.set({ labelMm: [58, 40] });
		const margins = { top: 15, right: 3, bottom: 20, left: 3 };
		render(PageProps, { editor: editorOf(templateOf('sticker', [], margins)) });
		expect(screen.getByText(/На наклейке этого рабочего места \(58 × 40 мм\)/)).toBeInTheDocument();
		expect(screen.getByText(WARNING)).toBeInTheDocument();
	});

	it('writes the size with a decimal comma', () => {
		printSettings.set({ labelMm: [57.5, 30] });
		const margins = { top: 3, right: 30, bottom: 3, left: 30 };
		render(PageProps, { editor: editorOf(templateOf('sticker', [], margins)) });
		expect(screen.getByText(/\(57,5 × 30 мм\)/)).toBeInTheDocument();
	});

	it('is silent when the content area is big enough, and exactly at the least', () => {
		printSettings.set({ labelMm: [58, 40] });
		const { unmount } = render(PageProps, {
			editor: editorOf(templateOf('sticker', [], { top: 3, right: 3, bottom: 3, left: 3 }))
		});
		expect(screen.queryByText(WARNING)).toBeNull();
		unmount();
		render(PageProps, {
			editor: editorOf(templateOf('sticker', [], { top: 15, right: 24, bottom: 15, left: 24 }))
		});
		expect(screen.queryByText(WARNING)).toBeNull();
	});

	it('is silent for a document that is not a sticker', () => {
		printSettings.set({ labelMm: [58, 40] });
		const margins = { top: 15, right: 3, bottom: 20, left: 3 };
		render(PageProps, { editor: editorOf(templateOf('intake_act', [], margins)) });
		expect(screen.queryByText(WARNING)).toBeNull();
	});
});

describe('PageProps: the layout «Два рядом на альбомном A4»', () => {
	const act = () => templateOf('intake_act', []);
	// jsdom does not place the list, so it stays hidden: opened from the keyboard, found with `hidden`.
	const hidden = { hidden: true } as const;
	async function openLayouts() {
		screen.getByRole('combobox', { name: 'Раскладка' }).focus();
		await userEvent.keyboard('{Enter}');
		await screen.findAllByRole('option', hidden);
	}

	it('offers four layouts, the new one named as the page prints', async () => {
		render(PageProps, { editor: editorOf(act()) });
		await openLayouts();
		expect(screen.getAllByRole('option', hidden)).toHaveLength(4);
		expect(screen.getAllByRole('option', hidden).map((o) => o.textContent?.trim())).toContain(
			'Два рядом на альбомном A4'
		);
	});

	it('picks A4 and landscape in one step', async () => {
		const editor = editorOf(act());
		render(PageProps, { editor });
		await openLayouts();
		const option = screen
			.getAllByRole('option', hidden)
			.find((o) => o.textContent?.trim() === 'Два рядом на альбомном A4')!;
		await userEvent.click(option);
		expect(editor.template.layout).toBe('two_across');
		expect(editor.template.page.size).toBe('a4');
		expect(editor.template.page.landscape).toBe(true);
		editor.undo();
		expect(editor.template.layout).toBe('two_per_a4');
		expect(editor.template.page.landscape).toBe(false);
	});

	it('leaves no size and no orientation to choose while it is on', () => {
		const t = act();
		const editor = editorOf({
			...t,
			layout: 'two_across',
			page: { ...t.page, landscape: true }
		});
		render(PageProps, { editor });
		expect(screen.queryByRole('radio', { name: 'A5' })).toBeNull();
		expect(screen.queryByRole('switch', { name: 'Альбомная' })).toBeNull();
	});
});
