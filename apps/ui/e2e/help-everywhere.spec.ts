import { expect, test, type Locator, type Page } from '@playwright/test';
import type { ServerInfo } from '$api/ServerInfo';
import { createClient, createOrder, fetchOrder, patchOrder, statusId, uniq } from './support/api';

// «Что это?» on every screen where a field or an action needs words (plan 5.7, task 5): on each screen one
// hint opens, says its point and closes with Esc; inside a sheet, a panel or a dialog Esc closes only the
// hint. Server of 4173: open mode, every right; each test makes its own orders and clients.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

/** Opens the hint `name` inside `scope`, checks its words, closes it with Esc: focus is back on the icon. */
async function expectHint(page: Page, scope: Locator | Page, name: string, words: string) {
	const icon = scope.getByRole('button', { name: `Что такое «${name}»` });
	await icon.click();
	const hint = page.getByRole('dialog', { name, exact: true });
	await expect(hint).toBeVisible();
	await expect(hint).toContainText(words);
	await page.keyboard.press('Escape');
	await expect(hint).toBeHidden();
	await expect(icon).toBeFocused();
}

const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });

test('the intake explains the IMEI lookup, the chips and the money', async ({ page }) => {
	await page.goto('/orders');
	await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
	await page.keyboard.press('n');
	const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
	await expect(sheet.getByRole('textbox', { name: 'Телефон' })).toBeFocused();

	await expectHint(page, sheet, 'IMEI или серийный номер', 'Этот IMEI уже был');
	// Esc closed the hint only: the sheet and what is typed stay.
	await expect(sheet).toBeVisible();
	await expectHint(page, sheet, 'Комплектность', '+ своё');
	await expectHint(page, sheet, 'Частые неисправности', 'Неисправности');
	for (const name of ['Пометка', 'Статус', 'Гарантия', 'Оценка от', 'Предоплата']) {
		await expect(sheet.getByRole('button', { name: `Что такое «${name}»` })).toBeVisible();
	}
	await expect(sheet).toBeVisible();
});

test('the order panel explains the money and the marks', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, { model: m });
	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await page
		.getByRole('grid', { name: 'Заказы' })
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(o.id), exact: true }) })
		.getByRole('gridcell')
		.nth(3)
		.click();
	const panel = panelOf(page, o.id);
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();

	await expectHint(page, panel, 'Расходы', '«К оплате» не входит');
	await expect(panel).toBeVisible();
	for (const name of ['Итог', 'Срок гарантии', 'Позвонить', 'Заказать запчасть']) {
		await expect(panel.getByRole('button', { name: `Что такое «${name}»` })).toBeVisible();
	}
});

test('the card of an issued order explains its warranty cases and the complaint', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const o = await createOrder(request, {
		model: m,
		status_id: await statusId(request, 'Готов'),
		master: 'Тестов'
	});
	const filled = await patchOrder(request, o.id, { work_done: 'Чистка', final_price: 100000 });
	const issued = await request.post(`/api/v1/orders/${o.id}/issue?version=${filled.version}`, {
		headers: { 'Content-Type': 'application/json' }
	});
	expect(issued.ok(), await issued.text()).toBe(true);
	expect((await fetchOrder(request, o.id)).issued_at).not.toBeNull();

	await page.goto(`/orders/${o.id}`);
	const card = page.getByRole('main');
	await expect(card.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await expectHint(page, card, 'Гарантийные случаи', 'Принять по гарантии');

	await card.getByRole('button', { name: 'Ещё действия' }).click();
	await page.getByRole('menuitem', { name: 'Принять по гарантии…' }).click();
	const dialog = page.getByRole('dialog', { name: `Приём заказа № ${o.id} по гарантии` });
	await expect(dialog).toBeVisible();
	await expectHint(page, dialog, 'Жалоба', 'Принят по гарантии');
	await expect(dialog).toBeVisible();
});

test('the orders list explains its quick filters, the sidebar its saved views', async ({
	page
}) => {
	await page.addInitScript(() => {
		if (localStorage.getItem('mw.views') === null)
			localStorage.setItem('mw.views', JSON.stringify([{ name: 'Готовые', search: 'chips=all' }]));
	});
	await page.goto('/orders');
	await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
	await expectHint(page, page.getByRole('main'), 'Быстрые фильтры', 'Ctrl');
	const nav = page.getByRole('navigation', { name: 'Разделы' });
	await expectHint(page, nav, 'Мои виды', 'Сохранить вид');
});

test('«Клиенты» explains «Похожие», the client panel the black list', async ({ page, request }) => {
	const name = `Клиентова ${uniq('k')}`;
	const c = await createClient(request, { full_name: name });
	await page.goto('/clients');
	await expectHint(page, page.getByRole('main'), 'Похожие', 'последние 10 цифр');

	await page.goto(`/clients?client=${c.id}`);
	const panel = page.getByRole('dialog', { name });
	await expect(panel.getByRole('textbox', { name: 'ФИО' })).toHaveValue(name);
	await expectHint(page, panel, 'Чёрный список', 'ничего не запрещает');
	await expect(panel).toBeVisible();
	for (const hint of ['Телефоны', 'Заметка']) {
		await expect(panel.getByRole('button', { name: `Что такое «${hint}»` })).toBeVisible();
	}
});

test('«Склад» explains «Докупить»', async ({ page }) => {
	await page.goto('/stock');
	await expect(page.getByRole('grid', { name: 'Склад' })).toBeVisible();
	await expectHint(page, page.getByRole('main'), 'Докупить', 'порог');
});

test('the settings explain the density, the replacement, the statuses and the data folder', async ({
	page
}) => {
	await page.goto('/settings/appearance');
	await expectHint(page, page.getByRole('main'), 'Плотность таблиц', '28');

	await page.goto('/settings/import');
	await expect(page.getByRole('button', { name: /^Перенести/ })).toBeVisible();
	await expectHint(page, page.getByRole('main'), 'Перенос с заменой', 'backups');

	await page.goto('/settings/statuses');
	await expectHint(page, page.getByRole('main'), 'Статусы', 'Принят по гарантии');

	await page.goto('/settings/about');
	await expect(page.getByRole('button', { name: 'Скопировать' })).toBeVisible();
	await expectHint(page, page.getByRole('main'), 'Папка данных', 'attachments');
});

test('«Проверять обязательные поля» says brand and model are the ones it lets go', async ({
	page
}) => {
	await page.goto('/settings/orders');
	await page
		.getByRole('button', { name: 'Что такое «Проверять обязательные поля при приёме»' })
		.click();
	const hint = page.getByRole('dialog', { name: 'Проверять обязательные поля при приёме' });
	await expect(hint).toContainText('бренд и модель можно не заполнять');
	await expect(hint).toContainText('Тип устройства, неисправность');
	await page.keyboard.press('Escape');
	await expect(hint).toBeHidden();
});

test('the first run explains the choice and the first point', async ({ page }) => {
	// The server of 4173 is set up: it is asked to say it is not, as the layout sweep does.
	await page.route('**/api/v1/server-info', async (route) => {
		const response = await route.fetch();
		const body = (await response.json()) as ServerInfo;
		await route.fulfill({ response, json: { ...body, setup_required: true } });
	});
	await page.goto('/setup');
	await expect(page.getByRole('heading', { name: 'Первый запуск' })).toBeVisible();
	await expectHint(page, page, 'С чего начать', 'без замены');
	await page.getByRole('button', { name: 'Начать с пустой базы' }).click();
	await expect(page.getByRole('textbox', { name: /^Первая точка/ })).toBeVisible();
	await expectHint(page, page, 'Первая точка', 'по умолчанию');
});

for (const path of [
	'/orders',
	'/clients',
	'/stock',
	'/settings/statuses',
	'/settings/import',
	'/settings/about'
] as const) {
	test(`${path}: the hints do not widen the page at 360 px and at 150 %`, async ({ page }) => {
		await page.addInitScript(() =>
			localStorage.setItem('mw.prefs', JSON.stringify({ v: 2, scale: 1.5 }))
		);
		for (const width of [360, 1280]) {
			await page.setViewportSize({ width, height: 800 });
			await page.goto(path);
			await expect(page.getByRole('button', { name: /^Что такое/ }).first()).toBeVisible();
			const fits = await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth
			);
			expect(fits, `${path} at ${width}`).toBe(true);
		}
	});
}

test.describe('a touch screen', () => {
	test.use({
		hasTouch: true,
		isMobile: true,
		screen: { width: 1024, height: 768 },
		viewport: { width: 1024, height: 768 }
	});

	test('the 44px icon of a hint does not push its field down: fields side by side stay level', async ({
		page
	}) => {
		await page.goto('/orders');
		expect(await page.evaluate(() => matchMedia('(pointer: coarse)').matches)).toBe(true);
		await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
		await page.keyboard.press('n');
		const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
		await expect(sheet.getByRole('textbox', { name: 'Телефон' })).toBeFocused();
		const top = async (control: Locator) => {
			await control.scrollIntoViewIfNeeded();
			return (await control.boundingBox())!.y;
		};
		// «Мастер» has no hint and «Статус» next to it has one; «Оценка от» has one, «Оценка до» has not.
		const pairs: [Locator, Locator][] = [
			[
				sheet.getByRole('combobox', { name: 'Мастер' }),
				sheet.getByRole('combobox', { name: 'Статус' })
			],
			[
				sheet.getByRole('textbox', { name: 'Оценка от' }),
				sheet.getByRole('textbox', { name: 'Оценка до' })
			]
		];
		for (const [a, b] of pairs) {
			const [ya, yb] = [await top(a), await top(b)];
			expect(Math.abs(ya - yb), `${ya} / ${yb}`).toBeLessThanOrEqual(1);
		}
		// The icon is still a whole finger target, and the field under it still takes a tap at its top.
		const icon = sheet.getByRole('button', { name: 'Что такое «Статус»' });
		const box = (await icon.boundingBox())!;
		expect(Math.min(box.width, box.height)).toBeGreaterThanOrEqual(44);
		const status = sheet.getByRole('combobox', { name: 'Статус' });
		const field = (await status.boundingBox())!;
		// The point is under both: the icon reaches over the top of the field, and the field is on top.
		expect(box.y + box.height).toBeGreaterThan(field.y + 2);
		const tapped = await status.evaluate(
			(el, [x, y]) => {
				const hit = document.elementFromPoint(x!, y!);
				return hit !== null && el.contains(hit);
			},
			[box.x + box.width / 2, field.y + 2]
		);
		expect(tapped, 'a tap at the top of «Статус» goes to the field').toBe(true);
	});
});
