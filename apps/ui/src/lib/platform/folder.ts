// Choosing a folder on the computer (the MyWork 2 folder to transfer). The desktop program has the system's
// folder dialog; a browser cannot give a full path, so there the screen shows a path field instead.

/** The program runs inside the desktop program (Tauri), not in a browser. */
export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * The full path of a folder the user chose, or null: no dialog here (a browser), or it was closed. The
 * dialog module is loaded only in the desktop program; a browser never downloads it.
 */
export async function pickFolder(): Promise<string | null> {
	if (!isTauri()) return null;
	const { open } = await import('@tauri-apps/plugin-dialog');
	const chosen = await open({ directory: true, multiple: false });
	return typeof chosen === 'string' ? chosen : null;
}
