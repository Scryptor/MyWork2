import { afterEach, expect, it } from 'vitest';
import { leaveDesktop } from './fake-desktop.js';
import { isTauri } from './tauri.js';

const bridge = window as unknown as Record<string, unknown>;

afterEach(leaveDesktop);

it('a browser has no bridge of the desktop program', () => {
	expect(isTauri()).toBe(false);
});

it('the desktop program is known by its bridge', () => {
	bridge.__TAURI_INTERNALS__ = {};
	expect(isTauri()).toBe(true);
});
