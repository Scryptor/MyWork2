// «Leave the screen with own changes not saved» (the order's card, the template editor): any way out
// (a link, Ctrl K, the browser's «Назад») is held until the changes are saved and then goes on; closing
// or reloading the tab gets the browser's own question; the sign-in page is not held, because without a
// session nothing can be saved.
import { beforeNavigate, goto } from '$app/navigation';
import { isSignIn } from '#lib/app/login.js';

/** The part of SvelteKit's `BeforeNavigate` the guard reads. */
export interface LeaveNavigation {
	cancel(): void;
	readonly willUnload: boolean;
	readonly type: string;
	readonly delta?: number;
	readonly to: { readonly url: URL } | null;
	readonly from?: { readonly url: URL } | null;
}

export interface LeaveGuardOptions<E> {
	/** The editor of the screen; null: nothing to hold. */
	editor: () => E | null;
	unsaved: (editor: E) => boolean;
	/** Saves what is typed (and tells why not); false: stay. */
	saved: (editor: E) => Promise<boolean>;
	/** A way out that keeps the screen as it is (a change of the list's own filter): nothing to wait for. */
	skip?: (nav: LeaveNavigation, editor: E) => boolean;
	/** Where SvelteKit was going; `goto` by default. */
	goto?: (url: URL) => void;
	/** The browser's «Назад» by its delta; `history.go` by default. */
	go?: (delta: number) => void;
}

/** The handler of `beforeNavigate`, apart from SvelteKit for its tests. */
export function createLeaveGuard<E>(o: LeaveGuardOptions<E>): (nav: LeaveNavigation) => void {
	// eslint-disable-next-line svelte/no-navigation-without-resolve -- where SvelteKit was going
	const open = o.goto ?? ((url: URL) => void goto(url));
	const back = o.go ?? ((delta: number) => history.go(delta));
	let resume: (() => void) | null = null;
	return (nav) => {
		const ed = o.editor();
		if (ed === null || !o.unsaved(ed) || (nav.to && isSignIn(nav.to.url))) return;
		if (o.skip?.(nav, ed)) return;
		nav.cancel();
		if (nav.willUnload || !nav.to) return;
		const { url } = nav.to;
		const delta = nav.type === 'popstate' ? (nav.delta ?? 0) : 0;
		const waiting = resume !== null;
		resume = delta !== 0 ? () => back(delta) : () => open(url);
		if (waiting) return;
		void o.saved(ed).then((ok) => {
			const go = resume;
			resume = null;
			if (ok && o.editor() === ed) go?.();
		});
	};
}

/** Holds the way out of the screen while its changes are not saved; call it while the component is made. */
export function leaveGuard<E>(o: LeaveGuardOptions<E>): void {
	beforeNavigate(createLeaveGuard(o));
}
