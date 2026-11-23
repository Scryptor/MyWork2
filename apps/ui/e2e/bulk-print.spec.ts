import fs from 'node:fs/promises';
import { expect, test } from '@playwright/test';
import { createOrder, uniq } from './support/api';

// «Печать ▾» of the selection bar (server of 4173, open mode). A browser prints nothing itself: the menu
// has only «Сохранить PDF», which gives one file with a sticker of each selected order. The core tests pin
// the number of pages; here the file is the server's, named by its count.

test('«Сохранить PDF» of three selected orders is one file named after their number', async ({
	page,
	request
}) => {
	const marker = uniq('BP');
	const ids = [
		(await createOrder(request, { model: marker })).id,
		(await createOrder(request, { model: marker })).id,
		(await createOrder(request, { model: marker })).id
	];
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	const grid = page.getByRole('grid', { name: 'Заказы' });
	for (const id of ids) {
		await expect(
			grid
				.getByRole('row')
				.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) })
		).toBeVisible();
	}
	await grid.getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	const bar = page.getByRole('region', { name: 'Выбранные строки' });
	await expect(bar).toContainText('Выбрано 3');

	await bar.getByRole('button', { name: 'Печать' }).click();
	const menu = page.getByRole('menu', { name: 'Печать' });
	// No printer in a browser: only the PDF section.
	await expect(menu.getByRole('group', { name: 'Печать' })).toHaveCount(0);
	const save = menu.getByRole('group', { name: 'Сохранить PDF' });
	const download = page.waitForEvent('download');
	await save.getByRole('menuitem', { name: 'Наклейка' }).click();
	const file = await download;

	expect(file.suggestedFilename()).toBe('naklejka-3.pdf');
	const path = await file.path();
	const bytes = await fs.readFile(path);
	expect(bytes.subarray(0, 4).toString('latin1')).toBe('%PDF');
});
