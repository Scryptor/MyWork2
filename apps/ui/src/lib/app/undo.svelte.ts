import { errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';

interface Entry {
	run: () => Promise<void>;
	toastId: string;
}

let entries: Entry[] = [];

async function perform(entry: Entry): Promise<void> {
	if (!entries.includes(entry)) return;
	entries = entries.filter((e) => e !== entry);
	toast.dismiss(entry.toastId);
	try {
		await entry.run();
	} catch (e) {
		toast.show({ message: ru.app.undoFailed(errorMessage(e)), tone: 'danger' });
	}
}

/**
 * «Отменить» of the last actions. `push` shows the toast «label · Отменить» itself: callers do not show
 * another one. Ctrl/⌘+Z (`run`) and the toast button undo the same entry, so an action can be undone
 * while its toast is on screen (6 s, longer under the pointer), each one once.
 */
export const undo = {
	push(label: string, run: () => Promise<void>): void {
		const entry: Entry = { run, toastId: '' };
		entry.toastId = toast.undoable(label, () => perform(entry));
		entries = [...entries, entry];
	},

	/** Undoes the newest action whose toast is still shown; nothing when there is none. */
	async run(): Promise<void> {
		const shown = toast.items.map((t) => t.id);
		entries = entries.filter((e) => shown.includes(e.toastId));
		const last = entries[entries.length - 1];
		if (last) await perform(last);
	}
};
