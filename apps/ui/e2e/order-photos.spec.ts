import { expect, test, type Page } from '@playwright/test';
import type { Attachment } from '$api/Attachment';
import { createOrder, TINY_PDF, tinyPng, uniq, uploadPhoto } from './support/api';

// Photos and files of an order, in the panel and the full card, on the server of 4173 (open mode).

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, id: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const cardOf = (page: Page) => page.getByRole('main');
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
const photosOf = (scope: ReturnType<Page['getByRole']>) =>
	scope.getByRole('group', { name: 'Фото', exact: true });
const dropOf = (scope: ReturnType<Page['getByRole']>) =>
	scope.getByRole('group', { name: 'Область для фото' });
const tilesOf = (scope: ReturnType<Page['getByRole']>) =>
	photosOf(scope).getByRole('button', { name: /^Открыть фото/ });

async function openPanel(page: Page, marker: string, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	await rowOf(page, id).getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, id)).toBeVisible();
	await expect(panelOf(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
}

async function openCard(page: Page, id: number) {
	await page.goto(`/orders/${id}`);
	await expect(cardOf(page).getByRole('heading', { name: `№ ${id}`, level: 1 })).toBeVisible();
	await expect(cardOf(page).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
}

const png = (name: string, seed: number) => ({
	name,
	mimeType: 'image/png',
	buffer: tinyPng(seed)
});

test('photos are added in the panel, captioned in the viewer and deleted', async ({
	page,
	request
}) => {
	const m = uniq('Ф');
	const o = await createOrder(request, { model: m });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	await panel.getByRole('button', { name: 'Добавить фото' }).click();
	await expect(page.getByRole('menuitem', { name: 'Снять камерой' })).toBeVisible();
	await expect(page.getByRole('menuitem', { name: 'Выбрать файлы' })).toBeVisible();
	await page.keyboard.press('Escape');
	await dropOf(panel)
		.locator('input[type=file]')
		.setInputFiles([png('a.png', 1), png('b.png', 2)]);
	await expect(tilesOf(panel)).toHaveCount(2);
	const list = await (await request.get(`/api/v1/orders/${o.id}/attachments`)).json();
	expect(list).toHaveLength(2);

	await tilesOf(panel).first().click();
	const viewer = page.getByRole('dialog', { name: 'Просмотр фото' });
	await expect(viewer).toBeVisible();
	const caption = viewer.getByRole('textbox', { name: 'Подпись' });
	await caption.fill('царапина');
	await caption.press('Enter');
	await expect
		.poll(async () => {
			const rows = (await (
				await request.get(`/api/v1/orders/${o.id}/attachments`)
			).json()) as Attachment[];
			return rows.map((r) => r.caption);
		})
		.toContain('царапина');

	await viewer.getByRole('button', { name: 'Удалить', exact: true }).click();
	await expect(toastsOf(page).getByText('Фото удалено')).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(viewer).toBeHidden();
	await expect(tilesOf(panel)).toHaveCount(1);
});

test('photos stay after a reload and the card shows files', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('К') });
	await uploadPhoto(request, o.id, tinyPng(3), 'one.png');
	await uploadPhoto(request, o.id, tinyPng(4), 'two.png');
	await openCard(page, o.id);
	await expect(tilesOf(cardOf(page))).toHaveCount(2);
	await page.reload();
	await expect(tilesOf(cardOf(page))).toHaveCount(2);

	const files = cardOf(page).getByRole('region', { name: 'Файлы' });
	await files.locator('input[type=file]').setInputFiles({
		name: 'акт.pdf',
		mimeType: 'application/pdf',
		buffer: TINY_PDF
	});
	const row = files.getByRole('listitem').filter({ hasText: 'акт.pdf' });
	await expect(row).toBeVisible();
	const download = page.waitForEvent('download');
	await row.getByRole('button', { name: /^Скачать/ }).click();
	const d = await download;
	expect(d.suggestedFilename()).toBe('акт.pdf');
	const path = await d.path();
	const { readFile } = await import('node:fs/promises');
	expect((await readFile(path)).equals(TINY_PDF)).toBe(true);
});

test('a file of the old program that is gone from the server says so', async ({
	page,
	request
}) => {
	const o = await createOrder(request, { model: uniq('Л') });
	const legacy: Attachment = {
		id: 987654,
		order_id: o.id,
		stock_item_id: null,
		kind: 'file',
		name: 'старый.doc',
		caption: '',
		content_type: 'application/msword',
		size: 1024,
		width: null,
		height: null,
		legacy: true,
		missing: true,
		created_at: '2020-01-01T10:00:00'
	};
	await page.route(`**/api/v1/orders/${o.id}/attachments`, (route) =>
		route.request().method() === 'GET' ? route.fulfill({ json: [legacy] }) : route.fallback()
	);
	await openCard(page, o.id);
	const files = cardOf(page).getByRole('region', { name: 'Файлы' });
	await expect(files.getByText('старый.doc')).toBeVisible();
	await expect(files.getByText('Файл не найден')).toBeVisible();
});

test('the camera adds a photo', async ({ page, request, browserName }) => {
	test.skip(browserName !== 'chromium', 'no fake camera in WebKit');
	const m = uniq('Ц');
	const o = await createOrder(request, { model: m });
	await openPanel(page, m, o.id);
	const panel = panelOf(page, o.id);
	await panel.getByRole('button', { name: 'Добавить фото' }).click();
	await page.getByRole('menuitem', { name: 'Снять камерой' }).click();
	const dialog = page.getByRole('dialog', { name: 'Камера' });
	await expect(dialog).toBeVisible();
	await expect
		.poll(() => dialog.locator('video').evaluate((v: HTMLVideoElement) => v.readyState >= 2))
		.toBe(true);
	await dialog.getByRole('button', { name: 'Снять' }).click();
	await dialog.getByRole('button', { name: 'Готово' }).click();
	await expect(dialog).toBeHidden();
	await expect(tilesOf(panel)).toHaveCount(1);
});

test('an upload goes on after moving to the next order and tells which order it was', async ({
	page,
	request
}) => {
	const m = uniq('Ф');
	const first = await createOrder(request, { model: m });
	const second = await createOrder(request, { model: m });
	// The list is newest first: the second order is above the first.
	await openPanel(page, m, second.id);
	let release: () => void = () => {};
	const gate = new Promise<void>((r) => (release = r));
	await page.route(`**/api/v1/orders/${second.id}/attachments?*`, async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		await gate;
		await route.fallback();
	});
	await dropOf(panelOf(page, second.id))
		.locator('input[type=file]')
		.setInputFiles(png('slow.png', 9));
	await expect(panelOf(page, second.id).getByText('Загружаю…').first()).toBeVisible();
	await page.keyboard.press('Alt+ArrowDown');
	await expect(panelOf(page, first.id)).toBeVisible();
	release();
	await expect(toastsOf(page).getByText(`Добавлено к заказу № ${second.id}: 1 фото`)).toBeVisible();
	await expect(tilesOf(panelOf(page, first.id))).toHaveCount(0);
	const list = (await (
		await request.get(`/api/v1/orders/${second.id}/attachments`)
	).json()) as Attachment[];
	expect(list).toHaveLength(1);
});

/** An order with two photos, its card open: the tiles are the photos. */
async function cardWithTwoPhotos(page: Page, request: Parameters<typeof createOrder>[0]) {
	const o = await createOrder(request, { model: uniq('У') });
	await uploadPhoto(request, o.id, tinyPng(21), 'one.png');
	await uploadPhoto(request, o.id, tinyPng(22), 'two.png');
	await openCard(page, o.id);
	await expect(tilesOf(cardOf(page))).toHaveCount(2);
	return o;
}
const onServer = async (request: Parameters<typeof createOrder>[0], id: number): Promise<number> =>
	((await (await request.get(`/api/v1/orders/${id}/attachments`)).json()) as Attachment[]).length;

async function deleteFirstPhoto(page: Page) {
	await tilesOf(cardOf(page)).first().click();
	const viewer = page.getByRole('dialog', { name: 'Просмотр фото' });
	await viewer.getByRole('button', { name: 'Удалить', exact: true }).click();
	await page.keyboard.press('Escape');
	await expect(viewer).toBeHidden();
}

test('a deleted photo waits 6 s for «Отменить»; then the request goes', async ({
	page,
	request
}) => {
	await page.clock.install();
	const o = await cardWithTwoPhotos(page, request);
	await deleteFirstPhoto(page);
	await expect(tilesOf(cardOf(page))).toHaveCount(1);
	await expect(toastsOf(page).getByText('Фото удалено')).toBeVisible();
	await expect(toastsOf(page).getByRole('button', { name: 'Отменить' })).toBeVisible();
	// Still on the server while the toast is on screen.
	expect(await onServer(request, o.id)).toBe(2);
	await page.clock.fastForward(6500);
	await expect.poll(() => onServer(request, o.id)).toBe(1);
	await expect(tilesOf(cardOf(page))).toHaveCount(1);
});

test('«Отменить» brings the photo back and no DELETE is sent', async ({ page, request }) => {
	await page.clock.install();
	const deletes: string[] = [];
	page.on('request', (r) => {
		if (r.method() === 'DELETE' && r.url().includes('/attachments/')) deletes.push(r.url());
	});
	const o = await cardWithTwoPhotos(page, request);
	await deleteFirstPhoto(page);
	await expect(tilesOf(cardOf(page))).toHaveCount(1);
	await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
	await expect(tilesOf(cardOf(page))).toHaveCount(2);
	await page.clock.fastForward(10_000);
	await page.waitForTimeout(300);
	expect(deletes).toEqual([]);
	expect(await onServer(request, o.id)).toBe(2);
});

test('leaving the page sends the waiting deletion at once (keepalive)', async ({
	page,
	request
}) => {
	const o = await cardWithTwoPhotos(page, request);
	await deleteFirstPhoto(page);
	await expect(tilesOf(cardOf(page))).toHaveCount(1);
	expect(await onServer(request, o.id)).toBe(2);
	// A real page unload: `pagehide` sends the request before the 6 s are over.
	await page.goto('/orders');
	await expect.poll(() => onServer(request, o.id)).toBe(1);
});

test('repair fields precede device details and a large album can be folded', async ({
	page,
	request
}) => {
	const o = await createOrder(request, { model: uniq('Альбом') });
	for (let i = 0; i < 6; i++) await uploadPhoto(request, o.id, tinyPng(i + 1), `f${i}.png`);
	await openCard(page, o.id);
	const card = cardOf(page);
	const work = card.getByRole('heading', { name: 'Неисправность и работа' });
	const device = card.getByRole('heading', { name: 'Устройство', exact: true });
	const workBox = await work.boundingBox();
	const deviceBox = await device.boundingBox();
	expect(workBox!.y).toBeLessThan(deviceBox!.y);
	await expect(tilesOf(card)).toHaveCount(4);
	await card.getByRole('button', { name: 'Показать все фото (6)' }).click();
	await expect(tilesOf(card)).toHaveCount(6);
	await card.getByRole('button', { name: 'Свернуть фото' }).click();
	await expect(tilesOf(card)).toHaveCount(4);
});
