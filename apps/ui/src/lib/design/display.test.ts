import { describe, expect, it } from 'vitest';
import { applyDisplay, autoScale, bandForWidth, computeDisplay, stepScale } from './display.js';

describe('autoScale', () => {
	it.each([
		[1280, 720, 1],
		[1728, 1117, 1],
		[1920, 1080, 1],
		[2048, 1152, 1],
		[2560, 1440, 1.1],
		[3440, 1440, 1.1],
		[3008, 1692, 1.25],
		[3840, 2160, 1.5],
		[0, 0, 1],
		[NaN, NaN, 1]
	])('autoScale(%i, %i) = %f', (w, h, s) => expect(autoScale(w, h)).toBe(s));
});

describe('stepScale', () => {
	it('stops at the ends', () => {
		expect(stepScale(1, 1)).toBe(1.1);
		expect(stepScale(1.25, -1)).toBe(1.1);
		expect(stepScale(1.5, 1)).toBe(1.5);
		expect(stepScale(0.9, -1)).toBe(0.9);
	});
});

describe('bandForWidth', () => {
	it.each([
		[599, 'xs'],
		[600, 'sm'],
		[899, 'sm'],
		[900, 'md'],
		[1023, 'md'],
		[1024, 'lg'],
		[1279, 'lg'],
		[1280, 'xl']
	])('bandForWidth(%i) = %s', (w, b) => expect(bandForWidth(w)).toBe(b));
});

describe('computeDisplay', () => {
	it('auto on a 4K screen in a half-width window', () => {
		const d = computeDisplay(
			{ theme: 'system', scale: 'auto', density: 'comfortable' },
			{ systemDark: true, screenWidth: 3840, screenHeight: 2160, windowWidth: 1920 }
		);
		expect(d).toMatchObject({
			theme: 'dark',
			scale: 1.5,
			layoutWidth: 1280,
			band: 'xl',
			layout: 'full'
		});
	});
	it('manual 125 % in a 1366 window is a rail (1093 layout px)', () => {
		const d = computeDisplay(
			{ theme: 'light', scale: 1.25, density: 'compact' },
			{ systemDark: false, screenWidth: 1366, screenHeight: 768, windowWidth: 1366 }
		);
		expect(d).toMatchObject({ scale: 1.25, band: 'lg', layout: 'rail', density: 'compact' });
	});
});

describe('applyDisplay', () => {
	it('sets the root', () => {
		const root = document.createElement('html');
		const d = computeDisplay(
			{ theme: 'dark', scale: 1.1, density: 'compact' },
			{ systemDark: false, screenWidth: 1920, screenHeight: 1080, windowWidth: 1100 }
		);
		applyDisplay(d, root);
		expect(root.getAttribute('data-theme')).toBe('dark');
		expect(root.getAttribute('data-density')).toBe('compact');
		expect(root.getAttribute('data-width')).toBe('md');
		expect(root.style.getPropertyValue('--ui-scale')).toBe('1.1');
	});
});
