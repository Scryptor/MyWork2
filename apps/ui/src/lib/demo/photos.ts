import type { StatusHue } from '#lib/design/status-colors.js';

/**
 * A made-up «photo» for the gallery: a phone outline on a tinted square, drawn in the colours of the
 * current theme's tokens (no colour literals). Empty where there is no canvas.
 */
export function demoPhoto(hue: StatusHue, size = 160): string {
	const canvas = document.createElement('canvas');
	canvas.width = size;
	canvas.height = size;
	const ctx = canvas.getContext('2d');
	if (!ctx) return '';
	const css = getComputedStyle(document.documentElement);
	const token = (name: string) => css.getPropertyValue(name).trim();
	ctx.fillStyle = token(`--st-${hue}-bg`);
	ctx.fillRect(0, 0, size, size);
	ctx.fillStyle = token(`--st-${hue}-dot`);
	ctx.beginPath();
	ctx.roundRect(size * 0.32, size * 0.12, size * 0.36, size * 0.76, size * 0.06);
	ctx.fill();
	ctx.fillStyle = token(`--st-${hue}-bg`);
	ctx.fillRect(size * 0.36, size * 0.18, size * 0.28, size * 0.56);
	return canvas.toDataURL('image/png');
}
