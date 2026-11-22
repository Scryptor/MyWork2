import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import type { DocumentTarget } from '#lib/api/documents.js';

// The preview: the pages of a document as pictures from `blob:` addresses, «Сохранить PDF», and «Печать»
// in the desktop program. The addresses go when the preview closes or shows something else; a request
// still on its way then is called off. The server is replaced by `load`.
vi.mock('$app/navigation', () => ({ afterNavigate: () => {} }));
vi.mock('$app/state', () => ({ navigating: { to: null } }));

import DocumentPreview from './DocumentPreview.svelte';
import type { PreviewRequest } from './print.svelte.js';

beforeAll(() => {
	Element.prototype.hasPointerCapture ??= () => false;
	Element.prototype.releasePointerCapture ??= () => {};
	Element.prototype.scrollIntoView ??= () => {};
});

/** Blob addresses as jsdom has none: numbered, the live ones counted. */
let made: string[] = [];
let revoked: string[] = [];
beforeEach(() => {
	made = [];
	revoked = [];
	URL.createObjectURL = (blob: Blob) => {
		const url = `blob:page-${made.length + 1}-${blob.type}`;
		made.push(url);
		return url;
	};
	URL.revokeObjectURL = (url: string) => void revoked.push(url);
});
afterEach(() => vi.clearAllMocks());
const live = () => made.filter((u) => !revoked.includes(u));

const pages = (n: number, width_mm = 210, height_mm = 297): DocumentPages => ({
	dpi: 110,
	width_mm,
	height_mm,
	pages: Array.from({ length: n }, () => 'iVBORw0KGgo=')
});

/** A request that answers when told to, with the signal it was given. */
function later() {
	const calls: { kind: DocumentKind; signal: AbortSignal; answer: (p: DocumentPages) => void }[] =
		[];
	const load = vi.fn(
		(kind: DocumentKind, _target: DocumentTarget, signal: AbortSignal) =>
			new Promise<DocumentPages>((answer) => calls.push({ kind, signal, answer }))
	);
	return { load, calls };
}

const request = (over: Partial<PreviewRequest> = {}): PreviewRequest => ({
	kind: 'intake_act',
	target: { orderId: 42 },
	kinds: ['intake_act'],
	...over
});

describe('the preview', () => {
	it('shows the pages of the document as pictures of blob addresses', async () => {
		render(DocumentPreview, {
			request: request(),
			load: async () => pages(2),
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		expect(screen.getByRole('dialog', { name: 'Акт приёма · № 42' })).toBeInTheDocument();
		const first = await screen.findByRole('img', { name: 'Страница 1 из 2' });
		expect(first).toHaveAttribute('src', 'blob:page-1-image/png');
		expect(screen.getByRole('img', { name: 'Страница 2 из 2' })).toHaveAttribute(
			'src',
			'blob:page-2-image/png'
		);
	});

	it('frees every address when it closes', async () => {
		const view = render(DocumentPreview, {
			request: request(),
			load: async () => pages(2),
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		await screen.findByRole('img', { name: 'Страница 2 из 2' });
		expect(live()).toHaveLength(2);
		await view.rerender({ request: null });
		expect(live()).toEqual([]);
	});

	it('another document frees the pages of the one before and asks for its own', async () => {
		const { load, calls } = later();
		render(DocumentPreview, {
			request: request({ kinds: ['intake_act', 'sticker'] }),
			load,
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		await waitFor(() => expect(calls).toHaveLength(1));
		calls[0]!.answer(pages(1));
		await screen.findByRole('img', { name: 'Страница 1 из 1' });

		const box = screen.getByRole('combobox', { name: 'Документ' });
		box.focus();
		await userEvent.keyboard('{Enter}');
		const sticker = (await screen.findAllByRole('option', { hidden: true })).find(
			(o) => o.textContent?.trim() === 'Наклейка'
		)!;
		await userEvent.click(sticker);
		await waitFor(() => expect(calls).toHaveLength(2));
		expect(calls[1]!.kind).toBe('sticker');
		expect(live()).toEqual([]);
		expect(screen.queryByRole('img')).toBeNull();
		calls[1]!.answer(pages(1, 58, 40));
		await screen.findByRole('img', { name: 'Страница 1 из 1' });
		expect(live()).toHaveLength(1);
	});

	it('closed before the answer: the request is called off and its late answer makes no address', async () => {
		const { load, calls } = later();
		const view = render(DocumentPreview, {
			request: request(),
			load,
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		await waitFor(() => expect(calls).toHaveLength(1));
		await view.rerender({ request: null });
		expect(calls[0]!.signal.aborted).toBe(true);
		calls[0]!.answer(pages(1));
		await Promise.resolve();
		expect(made).toEqual([]);
	});

	it('a failure is said with «Повторить», which asks again', async () => {
		const load = vi
			.fn<(k: DocumentKind, t: DocumentTarget, s: AbortSignal) => Promise<DocumentPages>>()
			.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи с сервером', null))
			.mockResolvedValueOnce(pages(1));
		render(DocumentPreview, {
			request: request(),
			load,
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		expect(await screen.findByText('Нет связи с сервером')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Повторить' }));
		await screen.findByRole('img', { name: 'Страница 1 из 1' });
	});

	it('«Сохранить PDF» saves the document shown', async () => {
		const save = vi.fn(async () => {});
		render(DocumentPreview, {
			request: request({ kind: 'issue_act', kinds: ['issue_act'], target: { orderId: 7 } }),
			load: async () => pages(1),
			save,
			print: null,
			onclose: () => {}
		});
		await userEvent.click(screen.getByRole('button', { name: 'Сохранить PDF' }));
		expect(save).toHaveBeenCalledWith('issue_act', { orderId: 7 });
	});

	it('«Печать» is only in the program: it closes the preview and prints', async () => {
		const print = vi.fn();
		const onclose = vi.fn();
		const view = render(DocumentPreview, {
			request: request(),
			load: async () => pages(1),
			save: vi.fn(),
			print: null,
			onclose
		});
		expect(screen.queryByRole('button', { name: 'Печать' })).toBeNull();
		await view.rerender({ print });
		await userEvent.click(screen.getByRole('button', { name: 'Печать' }));
		expect(onclose).toHaveBeenCalled();
		expect(print).toHaveBeenCalledWith('intake_act', { orderId: 42 });
	});

	it('names a sample and a document without an order', () => {
		const view = render(DocumentPreview, {
			request: request({ target: {} }),
			load: () => new Promise<DocumentPages>(() => {}),
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		expect(screen.getByRole('dialog', { name: 'Образец · Акт приёма' })).toBeInTheDocument();
		view.unmount();
		render(DocumentPreview, {
			request: request({ kind: 'blank_intake_act', kinds: ['blank_intake_act'], target: {} }),
			load: () => new Promise<DocumentPages>(() => {}),
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		expect(screen.getByRole('dialog', { name: 'Пустой акт приёма' })).toBeInTheDocument();
	});
});
