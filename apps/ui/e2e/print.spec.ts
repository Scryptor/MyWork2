import fs from 'node:fs/promises';
import { expect, test, type Page } from '@playwright/test';
import {
	createOrder,
	fetchOrder,
	masterId,
	patchOrder,
	setStatus,
	statusId,
	uniq,
	uniqPhone
} from './support/api';

// Printing from a browser (server of 4173: open mode, every right). A browser prints no document by
// itself: every one opens in the preview, its pages and «Сохранить PDF» with the server's file name. The
// browser's own printing of the page never opens from an order. The other specs keep the switches of
// the intake and the issue off (playwright.config.ts); here they are as on a new computer: on.
test.use({ storageState: { cookies: [], origins: [] } });

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const previewOf = (page: Page, name: string) => page.getByRole('dialog', { name, exact: true });
const rowOf = (page: Page, id: number) =>
	page
		.getByRole('grid', { name: 'Заказы' })
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });

/** The list narrowed to the marker, and the order's panel from its row. */
async function openPanel(page: Page, marker: string, id: number) {
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	await rowOf(page, id).getByRole('gridcell').nth(3).click();
	await expect(panelOf(page, id).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	return panelOf(page, id);
}

/** A page of the document has come and is drawn (not a broken picture). */
async function expectPage(preview: ReturnType<typeof previewOf>, name = 'Страница 1 из 1') {
	const img = preview.getByRole('img', { name });
	await expect(img).toBeVisible();
	await expect(img).toHaveAttribute('src', /^blob:/);
	await expect
		.poll(() => img.evaluate((el) => (el as HTMLImageElement).naturalWidth))
		.toBeGreaterThan(0);
}

/**
 * The browser's printing of the page, counted: `window.print`, and every Ctrl/⌘P the page let through
 * (not prevented: the browser would open its print dialog for it).
 */
async function watchBrowserPrinting(page: Page) {
	await page.addInitScript(() => {
		const w = window as unknown as { __printed: number; __ctrlP: boolean[] };
		w.__printed = 0;
		w.__ctrlP = [];
		window.print = () => void (w.__printed += 1);
		window.addEventListener('keydown', (e) => {
			if (e.code === 'KeyP' && (e.ctrlKey || e.metaKey))
				// After every listener of the page has had it.
				setTimeout(() => w.__ctrlP.push(e.defaultPrevented));
		});
	});
	return async () =>
		page.evaluate(() => {
			const w = window as unknown as { __printed: number; __ctrlP: boolean[] };
			return { printed: w.__printed, prevented: w.__ctrlP };
		});
}

test('«Печать ▾ → Предпросмотр…» shows the pages, «Сохранить PDF» saves the server’s file', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	const panel = await openPanel(page, m, o.id);
	await panel.getByRole('button', { name: 'Печать' }).click();
	const menu = page.getByRole('menu', { name: 'Печать' });
	await expect(menu.getByRole('menuitem')).toHaveText([
		/^\s*Акт приёма(?! по)/,
		'Акт выдачи',
		'Наклейка',
		'Предпросмотр…'
	]);
	await menu.getByRole('menuitem', { name: 'Предпросмотр…' }).click();

	const preview = previewOf(page, `Акт приёма · № ${o.id}`);
	await expectPage(preview);
	// A browser has no printer of its own: no «Печать» in the preview, the PDF is printed from a viewer.
	await expect(preview.getByRole('button', { name: 'Печать', exact: true })).toHaveCount(0);
	const saving = page.waitForEvent('download');
	await preview.getByRole('button', { name: 'Сохранить PDF' }).click();
	const pdf = await saving;
	expect(pdf.suggestedFilename()).toBe(`akt-priema-${o.id}.pdf`);
	const bytes = await fs.readFile((await pdf.path())!);
	expect(bytes.subarray(0, 5).toString()).toBe('%PDF-');

	// Another document of the order in the same preview.
	await preview.getByRole('combobox', { name: 'Документ' }).click();
	await page.getByRole('option', { name: 'Наклейка', exact: true }).click();
	await expect(preview).toBeHidden();
	await expectPage(previewOf(page, `Наклейка · № ${o.id}`));
	await page.keyboard.press('Escape');
	await expect(previewOf(page, `Наклейка · № ${o.id}`)).toBeHidden();
	// The panel is still there under it.
	await expect(panel).toBeVisible();
});

test('an issued order has the warranty acts too', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await patchOrder(request, o.id, { issued_at: '2026-11-01T12:00:00' });
	const panel = await openPanel(page, m, o.id);
	await panel.getByRole('button', { name: 'Печать' }).click();
	const menu = page.getByRole('menu', { name: 'Печать' });
	await expect(menu.getByRole('menuitem')).toHaveText([
		/^\s*Акт приёма(?! по)/,
		'Акт выдачи',
		'Наклейка',
		'Акт приёма по гарантии',
		'Акт выдачи после гарантии',
		'Предпросмотр…'
	]);
	await menu.getByRole('menuitem', { name: 'Акт выдачи после гарантии' }).click();
	await expectPage(previewOf(page, `Акт выдачи после гарантии · № ${o.id}`));
});

test('Ctrl/⌘P in the panel opens the intake act, not the browser’s printing', async ({
	page,
	request
}) => {
	const printing = await watchBrowserPrinting(page);
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	const panel = await openPanel(page, m, o.id);
	// From a field too.
	await panel.getByRole('textbox', { name: 'Неисправность' }).click();
	await page.keyboard.press(`${await mod(page)}+KeyP`);
	const preview = previewOf(page, `Акт приёма · № ${o.id}`);
	await expectPage(preview);
	// Over the preview the key does nothing more, and still not the browser’s printing.
	await page.keyboard.press(`${await mod(page)}+KeyP`);
	await expect.poll(async () => (await printing()).prevented).toEqual([true, true]);
	expect((await printing()).printed).toBe(0);
	await page.keyboard.press('Escape');
	await expect(preview).toBeHidden();
	await expect(panel).toBeVisible();
});

test('Ctrl/⌘P on the card of an issued order opens its issue act', async ({ page, request }) => {
	const printing = await watchBrowserPrinting(page);
	const o = await createOrder(request, { model: uniq('M'), master: 'Тестов' });
	await patchOrder(request, o.id, { work_done: 'Замена разъёма', final_price: 150000 });
	await setStatus(request, o.id, await statusId(request, 'Выдан'));
	await page.goto(`/orders/${o.id}`);
	await expect(page.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+KeyP`);
	await expectPage(previewOf(page, `Акт выдачи · № ${o.id}`));
	expect(await printing()).toEqual({ printed: 0, prevented: [true] });
});

test.describe('intake', () => {
	const sheetOf = (page: Page) => page.getByRole('dialog', { name: 'Новый заказ' });

	async function accept(page: Page, act: boolean) {
		await page.goto('/orders');
		await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
		await page.keyboard.press('n');
		const sheet = sheetOf(page);
		await expect(sheet.getByRole('textbox', { name: 'Телефон' })).toBeFocused();
		await sheet.getByRole('textbox', { name: 'Телефон' }).fill(uniqPhone());
		await sheet.getByRole('textbox', { name: 'ФИО' }).fill(`Тестов ${uniq('П')}`);
		await sheet.getByRole('combobox', { name: /^Тип устройства/ }).fill('Телефон');
		await sheet.getByRole('combobox', { name: /^Бренд/ }).fill('Тест');
		await sheet.getByRole('combobox', { name: /^Модель/ }).fill('Модель 1');
		await sheet.getByRole('textbox', { name: /^Неисправность/ }).fill('Не заряжается');
		const switches = sheet.getByRole('group', { name: 'Печать после приёма' });
		const actSwitch = switches.getByRole('switch', { name: 'Акт приёма' });
		await expect(actSwitch).toBeChecked();
		await expect(switches.getByRole('switch', { name: 'Наклейка' })).not.toBeChecked();
		if (!act) await actSwitch.uncheck();
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(sheet).toBeHidden();
		const accepted = page.getByRole('status', { name: 'Уведомления' }).getByText(/^Принят № \d+$/);
		await expect(accepted).toBeVisible();
		return Number((await accepted.textContent())?.replace(/\D/g, ''));
	}

	test('with «Акт приёма» on, the act opens in the preview after the order is saved', async ({
		page,
		request
	}) => {
		const id = await accept(page, true);
		await expectPage(previewOf(page, `Акт приёма · № ${id}`));
		expect((await fetchOrder(request, id)).malfunction).toBe('Не заряжается');
	});

	test('with it off nothing opens, and the switch is remembered on this computer', async ({
		page
	}) => {
		await accept(page, false);
		await expect(page.getByRole('dialog', { name: /^Акт приёма/ })).toHaveCount(0);
		const stored = await page.evaluate(() => localStorage.getItem('mw.print'));
		expect(JSON.parse(stored!)).toMatchObject({ onIntake: { act: false, sticker: false } });
		await page.keyboard.press('n');
		await expect(
			sheetOf(page)
				.getByRole('group', { name: 'Печать после приёма' })
				.getByRole('switch', { name: 'Акт приёма' })
		).not.toBeChecked();
	});
});

test('«Выдать» with «Печатать акт выдачи» opens the issue act once the order is issued', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const ready = await statusId(request, 'Готов');
	const o = await createOrder(request, { model: m, status_id: ready, master: 'Тестов' });
	await masterId(request, 'Тестов');
	await patchOrder(request, o.id, { work_done: 'Замена разъёма', final_price: 150000 });
	const panel = await openPanel(page, m, o.id);
	await panel.getByRole('button', { name: /^Выдать/ }).click();
	const dialog = page.getByRole('dialog', { name: `Выдача заказа № ${o.id}` });
	await expect(dialog.getByRole('switch', { name: 'Печатать акт выдачи' })).toBeChecked();
	await dialog.getByRole('button', { name: /^Выдать/ }).click();
	await expect(dialog).toBeHidden();
	await expectPage(previewOf(page, `Акт выдачи · № ${o.id}`));
	expect((await fetchOrder(request, o.id)).issued_at).not.toBeNull();
});

test('Ctrl K «Пустой акт приёма» opens the blank act', async ({ page }) => {
	await page.goto('/orders');
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
	await expect(palette).toBeVisible();
	await palette.getByRole('combobox').fill('пустой акт');
	await palette.getByRole('option', { name: /Пустой акт приёма/ }).click();
	await expectPage(previewOf(page, 'Пустой акт приёма'));
});

test('settings: «Образец» of every document and «Пробная наклейка» open their preview', async ({
	page
}) => {
	await page.goto('/settings/documents');
	const list = page.getByRole('list', { name: 'Документы' });
	await expect(list.getByRole('listitem')).toHaveText([
		/^Акт приёма/,
		/^Акт выдачи/,
		/^Акт приёма по гарантии/,
		/^Акт выдачи после гарантии/,
		/^Пустой акт приёма/,
		/^Наклейка/,
		/^Пробная наклейка/
	]);
	await list.getByRole('button', { name: 'Образец: Наклейка' }).click();
	const sample = previewOf(page, 'Образец · Наклейка');
	await expectPage(sample);
	await page.keyboard.press('Escape');
	await expect(sample).toBeHidden();

	await page.goto('/settings/print');
	await page.getByRole('button', { name: 'Пробная наклейка' }).click();
	await expectPage(previewOf(page, 'Пробная наклейка'));
});
