import { render, screen, waitFor } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentPages } from '$api/DocumentPages';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import type { DocumentTarget } from '#lib/api/documents.js';
import { __resetHotkeysForTests } from '#lib/design/hotkeys.svelte.js';

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
afterEach(() => {
	vi.clearAllMocks();
	__resetHotkeysForTests();
});
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
		// The whole document: no pages chosen.
		expect(save).toHaveBeenCalledWith('issue_act', { orderId: 7 }, undefined);
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
		// Without a printer the preview is not opened again: printing says so instead.
		expect(print).toHaveBeenCalledWith('intake_act', { orderId: 42 }, { noPreview: true });
	});

	const ctrlP = (repeat = false) => {
		const e = new KeyboardEvent('keydown', {
			key: 'p',
			code: 'KeyP',
			ctrlKey: true,
			repeat,
			bubbles: true,
			cancelable: true
		});
		document.body.dispatchEvent(e);
		return e;
	};

	it('Ctrl/⌘P while it is open is its own «Печать»; in a browser the key is only taken', async () => {
		const print = vi.fn();
		const onclose = vi.fn();
		const view = render(DocumentPreview, {
			request: request(),
			load: async () => pages(1),
			save: vi.fn(),
			print,
			onclose
		});
		expect(ctrlP().defaultPrevented).toBe(true);
		expect(ctrlP(true).defaultPrevented).toBe(true);
		expect(onclose).toHaveBeenCalledOnce();
		expect(print).toHaveBeenCalledExactlyOnceWith(
			'intake_act',
			{ orderId: 42 },
			{ noPreview: true }
		);

		await view.rerender({ print: null });
		onclose.mockClear();
		// The browser's printing of the page does not open over the preview either.
		expect(ctrlP().defaultPrevented).toBe(true);
		expect(onclose).not.toHaveBeenCalled();

		await view.rerender({ request: null });
		expect(ctrlP().defaultPrevented).toBe(false);
	});

	it('a page that is not a picture is said; the pages made before it are freed', async () => {
		const unhandled = vi.fn();
		process.on('unhandledRejection', unhandled);
		try {
			render(DocumentPreview, {
				request: request(),
				load: async () => ({ ...pages(1), pages: ['iVBORw0KGgo=', '%%% не base64 %%%'] }),
				save: vi.fn(),
				print: null,
				onclose: () => {}
			});
			expect(await screen.findByText('Не удалось показать страницы документа')).toBeInTheDocument();
			expect(made).toHaveLength(1);
			expect(live()).toEqual([]);
			expect(screen.queryByRole('img')).toBeNull();
			await new Promise((r) => setTimeout(r, 0));
			expect(unhandled).not.toHaveBeenCalled();
		} finally {
			process.off('unhandledRejection', unhandled);
		}
	});

	/** Everything put on the page while `run` runs: the sources of pictures and the texts. */
	async function added(run: () => Promise<void>) {
		const seen: string[] = [];
		const watch = new MutationObserver((records) => {
			for (const r of records)
				for (const node of r.addedNodes) {
					if (!(node instanceof Element)) continue;
					seen.push(node.textContent ?? '');
					for (const img of [node, ...node.querySelectorAll('img')])
						if (img instanceof HTMLImageElement) seen.push(img.getAttribute('src') ?? '');
				}
		});
		watch.observe(document.body, { childList: true, subtree: true });
		await run();
		await new Promise((r) => setTimeout(r, 0));
		watch.disconnect();
		return seen;
	}

	it('opened again, it never shows the pages of before, freed already', async () => {
		const load = vi
			.fn<(k: DocumentKind, t: DocumentTarget, s: AbortSignal) => Promise<DocumentPages>>()
			.mockResolvedValueOnce(pages(1))
			.mockReturnValue(new Promise<DocumentPages>(() => {}));
		const view = render(DocumentPreview, {
			request: request(),
			load,
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		await screen.findByRole('img', { name: 'Страница 1 из 1' });
		await view.rerender({ request: null });
		expect(revoked).toEqual(['blob:page-1-image/png']);
		const seen = await added(async () => {
			await view.rerender({ request: request({ kind: 'issue_act', kinds: ['issue_act'] }) });
			await screen.findByLabelText('Документ готовится…');
		});
		expect(seen.filter((s) => s.startsWith('blob:'))).toEqual([]);
	});

	it('opened again, it never shows the failure of before', async () => {
		const load = vi
			.fn<(k: DocumentKind, t: DocumentTarget, s: AbortSignal) => Promise<DocumentPages>>()
			.mockRejectedValueOnce(new ApiError(0, 'network', 'Нет связи с сервером', null))
			.mockReturnValue(new Promise<DocumentPages>(() => {}));
		const view = render(DocumentPreview, {
			request: request(),
			load,
			save: vi.fn(),
			print: null,
			onclose: () => {}
		});
		await screen.findByText('Нет связи с сервером');
		await view.rerender({ request: null });
		const seen = await added(async () => {
			await view.rerender({ request: request() });
			await screen.findByLabelText('Документ готовится…');
		});
		expect(seen.filter((s) => s.includes('Нет связи'))).toEqual([]);
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

	describe('the pages to print', () => {
		const uncheck = async (...names: string[]) => {
			for (const name of names)
				await userEvent.click(await screen.findByRole('checkbox', { name }));
		};

		it('a document of two pages has a check box for each, both checked', async () => {
			render(DocumentPreview, {
				request: request(),
				load: async () => pages(2),
				save: vi.fn(),
				print: null,
				onclose: () => {}
			});
			expect(await screen.findByRole('checkbox', { name: 'Страница 1' })).toBeChecked();
			expect(screen.getByRole('checkbox', { name: 'Страница 2' })).toBeChecked();
			expect(screen.getAllByRole('checkbox')).toHaveLength(2);
		});

		it('«Печать» prints only the checked pages', async () => {
			const print = vi.fn();
			render(DocumentPreview, {
				request: request(),
				load: async () => pages(2),
				save: vi.fn(),
				print,
				onclose: vi.fn()
			});
			await uncheck('Страница 2');
			await userEvent.click(screen.getByRole('button', { name: 'Печать' }));
			expect(print).toHaveBeenCalledWith(
				'intake_act',
				{ orderId: 42 },
				{ noPreview: true, pages: '1' }
			);
		});

		it('«Сохранить PDF» saves only the checked pages', async () => {
			const save = vi.fn(async () => {});
			render(DocumentPreview, {
				request: request(),
				load: async () => pages(2),
				save,
				print: null,
				onclose: () => {}
			});
			await uncheck('Страница 2');
			await userEvent.click(screen.getByRole('button', { name: 'Сохранить PDF' }));
			expect(save).toHaveBeenCalledWith('intake_act', { orderId: 42 }, '1');
		});

		it('nothing checked: neither «Печать» nor «Сохранить PDF», and Ctrl/⌘P prints nothing', async () => {
			const print = vi.fn();
			const onclose = vi.fn();
			render(DocumentPreview, {
				request: request(),
				load: async () => pages(2),
				save: vi.fn(),
				print,
				onclose
			});
			await uncheck('Страница 1', 'Страница 2');
			expect(screen.getByRole('button', { name: 'Печать' })).toBeDisabled();
			expect(screen.getByRole('button', { name: 'Сохранить PDF' })).toBeDisabled();
			expect(ctrlP().defaultPrevented).toBe(true);
			expect(print).not.toHaveBeenCalled();
			expect(onclose).not.toHaveBeenCalled();
		});

		it('a document of one page has no check boxes', async () => {
			render(DocumentPreview, {
				request: request(),
				load: async () => pages(1),
				save: vi.fn(),
				print: null,
				onclose: () => {}
			});
			await screen.findByRole('img', { name: 'Страница 1 из 1' });
			expect(screen.queryByRole('checkbox')).toBeNull();
		});

		it('another document comes with every page checked', async () => {
			const save = vi.fn(async () => {});
			const view = render(DocumentPreview, {
				request: request(),
				load: async () => pages(2),
				save,
				print: null,
				onclose: () => {}
			});
			await uncheck('Страница 2');
			await view.rerender({ request: request({ kind: 'issue_act', kinds: ['issue_act'] }) });
			await screen.findByRole('img', { name: 'Страница 2 из 2' });
			expect(screen.getByRole('checkbox', { name: 'Страница 2' })).toBeChecked();
			await userEvent.click(screen.getByRole('button', { name: 'Сохранить PDF' }));
			expect(save).toHaveBeenCalledWith('issue_act', { orderId: 42 }, undefined);
		});
	});
});
