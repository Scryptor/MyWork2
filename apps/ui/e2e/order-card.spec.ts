import { expect, test, type Page } from '@playwright/test';
import {
	createOrder,
	deleteOrder,
	fetchOrder,
	patchOrder,
	setStatus,
	statusId,
	uniq
} from './support/api';

// The full order card on the server of 4173 (open mode: every permission). Each test makes its own
// orders with a unique model.

async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, id: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const cardOf = (page: Page) => page.getByRole('main');
const regionOf = (page: Page, name: string) => cardOf(page).getByRole('region', { name });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

/** Opens the card at its address and waits for the order's fields. */
async function openCard(page: Page, id: number) {
	await page.goto(`/orders/${id}`);
	await expect(cardOf(page).getByRole('heading', { name: `№ ${id}`, level: 1 })).toBeVisible();
	await expect(cardOf(page).getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
}

test('full card opens from the panel and shows status history', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, {
		model: m,
		status_id: await statusId(request, 'Диагностика')
	});
	await setStatus(request, o.id, await statusId(request, 'Готов'));

	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
	const panel = panelOf(page, o.id);
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await panel.getByRole('button', { name: 'Открыть карточку' }).click();

	await expect(page).toHaveURL(new RegExp(`/orders/${o.id}$`));
	await expect(panel).toBeHidden();
	const card = cardOf(page);
	await expect(card.getByRole('heading', { name: `№ ${o.id}`, level: 1 })).toBeVisible();
	await expect(card.getByText(`Тест ${m}`)).toBeVisible();
	const timeline = regionOf(page, 'Статусы и сроки');
	await expect(timeline).toContainText('Статус: «Диагностика» → «Готов»');
	await expect(timeline).toContainText(`Заказ № ${o.id} принят`);
	await expect(
		card.getByRole('radiogroup', { name: 'Статус' }).getByRole('radio', { name: 'Готов' })
	).toBeChecked();
	await expect(card.getByRole('button', { name: /^Выдать/ })).toBeVisible();

	// Not issued: nothing to return to work or to take under warranty, so no ⋯.
	await expect(card.getByRole('button', { name: 'Ещё действия' })).toHaveCount(0);

	// A status picked in the card goes to the timeline.
	await card.getByRole('radio', { name: 'Ждёт запчасть' }).check();
	await expect(toastsOf(page)).toContainText(`№ ${o.id} → Ждёт запчасть`);
	await expect(timeline).toContainText('Статус: «Готов» → «Ждёт запчасть»');

	// «←» goes back to the list it came from.
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(page).toHaveURL(new RegExp(`/orders\\?q=${m}`));
	await expect(rowOf(page, o.id)).toContainText('Ждёт запчасть');
});

test('without a previous page «←» leads to the orders list', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	await cardOf(page).getByRole('button', { name: 'Назад' }).click();
	await expect(page).toHaveURL(/\/orders$/);
});

test('warranty case on an issued order', async ({ page, request }) => {
	const m = uniq('M');
	const o = await createOrder(request, {
		model: m,
		status_id: await statusId(request, 'Готов'),
		master: 'Тестов'
	});
	const filled = await patchOrder(request, o.id, { work_done: 'Чистка', final_price: 100000 });
	const issuedResponse = await request.post(
		`/api/v1/orders/${o.id}/issue?version=${filled.version}`,
		{
			headers: { 'Content-Type': 'application/json' }
		}
	);
	expect(issuedResponse.ok(), await issuedResponse.text()).toBe(true);
	const issuedAt = (await fetchOrder(request, o.id)).issued_at;
	expect(issuedAt).not.toBeNull();

	// The panel offers it too.
	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
	const panel = panelOf(page, o.id);
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await panel.getByRole('button', { name: 'Ещё действия' }).click();
	await expect(page.getByRole('menuitem', { name: 'Принять по гарантии…' })).toBeVisible();
	await page.keyboard.press('Escape');
	await page.keyboard.press('Escape');
	await expect(panel).toBeHidden();

	await openCard(page, o.id);
	const card = cardOf(page);
	await card.getByRole('button', { name: 'Ещё действия' }).click();
	await page.getByRole('menuitem', { name: 'Принять по гарантии…' }).click();
	const dialog = page.getByRole('dialog', { name: `Приём заказа № ${o.id} по гарантии` });
	await expect(dialog).toBeVisible();
	// The complaint is required.
	await dialog.getByRole('button', { name: /^Принять/ }).click();
	await expect(dialog.getByRole('textbox', { name: /^Жалоба/ })).toHaveAttribute(
		'aria-invalid',
		'true'
	);
	await dialog.getByRole('textbox', { name: /^Жалоба/ }).fill('Снова не заряжается');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(dialog).toBeHidden();

	await expect(
		card
			.getByRole('radiogroup', { name: 'Статус' })
			.getByRole('radio', { name: 'Принят по гарантии' })
	).toBeChecked();
	await expect(regionOf(page, 'Гарантийные случаи')).toContainText('Снова не заряжается');
	await expect(regionOf(page, 'Статусы и сроки')).toContainText(
		'Принят по гарантии: Снова не заряжается'
	);
	const after = await fetchOrder(request, o.id);
	expect(after.status_id).toBe(await statusId(request, 'Принят по гарантии'));
	expect(after.issued_at).toBe(issuedAt);
});

test('the card edits the device and opens from Ctrl K with Ctrl/⌘+Enter', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const o = await createOrder(request, {
		model: m,
		serial: '356789104512345',
		completeness: 'Зарядка',
		device_condition: 'Царапины на корпусе'
	});
	await page.goto('/orders');
	await expect(gridOf(page)).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+KeyK`);
	const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
	await palette.getByRole('combobox').fill(String(o.id));
	await expect(palette.getByRole('option').first()).toContainText(`№ ${o.id}`);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(page).toHaveURL(new RegExp(`/orders/${o.id}$`));

	const card = cardOf(page);
	const model = card.getByRole('textbox', { name: 'Модель' });
	await expect(model).toHaveValue(m);
	await model.fill(`${m}-Pro`);
	await card.getByRole('textbox', { name: 'IMEI/серийный' }).fill('356789104512346');
	await card.getByRole('combobox', { name: 'Цвет' }).fill('Синий');
	const completeness = card.getByRole('textbox', { name: 'Комплектность' });
	await expect(completeness).toHaveValue('Зарядка');
	await completeness.fill('Зарядка, чехол');
	const condition = card.getByRole('textbox', { name: 'Внешнее состояние' });
	await expect(condition).toHaveValue('Царапины на корпусе');
	await condition.fill('Царапины, скол угла');
	await card.getByRole('textbox', { name: 'Неисправность' }).click();
	await expect(card.getByText('✓ Сохранено')).toBeVisible();
	await expect
		.poll(async () => {
			const after = await fetchOrder(request, o.id);
			return [after.model, after.serial, after.color, after.completeness, after.device_condition];
		})
		.toEqual([`${m}-Pro`, '356789104512346', 'Синий', 'Зарядка, чехол', 'Царапины, скол угла']);
	// The header shows the new model.
	await expect(card.getByText(`Тест ${m}-Pro`)).toBeVisible();

	// The panel keeps the device as text.
	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
	const panel = panelOf(page, o.id);
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await expect(panel.getByText(`Смартфон · Тест · ${m}-Pro · Синий`)).toBeVisible();
	await expect(panel.getByText('Комплектность: Зарядка, чехол')).toBeVisible();
	await expect(panel.getByText('Внешнее состояние: Царапины, скол угла')).toBeVisible();
	await expect(panel.getByRole('textbox', { name: 'Модель' })).toHaveCount(0);
	await expect(panel.getByRole('textbox', { name: 'Комплектность' })).toHaveCount(0);
});

test('the card sets the pattern key with «Готово»; closing without it changes nothing', async ({
	page,
	request
}) => {
	const o = await createOrder(request, { model: uniq('M'), device_pattern: '1596' });
	await openCard(page, o.id);
	const field = cardOf(page).getByRole('button', { name: 'Графический ключ', exact: true });
	await expect(field).toHaveText('Задан, 4 точки');
	const popover = page.getByRole('dialog', { name: 'Графический ключ', exact: true });
	const point = (n: number) => popover.getByRole('button', { name: `Точка ${n}`, exact: true });

	// Esc drops what was pressed.
	await field.click();
	await expect(popover).toContainText('1 · 5 · 9 · 6');
	await popover.getByRole('button', { name: 'Сбросить' }).click();
	await point(3).click();
	await page.keyboard.press('Escape');
	await expect(popover).toBeHidden();
	await expect(field).toHaveText('Задан, 4 точки');

	await field.click();
	await popover.getByRole('button', { name: 'Сбросить' }).click();
	for (const n of [7, 5, 3, 6, 9]) await point(n).click();
	await popover.getByRole('button', { name: 'Готово' }).click();
	await expect(popover).toBeHidden();
	await expect(field).toHaveText('Задан, 5 точек');
	await expect.poll(async () => (await fetchOrder(request, o.id)).device_pattern).toBe('75369');
	// The history says what changed, not the pattern.
	const history = regionOf(page, 'История изменений');
	await expect(history).toContainText('Изменено: графический ключ');
	await expect(history).not.toContainText('75369');
});

/** PATCH requests of one order. */
const patchesOf = (id: number) => (url: URL) => url.pathname === `/api/v1/orders/${id}`;

test('leaving the card right after typing saves the text first', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	// A slow server: leaving must wait for the answer.
	await page.route(patchesOf(o.id), async (route) => {
		if (route.request().method() === 'PATCH') await new Promise((r) => setTimeout(r, 700));
		await route.continue();
	});
	const card = cardOf(page);
	await card.getByRole('textbox', { name: 'Выполненные работы' }).fill('Чистка платы');
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	expect((await fetchOrder(request, o.id)).work_done).toBe('Чистка платы');
});

test('a save that fails keeps the user on the card', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	const patches = patchesOf(o.id);
	await page.route(patches, (route) =>
		route.request().method() === 'PATCH' ? route.abort('internetdisconnected') : route.continue()
	);
	const card = cardOf(page);
	const work = card.getByRole('textbox', { name: 'Выполненные работы' });
	await work.fill('Замена кнопки');
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(toastsOf(page)).toContainText(`Изменения № ${o.id} не сохранены`);
	await expect(page).toHaveURL(new RegExp(`/orders/${o.id}$`));
	await expect(card.getByText('Не сохранено')).toBeVisible();
	await expect(card.getByRole('button', { name: 'Повторить' })).toBeVisible();

	// A link of the sidebar waits for the save as well.
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: /^Заказы/ })
		.click();
	await expect(toastsOf(page).getByText(`Изменения № ${o.id} не сохранены`)).toHaveCount(2);
	await expect(card.getByText('Не сохранено')).toBeVisible();
	await expect(page).toHaveURL(new RegExp(`/orders/${o.id}$`));
	await expect(work).toHaveValue('Замена кнопки');

	await page.unroute(patches);
	await card.getByRole('button', { name: 'Повторить' }).click();
	await expect(card.getByText('✓ Сохранено')).toBeVisible();
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	expect((await fetchOrder(request, o.id)).work_done).toBe('Замена кнопки');
});

test('an order deleted elsewhere: own edits can be dropped and the card left', async ({
	page,
	request
}) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	await deleteOrder(request, o.id);
	const card = cardOf(page);
	const work = card.getByRole('textbox', { name: 'Выполненные работы' });
	await work.fill('Замена кнопки');
	await expect(card.getByText('Не сохранено: заказ удалён')).toBeVisible();
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(toastsOf(page)).toContainText(`Изменения № ${o.id} не сохранены: заказ удалён`);
	await expect(page).toHaveURL(new RegExp(`/orders/${o.id}$`));

	await card.getByRole('button', { name: 'Отбросить мои правки' }).click();
	await expect(work).toHaveValue('');
	await card.getByRole('button', { name: 'Назад' }).click();
	await expect(page).toHaveURL(/\/orders$/);
});

test('the order of the card does not open in the panel over it', async ({ page, request }) => {
	const [shown, other] = [
		await createOrder(request, { model: uniq('M') }),
		await createOrder(request, { model: uniq('M') })
	];
	await openCard(page, shown.id);
	async function find(id: number) {
		await page.keyboard.press(`${await mod(page)}+KeyK`);
		const palette = page.getByRole('dialog', { name: 'Найти или выполнить' });
		await palette.getByRole('combobox').fill(String(id));
		await expect(palette.getByRole('option').first()).toContainText(`№ ${id}`);
		await page.keyboard.press('Enter');
		await expect(palette).toBeHidden();
	}
	await find(other.id);
	await expect(panelOf(page, other.id)).toBeVisible();
	// The card's own order: the panel makes way for the card instead of a second editor.
	await find(shown.id);
	await expect(panelOf(page, other.id)).toBeHidden();
	await expect(panelOf(page, shown.id)).toHaveCount(0);
	await expect(
		cardOf(page).getByRole('heading', { name: `№ ${shown.id}`, level: 1 })
	).toBeVisible();
});

test('closing the tab with unsaved text asks first', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	await page.route(patchesOf(o.id), (route) =>
		route.request().method() === 'PATCH' ? route.abort('internetdisconnected') : route.continue()
	);
	const card = cardOf(page);
	await card.getByRole('textbox', { name: 'Выполненные работы' }).fill('Замена кнопки');
	await expect(card.getByText('Не сохранено')).toBeVisible();
	const asked = page.waitForEvent('dialog');
	await page.close({ runBeforeUnload: true });
	const dialog = await asked;
	expect(dialog.type()).toBe('beforeunload');
	await dialog.dismiss();
	expect(page.isClosed()).toBe(false);
});

test('the whole history comes by pages of 50', async ({ page, request }) => {
	const o = await createOrder(request, { model: uniq('M') });
	for (let i = 1; i <= 55; i++)
		await patchOrder(request, o.id, { internal_comment: `Запись ${i}` });
	await openCard(page, o.id);
	const history = regionOf(page, 'История изменений');
	await expect(history.getByRole('listitem')).toHaveCount(50);
	await history.getByRole('button', { name: 'Показать ещё' }).click();
	// 55 edits and the intake.
	await expect(history.getByRole('listitem')).toHaveCount(56);
	await expect(history.getByRole('button', { name: 'Показать ещё' })).toHaveCount(0);
});

test('crossing 1024px does not rebuild the card: the focus stays in the field', async ({
	page,
	request
}) => {
	await page.setViewportSize({ width: 1200, height: 800 });
	const o = await createOrder(request, { model: uniq('M') });
	await openCard(page, o.id);
	const field = cardOf(page).getByRole('textbox', { name: 'Неисправность' });
	await field.fill('Не включается');
	await expect(field).toBeFocused();
	const handle = await field.elementHandle();
	await page.setViewportSize({ width: 900, height: 800 });
	await expect(cardOf(page).getByRole('tablist')).toBeVisible();
	await expect(field).toBeFocused();
	expect(await handle!.evaluate((el) => el.isConnected)).toBe(true);
	await expect(field).toHaveValue('Не включается');
	await page.setViewportSize({ width: 1200, height: 800 });
	await expect(cardOf(page).getByRole('tablist')).toHaveCount(0);
	await expect(field).toBeFocused();
	expect(await handle!.evaluate((el) => el.isConnected)).toBe(true);
});

test.describe('narrower than 1024px', () => {
	test.use({ viewport: { width: 900, height: 800 } });

	test('narrow card uses tabs', async ({ page, request }) => {
		const o = await createOrder(request, { model: uniq('M') });
		await openCard(page, o.id);
		const tabs = cardOf(page).getByRole('tablist');
		await expect(tabs.getByRole('tab', { name: 'Заказ' })).toHaveAttribute('aria-selected', 'true');
		await expect(regionOf(page, 'Статусы и сроки')).toBeHidden();
		await tabs.getByRole('tab', { name: 'История' }).click();
		await expect(regionOf(page, 'Статусы и сроки')).toContainText(`Заказ № ${o.id} принят`);
		await expect(cardOf(page).getByRole('textbox', { name: 'Неисправность' })).toBeHidden();
	});

	test('the tabs have panels named by their tab', async ({ page, request }) => {
		const o = await createOrder(request, { model: uniq('M') });
		await openCard(page, o.id);
		const card = cardOf(page);
		const names = ['Заказ', 'История', 'Файлы'];
		for (const active of names) {
			await card.getByRole('tab', { name: active }).click();
			await expect(card.getByRole('tabpanel', { name: active })).toBeVisible();
			for (const other of names.filter((n) => n !== active)) {
				await expect(card.getByRole('tabpanel', { name: other })).toHaveCount(0);
			}
			// Every tab points at an existing panel.
			for (const name of names) {
				const controls = await card.getByRole('tab', { name }).getAttribute('aria-controls');
				expect(controls).toBeTruthy();
				expect(await page.locator(`[id="${controls}"]`).count()).toBe(1);
			}
		}
	});
});
