// What the panel and the full card do with their open order (spec 6.2–6.4): the status with «Отменить»,
// «Выдать», «Вернуть в работу», «Принять по гарантии». Every write goes through the order's editor.
import { tick } from 'svelte';
import type { NewWarrantyCase } from '$api/NewWarrantyCase';
import type { OrderPatch } from '$api/OrderPatch';
import type { Status } from '$api/Status';
import { errorMessage } from '#lib/api/client.js';
import { acceptWarrantyCase, changeStatus, issueOrder, updateOrder } from '#lib/api/orders.js';
import { app } from '#lib/app/context.svelte.js';
import { undo } from '#lib/app/undo.svelte.js';
import { registerHotkey } from '#lib/design/hotkeys.svelte.js';
import { ru } from '#lib/i18n/ru.js';
import type { MenuItem } from '#lib/ui/DropdownMenu.svelte';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { revertIssue, revertStatus, type OrderEditor } from './editor.svelte.js';
import type { IssueValues } from './IssueDialog.svelte';

const t = ru.order;

/** Why the order's own changes are not saved: the failed save, or a conflict waiting for a choice. */
export function unsavedText(ed: OrderEditor): string {
	const reason =
		ed.conflicts.size > 0 ? t.conflictTitle : (ed.autosave.error?.message ?? ru.api.failed);
	return t.unsaved(ed.order.id, reason);
}

/**
 * An editor that is no longer shown (another order, the screen left): what is left is saved; a failure
 * or a conflict left undecided (its values are not saved) is told; then it stops.
 */
export function closeEditor(ed: OrderEditor): void {
	void ed
		.flush()
		.then((ok) => {
			if (!ok) toast.show({ message: unsavedText(ed), tone: 'danger' });
		})
		.finally(() => ed.dispose());
}

export class OrderActions {
	readonly #editor: () => OrderEditor | null;
	/** A status picked a moment ago, shown until the server answers. */
	#picked = $state.raw<{ ed: OrderEditor; id: number } | null>(null);
	issueOpen = $state(false);
	/** The issued status picked in the status control; null: the first issued one (`issueOrder`). */
	issueTo = $state<number | null>(null);
	returnOpen = $state(false);
	warrantyOpen = $state(false);

	/** `editor`: the editor of the order shown now, read at every action and every «Отменить». */
	constructor(editor: () => OrderEditor | null) {
		this.#editor = editor;
	}

	get editor(): OrderEditor | null {
		return this.#editor();
	}
	get status(): Status | undefined {
		const o = this.editor?.order;
		return o ? app.statusById(o.status_id) : undefined;
	}
	get issued(): boolean {
		return this.status?.kind === 'issued';
	}
	get canEdit(): boolean {
		return app.can('orders_edit');
	}
	/** Statuses of the status control: not archived, and the order's own. */
	get pickable(): Status[] {
		const own = this.editor?.order.status_id;
		return app.statuses.filter((s) => !s.archived || s.id === own);
	}
	/** Statuses «Вернуть в работу» offers: not issued, not archived. */
	get backToWork(): Status[] {
		return app.statuses.filter((s) => !s.archived && s.kind !== 'issued');
	}
	/** The status the control shows: the one just picked, or the order's. */
	get statusValue(): number {
		const p = this.#picked;
		const ed = this.editor;
		return p && p.ed === ed ? p.id : (ed?.order.status_id ?? 0);
	}

	/** Items of ⋯ that change the order: an issued one goes back to work or comes under warranty. */
	get menuItems(): MenuItem[] {
		if (!this.issued) return [];
		return [
			{ label: t.returnToWork, disabled: !this.canEdit, onselect: () => (this.returnOpen = true) },
			{ label: t.warrantyCase, disabled: !this.canEdit, onselect: () => (this.warrantyOpen = true) }
		];
	}

	async pickStatus(id: number, previous: number): Promise<void> {
		const ed = this.editor;
		const s = app.statusById(id);
		if (!ed || !s) return;
		// An issued status is not set here: «Выдать» checks what is required first (spec 6.4).
		if (s.kind === 'issued') {
			this.openIssue(id);
			return;
		}
		this.#picked = { ed, id };
		try {
			const o = await ed.run((v) => changeStatus(ed.order.id, v, { status_id: id }));
			undo.push(t.statusChanged(o.id, s.name), () => this.#undoStatus(o.id, previous));
		} catch (e) {
			toast.show({ message: t.statusFailed(errorMessage(e)), tone: 'danger' });
		} finally {
			// A later pick still waiting keeps showing.
			if (this.#picked?.ed === ed && this.#picked.id === id) this.#picked = null;
		}
	}

	/**
	 * «Отменить»: through the editor when it still shows the order, else on the newest version. Back to an
	 * issued status (after «Вернуть в работу») no new issue date is stamped.
	 */
	async #undoStatus(id: number, statusId: number): Promise<void> {
		const ed = this.editor;
		if (ed && ed.order.id === id) {
			await ed.run((v) => changeStatus(id, v, { status_id: statusId, stamp_issued_at: false }));
		} else {
			await revertStatus(id, statusId);
		}
	}

	openIssue(statusId: number | null = null): void {
		if (!this.editor || this.issued || !this.canEdit) return;
		this.issueTo = statusId;
		this.issueOpen = true;
	}

	/** «Выдать» of the dialog: what was typed goes first, then what the dialog changed, then the issue. */
	async issue(values: IssueValues): Promise<void> {
		const ed = this.editor;
		if (!ed) return;
		const id = ed.order.id;
		const before = { status: ed.order.status_id, issuedAt: ed.order.issued_at };
		await ed.autosave.flush().catch(() => {});
		const patch: OrderPatch = {};
		if (values.work_done !== ed.order.work_done) patch.work_done = values.work_done;
		if (values.master !== ed.order.master) patch.master = values.master;
		if (values.final_price !== ed.order.final_price) patch.final_price = values.final_price;
		if (values.discount !== ed.order.discount) patch.discount = values.discount;
		if (values.warranty !== ed.order.warranty) patch.warranty = values.warranty;
		if (Object.keys(patch).length > 0) await ed.run((v) => updateOrder(id, v, patch));
		const target = this.issueTo;
		const o = await ed.run((v) =>
			target === null ? issueOrder(id, v) : changeStatus(id, v, { status_id: target })
		);
		this.issueOpen = false;
		// Toasts wait for the dialog to go.
		await tick();
		const clearIssuedAt = before.issuedAt === null && app.can('order_dates_edit');
		undo.push(t.statusChanged(id, app.statusById(o.status_id)?.name ?? ''), () =>
			this.#undoIssue(id, before.status, clearIssuedAt)
		);
	}

	async #undoIssue(id: number, statusId: number, clearIssuedAt: boolean): Promise<void> {
		const ed = this.editor;
		if (ed && ed.order.id === id) {
			const o = await ed.run((v) => changeStatus(id, v, { status_id: statusId }));
			if (clearIssuedAt && o.issued_at !== null) {
				await ed.run((v) => updateOrder(id, v, { issued_at: null }));
			}
		} else {
			await revertIssue(id, { statusId, clearIssuedAt });
		}
	}

	async returnToWork(statusId: number): Promise<void> {
		const ed = this.editor;
		if (!ed) return;
		const id = ed.order.id;
		const issuedStatus = ed.order.status_id;
		await ed.run((v) => changeStatus(id, v, { status_id: statusId }));
		this.returnOpen = false;
		await tick();
		undo.push(t.statusChanged(id, app.statusById(statusId)?.name ?? ''), () =>
			this.#undoStatus(id, issuedStatus)
		);
	}

	/** «Принять» of the warranty case: the status «Принят по гарантии» and a case in the history. */
	async acceptWarranty(c: NewWarrantyCase): Promise<void> {
		const ed = this.editor;
		if (!ed) return;
		const id = ed.order.id;
		await ed.run((v) => acceptWarrantyCase(id, v, c));
		this.warrantyOpen = false;
		await tick();
		toast.show({ message: t.warrantyAccepted(id), tone: 'success' });
	}

	/**
	 * «Выдать» on Ctrl/⌘+Enter, fields included, not over a modal (it opens one). Call it in an effect:
	 * it registers while the order can be issued and `when` allows.
	 */
	issueShortcut(when: () => boolean = () => true): (() => void) | undefined {
		if (!this.editor || this.issued || !this.canEdit) return;
		return registerHotkey(
			'Mod+Enter',
			() => {
				if (when()) this.openIssue();
			},
			{ allowInFields: true }
		);
	}
}
