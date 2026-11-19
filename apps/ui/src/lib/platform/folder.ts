// Choosing a folder on the computer (the MyWork 2 folder to transfer). The desktop program has the system's
// folder dialog; a browser cannot give a full path, so there the screen shows a path field instead.

/** The program runs inside the desktop program (Tauri), not in a browser. */
export function isTauri(): boolean {
	return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * The full path of a folder the user chose, or null: no dialog here (a browser), or it was closed. The
 * dialog of the desktop program comes with it (plan 5.6, task 4); until then the path is typed in.
 */
export async function pickFolder(): Promise<string | null> {
	if (!isTauri()) return null;
	return null;
}
