import { afterEach, describe, expect, it } from 'vitest';
import { mockIPC } from '@tauri-apps/api/mocks';
import { leaveDesktop } from './fake-desktop.js';
import { pickFolder } from './folder.js';

afterEach(leaveDesktop);

describe('pickFolder', () => {
	it('in a browser there is no dialog: null, and the screen shows the path field', async () => {
		expect(await pickFolder()).toBeNull();
	});

	it('in the desktop program the system dialog chooses one folder', async () => {
		const calls: [string, unknown][] = [];
		mockIPC((cmd, args) => {
			calls.push([cmd, args]);
			return cmd === 'plugin:dialog|open' ? '/Users/u/MyWork2' : null;
		});
		expect(await pickFolder()).toBe('/Users/u/MyWork2');
		expect(calls).toEqual([
			[
				'plugin:dialog|open',
				{ options: expect.objectContaining({ directory: true, multiple: false }) }
			]
		]);
	});

	it('a dialog closed without a choice gives null', async () => {
		mockIPC(() => null);
		expect(await pickFolder()).toBeNull();
	});
});
