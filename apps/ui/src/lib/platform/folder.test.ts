import { afterEach, describe, expect, it } from 'vitest';
import { isTauri, pickFolder } from './folder.js';

const bridge = window as unknown as Record<string, unknown>;

afterEach(() => {
	delete bridge.__TAURI_INTERNALS__;
});

describe('pickFolder', () => {
	it('in a browser there is no dialog: null, and the screen shows the path field', async () => {
		expect(isTauri()).toBe(false);
		expect(await pickFolder()).toBeNull();
	});

	it('the desktop program is known by its bridge', () => {
		bridge.__TAURI_INTERNALS__ = {};
		expect(isTauri()).toBe(true);
	});
});
