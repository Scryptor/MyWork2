import { BAND_LIMITS, type ShellLayout } from './breakpoints.js';

export type ThemePref = 'system' | 'light' | 'dark';
export type Density = 'compact' | 'comfortable';
export type UiScale = 0.9 | 1 | 1.1 | 1.25 | 1.5;
export type ScalePref = 'auto' | UiScale;
export type WidthBand = 'xs' | 'sm' | 'md' | 'lg' | 'xl';

export interface Prefs {
	theme: ThemePref;
	scale: ScalePref;
	density: Density;
}

/** Ascending. */
export const UI_SCALES: readonly UiScale[] = [0.9, 1, 1.1, 1.25, 1.5];

export interface DisplayEnv {
	systemDark: boolean;
	screenWidth: number;
	screenHeight: number;
	windowWidth: number;
}

export interface Display {
	theme: 'light' | 'dark';
	density: Density;
	scale: UiScale;
	layoutWidth: number;
	band: WidthBand;
	layout: ShellLayout;
}

/**
 * "Auto" follows the screen, not the window: the screen's size in points tracks the monitor
 * resolution, and resizing a window must not change the text size.
 */
export function autoScale(screenWidth: number, screenHeight: number): UiScale {
	if (!(screenWidth > 0) || !(screenHeight > 0)) return 1;
	const e = Math.min(screenWidth, (screenHeight * 16) / 9);
	if (e < 2200) return 1;
	if (e < 2880) return 1.1;
	if (e < 3400) return 1.25;
	return 1.5;
}

export function stepScale(current: UiScale, dir: 1 | -1): UiScale {
	const i = UI_SCALES.indexOf(current) + dir;
	return UI_SCALES[Math.min(UI_SCALES.length - 1, Math.max(0, i))] ?? current;
}

export function bandForWidth(layoutWidth: number): WidthBand {
	if (layoutWidth < BAND_LIMITS.xs) return 'xs';
	if (layoutWidth < BAND_LIMITS.sm) return 'sm';
	if (layoutWidth < BAND_LIMITS.md) return 'md';
	if (layoutWidth < BAND_LIMITS.lg) return 'lg';
	return 'xl';
}

function layoutForWidth(width: number): ShellLayout {
	if (width < BAND_LIMITS.xs) return 'phone';
	if (width < BAND_LIMITS.lg) return 'rail';
	return 'full';
}

export function computeDisplay(p: Prefs, env: DisplayEnv): Display {
	const scale = p.scale === 'auto' ? autoScale(env.screenWidth, env.screenHeight) : p.scale;
	const layoutWidth = env.windowWidth / scale;
	return {
		theme: p.theme === 'system' ? (env.systemDark ? 'dark' : 'light') : p.theme,
		density: p.density,
		scale,
		layoutWidth,
		band: bandForWidth(layoutWidth),
		layout: layoutForWidth(layoutWidth)
	};
}

/** data-theme, data-density, data-width (= band), style --ui-scale. */
export function applyDisplay(d: Display, root: HTMLElement): void {
	root.setAttribute('data-theme', d.theme);
	root.setAttribute('data-density', d.density);
	root.setAttribute('data-width', d.band);
	root.style.setProperty('--ui-scale', String(d.scale));
}
