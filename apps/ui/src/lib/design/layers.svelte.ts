export type LayerKind = 'popup' | 'modal' | 'panel';

interface Entry {
	kind: LayerKind;
	close: () => void;
}

let stack = $state.raw<Entry[]>([]);
let listening = false;

function onKeydown(e: KeyboardEvent) {
	// A bits-ui layer that handled this Escape calls preventDefault (it may already be gone from the
	// stack by now); Escape during IME input cancels the composition.
	if (e.key !== 'Escape' || e.defaultPrevented || e.isComposing || e.keyCode === 229) return;
	const top = stack[stack.length - 1];
	// bits-ui closes popups and modals itself; only panels are ours to close.
	if (top?.kind === 'panel') top.close();
}

/** Stack of open overlays: tells hotkeys to stand down and closes the top panel on Esc. */
export const layers = {
	open(kind: LayerKind, close: () => void): () => void {
		if (!listening) {
			window.addEventListener('keydown', onKeydown);
			listening = true;
		}
		const entry: Entry = { kind, close };
		stack = [...stack, entry];
		return () => {
			stack = stack.filter((x) => x !== entry);
		};
	},
	get topKind(): LayerKind | null {
		return stack[stack.length - 1]?.kind ?? null;
	},
	get blocking(): boolean {
		const k = stack[stack.length - 1]?.kind;
		return k === 'popup' || k === 'modal';
	}
};
