import { tick } from 'svelte';

/**
 * `onInteractOutside` of a modal: toasts are pressed with the dialog open («Отменить» of what the dialog
 * just did), and the dialog stays.
 */
export function keepOpenOnToasts(e: Event): void {
	// `data-mw-toaster` marks the toast region in Toaster.svelte.
	if (e.target instanceof Element && e.target.closest('[data-mw-toaster]')) e.preventDefault();
}

const CONTROL =
	'button, a[href], input, select, textarea, summary, [tabindex], [contenteditable]:not([contenteditable="false"])';

/** The control the last pointer press landed on; a key since then makes it stale. */
let pressed: Element | null = null;
let tracking = false;

function track(): void {
	if (tracking || typeof document === 'undefined') return;
	tracking = true;
	document.addEventListener(
		'pointerdown',
		(e) => (pressed = e.target instanceof Element ? e.target.closest(CONTROL) : null),
		true
	);
	document.addEventListener('keydown', () => (pressed = null), true);
}

/**
 * Focus after a dialog closes: back to the element that opened it, or to `<main>` (tabindex="-1") when
 * that element is gone (its draft was deleted, its row left the list) or there was none (`<body>`).
 * A click focuses the button in Chromium but not in WebKit, which leaves focus on an element around it
 * (`<main>`, a panel) or on `<body>`: then the pressed button is the opener.
 */
export function dialogFocus() {
	track();
	let opener: Element | null = null;
	const main = () => document.querySelector<HTMLElement>('main');
	return {
		/** When the dialog opens, before it takes focus. */
		opened(): void {
			const active = document.activeElement;
			const press = pressed?.isConnected ? pressed : null;
			opener =
				press && (!active || active === document.body || active.contains(press)) ? press : active;
		},
		/** `onCloseAutoFocus`, as the dialog goes. */
		closed(e: Event): void {
			const target = opener instanceof HTMLElement && opener !== document.body ? opener : null;
			opener = null;
			const back = target?.isConnected ? target : main();
			if (!back) return; // no <main> here: bits-ui's default
			e.preventDefault();
			back.focus({ preventScroll: true });
			// The opener may still go in this very update, or not take focus: look again once it is done.
			void tick().then(() => {
				if (document.activeElement === document.body) main()?.focus({ preventScroll: true });
			});
		}
	};
}
