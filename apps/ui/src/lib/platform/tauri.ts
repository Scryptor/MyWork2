// The desktop program (Tauri) or a browser. The desktop program gives the page its bridge before any script runs.

/** The program runs inside the desktop program (Tauri), not in a browser. */
export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}
