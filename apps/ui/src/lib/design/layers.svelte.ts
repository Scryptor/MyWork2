export type LayerKind = 'popup' | 'modal' | 'panel';

interface Entry {
	kind: LayerKind;
	close: () => void;
	/** The element of a modal or panel: what is inside it is on top. */
	element?: () => Element | null;
}

/**
 * The stack is a plain array. As `$state` it lost entries: a layer that opens while another one closes
 * in the same update (Ctrl K → Enter closes the palette and opens the order panel) was dropped by the
 * closing one, whose teardown read the stack from before the opening. `changed` only makes the getters
 * reactive; nothing reads it to change the stack.
 */
let stack: Entry[] = [];
let changed = $state.raw<Entry[]>([]);
let listening = false;

function setStack(next: Entry[]) {
	stack = next;
	changed = next;
}

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
	/** `element`: of a modal or a panel, read when needed (it may render after the layer opens). */
	open(kind: LayerKind, close: () => void, element?: () => Element | null): () => void {
		if (!listening) {
			window.addEventListener('keydown', onKeydown);
			listening = true;
		}
		const entry: Entry = { kind, close, element };
		setStack([...stack, entry]);
		return () => setStack(stack.filter((x) => x !== entry));
	},
	/**
	 * An open tooltip takes Esc for itself (bits-ui prevents it): the panel under it closes as well, so one Esc
	 * is enough. Popups and modals keep their own Esc.
	 */
	escapeFromTooltip(e: KeyboardEvent): void {
		if (e.isComposing) return;
		const top = stack[stack.length - 1];
		if (top?.kind === 'panel') top.close();
	},
	/**
	 * Whether input of the whole page (a paste, a drop) belongs to `node`: it is inside the topmost modal
	 * or panel, or none is open. Popups float over their layer and do not count; a modal or panel that
	 * does not tell its element keeps the input from everything under it.
	 */
	takesInput(node: Node): boolean {
		for (let i = stack.length - 1; i >= 0; i--) {
			const entry = stack[i]!;
			if (entry.kind === 'popup') continue;
			return entry.element?.()?.contains(node) ?? false;
		}
		return true;
	},
	get topKind(): LayerKind | null {
		void changed;
		return stack[stack.length - 1]?.kind ?? null;
	},
	get blocking(): boolean {
		void changed;
		const k = stack[stack.length - 1]?.kind;
		return k === 'popup' || k === 'modal';
	}
};
