// The program of this computer: restart it and open a folder in the file manager. The commands are the
// desktop program's own (`apps/desktop/src-tauri/src/system.rs`); a browser has neither.

import type { InvokeArgs } from '@tauri-apps/api/core';
import { ru } from '#lib/i18n/ru.js';
import { isTauri } from '#lib/platform/tauri.js';

/** Only the program can restart itself and open a folder of this computer. */
export function canControlProgram(): boolean {
	return isTauri();
}

/** A command of the program; its error (a Russian string) becomes an `Error` with that text. */
async function call(command: string, args?: InvokeArgs): Promise<void> {
	if (!isTauri()) throw new Error(ru.platform.onlyInProgram);
	const { invoke } = await import('@tauri-apps/api/core');
	try {
		await invoke<void>(command, args);
	} catch (e) {
		const message = typeof e === 'string' ? e : e instanceof Error ? e.message : ru.platform.failed;
		throw new Error(message, { cause: e });
	}
}

/** Restarts the program: the server stops and the data folder is released before the new copy starts. */
export const restartProgram = () => call('restart_app');

/** Opens a folder of this computer in the file manager. */
export const openFolder = (path: string) => call('open_folder', { path });
