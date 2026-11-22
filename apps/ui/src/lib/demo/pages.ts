/**
 * A made-up page of a document for the showcase, as the server sends one: base64 PNG. An act is a title,
 * lines of text and two signatures; a label is a barcode of bars and a caption. Drawn in the colours of
 * the current theme's tokens (no colour literals). Empty where there is no canvas.
 */
export function demoPage(widthMm: number, heightMm: number, dpi = 110): string {
	const px = (mm: number) => Math.round((mm / 25.4) * dpi);
	const canvas = document.createElement('canvas');
	canvas.width = px(widthMm);
	canvas.height = px(heightMm);
	const ctx = canvas.getContext('2d');
	if (!ctx) return '';
	const css = getComputedStyle(document.documentElement);
	const token = (name: string) => css.getPropertyValue(name).trim();
	ctx.fillStyle = token('--surface');
	ctx.fillRect(0, 0, canvas.width, canvas.height);
	ctx.fillStyle = token('--text');
	if (widthMm < 100) {
		const top = px(5);
		const height = px(heightMm - 16);
		for (let x = px(4), i = 0; x < canvas.width - px(4); i++) {
			const bar = (i % 3) + 1;
			if (i % 2 === 0) ctx.fillRect(x, top, bar * 2, height);
			x += bar * 2 + 2;
		}
		ctx.fillRect(px(widthMm / 2 - 10), px(heightMm - 8), px(20), px(2.5));
	} else {
		ctx.fillRect(px(20), px(15), px(90), px(6));
		for (let y = 32, i = 0; y < heightMm - 45; y += 8, i++)
			ctx.fillRect(px(20), px(y), px(widthMm - 40 - (i % 4) * 15), px(1.5));
		ctx.fillRect(px(20), px(heightMm - 25), px(60), px(0.5));
		ctx.fillRect(px(widthMm - 80), px(heightMm - 25), px(60), px(0.5));
	}
	return canvas.toDataURL('image/png').replace(/^data:image\/png;base64,/, '');
}
