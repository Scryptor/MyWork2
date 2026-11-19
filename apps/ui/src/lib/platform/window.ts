// The desktop program's window events the display environment needs (`#lib/design/prefs.svelte.js`).
// Moving the window to another monitor with the same pixel ratio fires no `resize` or `focus` in WebKit,
// so «Авто» would keep the old monitor's scale until then; the window itself reports the move.

import { isTauri } from './folder.js';

type Unlisten = () => void;

/**
 * Calls `onchange` whenever the desktop window moves or its monitor's scale changes; returns the function
 * that stops it. In a browser there is nothing to follow. The subscriptions are asynchronous: one that is
 * still on its way when stopped is taken back as soon as it is made.
 */
export function watchWindow(onchange: () => void): () => void {
	if (!isTauri()) return () => {};
	let stopped = false;
	const subscribed: Unlisten[] = [];
	const keep = (unlisten: Unlisten) => {
		if (stopped) unlisten();
		else subscribed.push(unlisten);
	};
	void (async () => {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		const win = getCurrentWindow();
		if (stopped) return;
		keep(await win.onMoved(() => onchange()));
		if (stopped) return;
		keep(await win.onScaleChanged(() => onchange()));
	})().catch(() => {
		// Without the window events «Авто» still follows `resize` and `focus`.
	});
	return () => {
		stopped = true;
		for (const unlisten of subscribed.splice(0)) unlisten();
	};
}
