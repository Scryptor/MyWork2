import { expect, test } from '@playwright/test';
import { layoutOffenders } from './support/layout';

// The sweep's own checks: `data-scroll-x` excuses only a box that really scrolls sideways.

const strip = (overflow: string) => `
	<div data-scroll-x style="width:200px;overflow-x:${overflow};white-space:nowrap">
		<button style="width:320px">Длинная кнопка</button>
	</div>`;

async function offenders(page: import('@playwright/test').Page, overflow: string) {
	await page.setContent(`<body style="margin:0">${strip(overflow)}</body>`);
	const found = await layoutOffenders(page, []);
	return found.flatMap((f) => f.offenders);
}

test('a strip that scrolls sideways on purpose is not reported', async ({ page }) => {
	expect(await offenders(page, 'auto')).toEqual([]);
});

test('data-scroll-x on a box that cuts instead of scrolling is still checked', async ({ page }) => {
	const found = await offenders(page, 'hidden');
	expect(found.length).toBeGreaterThan(0);
	expect(found.join('\n')).toMatch(/wider inside/);
});
