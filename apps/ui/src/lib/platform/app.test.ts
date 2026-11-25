import { mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it } from 'vitest';
import { leaveDesktop } from '#lib/platform/fake-desktop.js';
import { canControlProgram, openFolder, restartProgram } from './app.js';

// The program restarts and opens a folder through its own commands; in a browser both say why not.

afterEach(leaveDesktop);

function desktop(fail?: { command: string; error: unknown }) {
	const calls: [string, unknown][] = [];
	mockIPC((cmd, args) => {
		calls.push([cmd, args]);
		if (fail && cmd === fail.command) throw fail.error;
		return null;
	});
	return calls;
}

describe('in a browser', () => {
	it('the program cannot be controlled, and a command says why', async () => {
		expect(canControlProgram()).toBe(false);
		await expect(openFolder('/x')).rejects.toThrow(
			'Это работает только в программе для компьютера'
		);
		await expect(restartProgram()).rejects.toThrow(
			'Это работает только в программе для компьютера'
		);
	});
});

describe('in the desktop program', () => {
	it('restarts the program by its command', async () => {
		const calls = desktop();
		expect(canControlProgram()).toBe(true);
		await restartProgram();
		expect(calls.map(([cmd]) => cmd)).toEqual(['restart_app']);
	});

	it('opens a folder by its path', async () => {
		const calls = desktop();
		await openFolder('/x');
		expect(calls).toEqual([['open_folder', { path: '/x' }]]);
	});

	it('the error of the program (a string) becomes an Error with that text', async () => {
		desktop({ command: 'open_folder', error: 'Папки нет: /x' });
		const failure = openFolder('/x');
		await expect(failure).rejects.toBeInstanceOf(Error);
		await expect(failure).rejects.toThrow('Папки нет: /x');
	});

	it('an error of another shape says the program could not do it', async () => {
		desktop({ command: 'restart_app', error: { code: 7 } });
		await expect(restartProgram()).rejects.toThrow('Программа не смогла выполнить команду');
	});
});
