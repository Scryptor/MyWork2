// The filter field of a list (orders, clients, stock): its text is ahead of the address while the user types,
// goes to the address once typing pauses, and follows the address when that changes from outside (a saved
// view, «Назад»). Call it while the page initializes: it owns effects.
import { untrack } from 'svelte';
import { registerHotkey } from '#lib/design/hotkeys.svelte.js';

/** The list filter asks the server once typing pauses for this long. */
export const FILTER_MS = 250;

export function createListFilter(opts: {
	/** The filter text the address holds now. */
	query: () => string;
	/** Typing paused on this text: it goes to the address (a replace of the history entry). */
	send: (query: string) => void;
}) {
	let text = $state(untrack(opts.query));
	let ref = $state<HTMLInputElement | null>(null);
	let typing: ReturnType<typeof setTimeout> | undefined;
	/** The text sent last from typing; its arrival in the address does not touch the field. */
	let sent: string | null = null;

	// A derived string: the effect reruns when the text of the address changes, not at every other change
	// of the view (a chip, a column), which would put the address's old text over what is being typed.
	const addressText = $derived(opts.query());
	$effect(() => {
		const q = addressText;
		untrack(() => {
			if (q === sent) {
				sent = null;
				return;
			}
			sent = null;
			if (q !== text) {
				clearTimeout(typing);
				text = q;
			}
		});
	});
	$effect(() => () => clearTimeout(typing));
	$effect(() => registerHotkey('/', () => ref?.focus()));

	return {
		/** The text of the field (`bind:value`). */
		get text(): string {
			return text;
		},
		set text(v: string) {
			text = v;
		},
		/** The input element (`bind:ref`): `/` focuses it. */
		get ref(): HTMLInputElement | null {
			return ref;
		},
		set ref(v: HTMLInputElement | null) {
			ref = v;
		},
		/** A change of the view or a panel takes the text as typed: no send is left waiting. */
		cancel(): void {
			clearTimeout(typing);
		},
		/** `oninput` of the field. */
		typed(value: string): void {
			clearTimeout(typing);
			typing = setTimeout(() => {
				if (value === opts.query()) return;
				sent = value;
				opts.send(value);
			}, FILTER_MS);
		}
	};
}
