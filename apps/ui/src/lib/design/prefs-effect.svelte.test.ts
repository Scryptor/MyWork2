import { afterEach, expect, it, vi } from 'vitest';
import { flushSync } from 'svelte';
import { DEFAULT_PREFS, prefs } from './prefs.svelte.js';

afterEach(() => vi.unstubAllGlobals());

it('init() inside $effect is not re-run by pref changes', () => {
	const getItem = vi.fn(() => {
		throw new Error('denied');
	});
	vi.stubGlobal('localStorage', {
		getItem,
		setItem: () => {
			throw new Error('denied');
		}
	});
	const addEventListener = vi.fn();
	vi.stubGlobal('matchMedia', () => ({
		matches: false,
		addEventListener,
		removeEventListener: vi.fn()
	}));

	const stop = $effect.root(() => {
		$effect(() => prefs.init());
	});
	flushSync();
	expect(prefs.value).toEqual(DEFAULT_PREFS);
	expect(getItem).toHaveBeenCalledTimes(1);

	prefs.set({ density: 'compact' });
	flushSync();
	expect(prefs.value.density).toBe('compact');
	expect(getItem).toHaveBeenCalledTimes(1);
	expect(addEventListener).toHaveBeenCalledTimes(1);
	stop();
});
