import { untrack } from 'svelte';
import { onSessionReset } from '#lib/app/session-reset.js';
import { ru } from '#lib/i18n/ru.js';

export interface ToastOptions {
	message: string;
	tone?: 'neutral' | 'success' | 'danger';
	action?: { label: string; run: () => void | Promise<void> };
	/** Milliseconds on screen, 6000 by default. */
	duration?: number;
	/** Called once when the toast is gone, however it went (timer, button, pushed out, cleared). */
	onclose?: () => void;
}

export type ToastItem = ToastOptions & { id: string };

const MAX_VISIBLE = 3;
const DEFAULT_DURATION = 6000;

interface Meta {
	/** Made by `undoable`: the only kind Ctrl/⌘+Z may run. */
	undo: boolean;
	/** Milliseconds left on screen; null when the toast has no timer. */
	left: number | null;
	startedAt: number;
	handle: ReturnType<typeof setTimeout> | undefined;
	onclose: (() => void) | undefined;
}

let items = $state.raw<ToastItem[]>([]);
// eslint-disable-next-line svelte/prefer-svelte-reactivity -- timer bookkeeping, nothing renders it
const meta = new Map<string, Meta>();
let paused = false;
let seq = 0;

function arm(id: string, m: Meta): void {
	if (m.left === null) return;
	m.startedAt = Date.now();
	m.handle = setTimeout(() => toast.dismiss(id), m.left);
}

function forget(id: string): void {
	const m = meta.get(id);
	if (m?.handle !== undefined) clearTimeout(m.handle);
	meta.delete(id);
	m?.onclose?.();
}

/**
 * Notifications at the bottom of the screen: at most three, each for 6 s unless paused.
 * The methods run untracked: an effect that calls them must not start depending on the list
 * (or on the options and undo action) they touch, or every write would re-run it.
 */
export const toast = {
	get items(): readonly ToastItem[] {
		return items;
	},

	show(o: ToastOptions): string {
		return untrack(() => {
			const id = `toast-${++seq}`;
			const next = [...items, { ...o, id }];
			while (next.length > MAX_VISIBLE) {
				const oldest = next.shift();
				if (oldest) forget(oldest.id);
			}
			items = next;
			const ms = o.duration ?? DEFAULT_DURATION;
			const m: Meta = {
				undo: false,
				left: Number.isFinite(ms) && ms > 0 ? ms : null,
				startedAt: 0,
				handle: undefined,
				onclose: o.onclose
			};
			meta.set(id, m);
			if (!paused) arm(id, m);
			return id;
		});
	},

	dismiss(id: string): void {
		untrack(() => {
			forget(id);
			items = items.filter((t) => t.id !== id);
		});
	},

	undoable(message: string, undo: () => void | Promise<void>, onclose?: () => void): string {
		const id = toast.show({ message, action: { label: ru.common.undo, run: undo }, onclose });
		const m = meta.get(id);
		if (m) m.undo = true;
		return id;
	},

	/** Runs the undo of the newest undoable toast and removes it; false when there is none. */
	undoLast(): boolean {
		return untrack(() => {
			const last = items.findLast((t) => meta.get(t.id)?.undo);
			if (!last?.action) return false;
			toast.dismiss(last.id);
			void last.action.run();
			return true;
		});
	},

	/** Removes every toast: after signing out nobody may press the undo of the previous user. */
	clear(): void {
		untrack(() => {
			for (const id of [...meta.keys()]) forget(id);
			items = [];
		});
	},

	/** Holds every timer while the pointer or focus is on the toasts. */
	pause(): void {
		if (paused) return;
		paused = true;
		const now = Date.now();
		for (const m of meta.values()) {
			if (m.handle === undefined || m.left === null) continue;
			clearTimeout(m.handle);
			m.handle = undefined;
			m.left = Math.max(0, m.left - (now - m.startedAt));
		}
	},

	resume(): void {
		if (!paused) return;
		paused = false;
		for (const [id, m] of meta) arm(id, m);
	}
};

onSessionReset(() => toast.clear());
