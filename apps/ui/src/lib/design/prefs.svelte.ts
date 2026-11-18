import { untrack } from 'svelte';
import { readStorage, writeStorage } from './storage.js';
import {
	applyDisplay,
	computeDisplay,
	UI_SCALES,
	type Density,
	type Display,
	type DisplayEnv,
	type Prefs,
	type ThemePref
} from './display.js';

export type { Density, Prefs, ThemePref };
export type { ScalePref, UiScale } from './display.js';

export const PREFS_KEY = 'mw.prefs';
/** Without `v: 2` the stored `scale` is ignored: before 5.3 nearly everyone saved `1` with the theme. */
export const PREFS_VERSION = 2;
export const DEFAULT_PREFS: Prefs = { theme: 'system', scale: 'auto', density: 'comfortable' };

const THEMES: readonly ThemePref[] = ['system', 'light', 'dark'];
const DENSITIES: readonly Density[] = ['compact', 'comfortable'];

/** Never throws: an invalid or missing field falls back to its default. */
export function parsePrefs(raw: string | null): Prefs {
	let obj: unknown;
	try {
		obj = raw === null ? null : JSON.parse(raw);
	} catch {
		obj = null;
	}
	const o = (typeof obj === 'object' && obj !== null ? obj : {}) as Record<string, unknown>;
	const scale =
		o.v === PREFS_VERSION
			? o.scale === 'auto'
				? 'auto'
				: (UI_SCALES.find((s) => s === o.scale) ?? DEFAULT_PREFS.scale)
			: DEFAULT_PREFS.scale;
	return {
		theme: THEMES.find((t) => t === o.theme) ?? DEFAULT_PREFS.theme,
		scale,
		density: DENSITIES.find((d) => d === o.density) ?? DEFAULT_PREFS.density
	};
}

let value = $state<Prefs>({ ...DEFAULT_PREFS });
let env = $state<DisplayEnv>({
	systemDark: false,
	screenWidth: 0,
	screenHeight: 0,
	windowWidth: 1280
});
const current = $derived<Display>(computeDisplay(value, env));

function apply(): void {
	if (typeof document !== 'undefined') applyDisplay(current, document.documentElement);
}

export const prefs = {
	get value(): Prefs {
		return value;
	},
	get resolvedTheme(): 'light' | 'dark' {
		return current.theme;
	},
	set(patch: Partial<Prefs>): void {
		value = parsePrefs(JSON.stringify({ ...value, ...patch, v: PREFS_VERSION }));
		// storage unavailable: keep working in memory
		writeStorage(PREFS_KEY, JSON.stringify({ v: PREFS_VERSION, ...value }));
		apply();
	},
	init(): () => void {
		// untracked: safe to call inside $effect without re-running on every pref change
		return untrack(initPrefs);
	}
};

export const display = {
	get value(): Display {
		return current;
	},
	get phone(): boolean {
		return current.band === 'xs';
	},
	get narrow(): boolean {
		return current.band === 'xs' || current.band === 'sm';
	},
	get cardColumns(): boolean {
		return current.band === 'lg' || current.band === 'xl';
	}
};

function readEnv(systemDark: boolean): DisplayEnv {
	return {
		systemDark,
		screenWidth: window.screen?.width ?? 0,
		screenHeight: window.screen?.height ?? 0,
		windowWidth: window.innerWidth
	};
}

function initPrefs(): () => void {
	value = parsePrefs(readStorage(PREFS_KEY));
	let stopped = false;
	let frame = 0;
	let darkMq: MediaQueryList | null = null;
	let resMq: MediaQueryList | null = null;
	let resDppx = 0;

	const currentDark = () => darkMq?.matches ?? false;
	const refresh = () => {
		const next = readEnv(currentDark());
		// An unchanged environment must not make `display.value` a new object.
		if (
			next.systemDark !== env.systemDark ||
			next.screenWidth !== env.screenWidth ||
			next.screenHeight !== env.screenHeight ||
			next.windowWidth !== env.windowWidth
		)
			env = next;
		apply();
	};
	// Several events in a row become one update per frame.
	const schedule = () => {
		if (stopped || frame) return;
		frame = requestAnimationFrame(() => {
			frame = 0;
			if (!stopped) refresh();
		});
	};
	const onResolution = () => {
		watchResolution();
		schedule();
	};
	function watchResolution(): void {
		resMq?.removeEventListener('change', onResolution);
		resMq = null;
		try {
			resDppx = window.devicePixelRatio || 1;
			resMq = window.matchMedia(`(resolution: ${resDppx}dppx)`);
			resMq.addEventListener('change', onResolution);
		} catch {
			resMq = null;
		}
	}

	try {
		darkMq = window.matchMedia('(prefers-color-scheme: dark)');
		darkMq.addEventListener('change', schedule);
	} catch {
		darkMq = null;
	}
	watchResolution();
	window.addEventListener('resize', schedule);
	window.addEventListener('focus', schedule);
	const screenObj = window.screen as unknown as EventTarget | undefined;
	screenObj?.addEventListener?.('change', schedule);

	refresh();
	return () => {
		stopped = true;
		if (frame) cancelAnimationFrame(frame);
		frame = 0;
		darkMq?.removeEventListener('change', schedule);
		resMq?.removeEventListener('change', onResolution);
		window.removeEventListener('resize', schedule);
		window.removeEventListener('focus', schedule);
		screenObj?.removeEventListener?.('change', schedule);
	};
}
