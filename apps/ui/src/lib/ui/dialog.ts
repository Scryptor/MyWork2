import { tick } from 'svelte';

/**
 * `onInteractOutside` of a modal: toasts are pressed with the dialog open («Отменить» of what the dialog
 * just did), and the dialog stays.
 */
export function keepOpenOnToasts(e: Event): void {
	// `data-mw-toaster` marks the toast region in Toaster.svelte.
	if (e.target instanceof Element && e.target.closest('[data-mw-toaster]')) e.preventDefault();
}

/**
 * Focus after a dialog closes: bits-ui returns it to the element that opened the dialog. When that element
 * is gone (its draft was deleted, its row left the list), focus goes to `<main>` (tabindex="-1")
 * instead of nowhere.
 */
export function dialogFocus() {
	let opener: Element | null = null;
	const main = () => document.querySelector<HTMLElement>('main');
	return {
		/** When the dialog opens, before it takes focus. */
		opened(): void {
			opener = document.activeElement;
		},
		/** `onCloseAutoFocus`, as the dialog goes. */
		closed(e: Event): void {
			const target = opener;
			opener = null;
			if (!target?.isConnected) {
				const m = main();
				if (!m) return;
				e.preventDefault();
				m.focus({ preventScroll: true });
				return;
			}
			// The opener may still go in this very update: look again once it is done.
			void tick().then(() => {
				if (!target.isConnected && document.activeElement === document.body) {
					main()?.focus({ preventScroll: true });
				}
			});
		}
	};
}
