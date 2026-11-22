import type { AppSettings } from '$api/AppSettings';
import type { Order } from '$api/Order';
import type { ServerInfo } from '$api/ServerInfo';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { app } from '#lib/app/context.svelte.js';
import { order, POINTS, STATUSES, user } from '#lib/app/fixtures.test-helper.js';
import { __resetHotkeysForTests } from '#lib/design/hotkeys.svelte.js';
import { DEFAULT_PRINT_SETTINGS, printSettings } from '#lib/print/settings.svelte.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { closeEditor, OrderActions, unsavedText } from './actions.svelte.js';
import type { Conflict, OrderEditor, OrderField } from './editor.svelte.js';

// Printing itself is `print.svelte.ts`'s (tested there): here only what the order asks of it.
const print = vi.hoisted(() => ({ printDocument: vi.fn(async () => {}), openPreview: vi.fn() }));
vi.mock('#lib/print/print.svelte.js', async (original) => ({
	...(await original<typeof import('#lib/print/print.svelte.js')>()),
	...print
}));

/** An editor whose flush ends as given: saved, failed with an error, or with a disputed field. */
function editor(o: Order, end: 'saved' | 'error' | 'conflict') {
	const conflicts = new Map<OrderField, Conflict>(
		end === 'conflict' ? [['work_done', { mine: 'Моё', theirs: 'Чужое' }]] : []
	);
	const error = end === 'error' ? new ApiError(0, 'network', 'Нет связи с сервером', null) : null;
	return {
		order: o,
		conflicts,
		autosave: { error },
		flush: vi.fn(async () => end === 'saved'),
		dispose: vi.fn()
	} as unknown as OrderEditor & { dispose: ReturnType<typeof vi.fn> };
}

const settle = async () => {
	for (let i = 0; i < 10; i++) await Promise.resolve();
};

afterEach(() => {
	for (const t of toast.items) toast.dismiss(t.id);
});

describe('closeEditor', () => {
	it('saves what is left and stops quietly', async () => {
		const ed = editor(order(5), 'saved');
		closeEditor(ed);
		await settle();
		expect(ed.flush).toHaveBeenCalled();
		expect(ed.dispose).toHaveBeenCalled();
		expect(toast.items).toEqual([]);
	});

	it('a failed save is told', async () => {
		closeEditor(editor(order(5), 'error'));
		await settle();
		expect(toast.items.map((t) => t.message)).toEqual([
			'Изменения № 5 не сохранены: Нет связи с сервером'
		]);
	});

	it('a conflict left undecided is told too: the disputed values are not saved', async () => {
		const ed = editor(order(5), 'conflict');
		closeEditor(ed);
		await settle();
		expect(toast.items.map((t) => [t.message, t.tone])).toEqual([
			['Изменения № 5 не сохранены: Заказ изменили на другом компьютере', 'danger']
		]);
		expect(unsavedText(ed)).toBe('Изменения № 5 не сохранены: Заказ изменили на другом компьютере');
		expect(ed.dispose).toHaveBeenCalled();
	});
});

describe('printing an order', () => {
	const ISSUED = 4;
	beforeEach(() => {
		app.set({
			server: {} as ServerInfo,
			me: user({ permissions: ['orders_edit'] }),
			statuses: STATUSES,
			points: POINTS,
			settings: { issue_required: [], default_warranty: '' } as unknown as AppSettings
		});
	});
	afterEach(() => {
		vi.clearAllMocks();
		app.reset();
		printSettings.set(DEFAULT_PRINT_SETTINGS);
		localStorage.clear();
		__resetHotkeysForTests();
	});

	/** An editor of `o` whose saves are done; `run` answers with `after` (the issued order). */
	function shown(o: Order, after: Order = o) {
		const flush = vi.fn(async () => true);
		const ed = {
			order: o,
			conflicts: new Map(),
			autosave: { error: null, flush: vi.fn(async () => {}) },
			flush,
			run: vi.fn(async () => after),
			dispose: vi.fn()
		} as unknown as OrderEditor;
		return { actions: new OrderActions(() => ed), flush };
	}
	const ctrlP = () =>
		document.body.dispatchEvent(
			new KeyboardEvent('keydown', {
				key: 'p',
				code: 'KeyP',
				ctrlKey: true,
				bubbles: true,
				cancelable: true
			})
		);

	it('Ctrl/⌘P prints the intake act of an order in work and the issue act of an issued one', async () => {
		const inWork = shown(order(7, { status_id: 2 }));
		const stop = inWork.actions.printShortcut();
		ctrlP();
		await settle();
		expect(print.printDocument).toHaveBeenLastCalledWith('intake_act', { orderId: 7 });
		stop?.();

		shown(
			order(8, { status_id: ISSUED, issued_at: '2026-11-20T12:00:00' })
		).actions.printShortcut();
		ctrlP();
		await settle();
		expect(print.printDocument).toHaveBeenLastCalledWith('issue_act', { orderId: 8 });
	});

	it('what is typed is saved before the server makes the document', async () => {
		const { actions, flush } = shown(order(7));
		let savedFirst = false;
		print.printDocument.mockImplementationOnce(async () => {
			savedFirst = flush.mock.calls.length === 1;
		});
		await actions.print('sticker');
		expect(savedFirst).toBe(true);
		expect(print.printDocument).toHaveBeenCalledWith('sticker', { orderId: 7 });
	});

	it('what is typed is not saved: nothing is printed and the notice says why', async () => {
		const ed = editor(order(7), 'error');
		const actions = new OrderActions(() => ed);
		await actions.print('sticker');
		expect(print.printDocument).not.toHaveBeenCalled();
		expect(toast.items.map((t) => [t.message, t.tone])).toEqual([
			[
				'Не напечатано: изменения № 7 не сохранены (Нет связи с сервером). Документ напечатался бы без них',
				'danger'
			]
		]);
	});

	it('the preview still opens over an unsaved order, with a warning that it shows what is saved', async () => {
		const ed = editor(order(7), 'conflict');
		const actions = new OrderActions(() => ed);
		await actions.preview('intake_act', ['intake_act', 'sticker']);
		expect(print.openPreview).toHaveBeenCalledWith('intake_act', { orderId: 7 }, [
			'intake_act',
			'sticker'
		]);
		expect(toast.items.map((t) => [t.message, t.tone])).toEqual([
			[
				'В предпросмотре — сохранённый заказ: изменения № 7 не сохранены (Заказ изменили на другом компьютере)',
				'neutral'
			]
		]);
	});

	it('the issue prints the issue act after it is saved, when «Печатать акт выдачи» is on', async () => {
		const o = order(7, { status_id: 3 });
		const issued = { ...o, status_id: ISSUED, issued_at: '2026-11-20T12:00:00', version: 2 };
		const on = shown(o, issued);
		on.actions.issueOpen = true;
		await on.actions.issue({
			work_done: '',
			master_user_id: null,
			final_price: null,
			discount: 0,
			warranty: ''
		});
		await settle();
		expect(on.actions.issueOpen).toBe(false);
		// The first issue: the ordinary act.
		expect(print.printDocument).toHaveBeenCalledExactlyOnceWith('issue_act', { orderId: 7 });

		vi.clearAllMocks();
		printSettings.set({ onIssue: { act: false } });
		await shown(o, issued).actions.issue({
			work_done: '',
			master_user_id: null,
			final_price: null,
			discount: 0,
			warranty: ''
		});
		await settle();
		expect(print.printDocument).not.toHaveBeenCalled();
	});

	const issueValues = {
		work_done: '',
		master_user_id: null,
		final_price: null,
		discount: 0,
		warranty: ''
	};
	/** The statuses of the program with «Принят по гарантии» (id 6), written as `name`. */
	const withWarranty = (name: string) =>
		app.set({
			server: {} as ServerInfo,
			me: user({ permissions: ['orders_edit'] }),
			statuses: [
				...STATUSES,
				{ id: 6, name, kind: 'open', color: 'teal', sort_order: 6, archived: false }
			],
			points: POINTS,
			settings: { issue_required: [], default_warranty: '' } as unknown as AppSettings
		});

	it('issued from «Принят по гарантии»: the issue act after warranty', async () => {
		withWarranty('Принят по гарантии');
		// The warranty intake kept the first issue date.
		const o = order(8, { status_id: 6, issued_at: '2026-11-01T10:00:00' });
		await shown(o, { ...o, status_id: ISSUED, version: 2 }).actions.issue(issueValues);
		await settle();
		expect(print.printDocument).toHaveBeenCalledExactlyOnceWith('warranty_issue_act', {
			orderId: 8
		});
	});

	it('the warranty status written in another case is found as the server finds it', async () => {
		withWarranty('ПРИНЯТ  по гарантии');
		const o = order(8, { status_id: 6, issued_at: '2026-11-01T10:00:00' });
		await shown(o, { ...o, status_id: ISSUED, version: 2 }).actions.issue(issueValues);
		await settle();
		expect(print.printDocument).toHaveBeenCalledExactlyOnceWith('warranty_issue_act', {
			orderId: 8
		});
	});

	it('issued again after «Вернуть в работу»: the ordinary issue act, though it has an issue date', async () => {
		withWarranty('Принят по гарантии');
		const o = order(8, { status_id: 3, issued_at: '2026-11-01T10:00:00' });
		await shown(o, { ...o, status_id: ISSUED, version: 2 }).actions.issue(issueValues);
		await settle();
		expect(print.printDocument).toHaveBeenCalledExactlyOnceWith('issue_act', { orderId: 8 });
	});

	it('the warranty case prints its intake act when «Печатать акт приёма по гарантии» is on', async () => {
		const o = order(9, { status_id: ISSUED, issued_at: '2026-11-20T12:00:00' });
		const on = shown(o);
		await on.actions.acceptWarranty({ complaint: 'Снова не включается' });
		await settle();
		expect(print.printDocument).toHaveBeenCalledWith('warranty_intake_act', { orderId: 9 });

		vi.clearAllMocks();
		printSettings.set({ onIntake: { act: false } });
		await shown(o).actions.acceptWarranty({ complaint: 'Снова не включается' });
		await settle();
		expect(print.printDocument).not.toHaveBeenCalled();
	});
});
