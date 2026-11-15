export type ThemePref = 'system' | 'light' | 'dark';
export type Density = 'compact' | 'comfortable';
export type UiScale = 0.9375 | 1 | 1.125 | 1.25;
export interface Prefs {
	theme: ThemePref;
	scale: UiScale;
	density: Density;
}

export const PREFS_KEY = 'mw.prefs';
export const DEFAULT_PREFS: Prefs = { theme: 'system', scale: 1, density: 'comfortable' };

const THEMES: readonly ThemePref[] = ['system', 'light', 'dark'];
const SCALES: readonly UiScale[] = [0.9375, 1, 1.125, 1.25];
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
	return {
		theme: THEMES.find((t) => t === o.theme) ?? DEFAULT_PREFS.theme,
		scale: SCALES.find((s) => s === o.scale) ?? DEFAULT_PREFS.scale,
		density: DENSITIES.find((d) => d === o.density) ?? DEFAULT_PREFS.density
	};
}

function resolveTheme(p: Prefs, systemDark: boolean): 'light' | 'dark' {
	return p.theme === 'system' ? (systemDark ? 'dark' : 'light') : p.theme;
}

export function applyPrefs(p: Prefs, root: HTMLElement, systemDark: boolean): void {
	root.setAttribute('data-theme', resolveTheme(p, systemDark));
	root.setAttribute('data-density', p.density);
	root.style.setProperty('--ui-scale', String(p.scale));
}

function readStored(): string | null {
	try {
		return localStorage.getItem(PREFS_KEY);
	} catch {
		return null;
	}
}

let value = $state<Prefs>({ ...DEFAULT_PREFS });
let systemDark = $state(false);
const resolvedTheme = $derived(resolveTheme(value, systemDark));

function apply(): void {
	if (typeof document !== 'undefined') applyPrefs(value, document.documentElement, systemDark);
}

export const prefs = {
	get value(): Prefs {
		return value;
	},
	get resolvedTheme(): 'light' | 'dark' {
		return resolvedTheme;
	},
	set(patch: Partial<Prefs>): void {
		value = parsePrefs(JSON.stringify({ ...value, ...patch }));
		try {
			localStorage.setItem(PREFS_KEY, JSON.stringify(value));
		} catch {
			// storage unavailable: keep working in memory
		}
		apply();
	},
	init(): () => void {
		value = parsePrefs(readStored());
		let mq: MediaQueryList | null = null;
		const onChange = (e: MediaQueryListEvent) => {
			systemDark = e.matches;
			apply();
		};
		try {
			mq = window.matchMedia('(prefers-color-scheme: dark)');
			systemDark = mq.matches;
			mq.addEventListener('change', onChange);
		} catch {
			systemDark = false;
		}
		apply();
		return () => mq?.removeEventListener('change', onChange);
	}
};
