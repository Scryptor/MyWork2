import { expect, test, type Page } from '@playwright/test';
import { createOrder, fetchOrder, patchOrder, setStatus, statusId, uniq } from './support/api';

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
	const filled = await patchOrder(request, o.id, { work_done: 'Чистка' });
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
	const o = await createOrder(request, { model: m, serial: '356789104512345' });
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
	await card.getByRole('textbox', { name: 'Неисправность' }).click();
	await expect(card.getByText('✓ Сохранено')).toBeVisible();
	await expect
		.poll(async () => {
			const after = await fetchOrder(request, o.id);
			return [after.model, after.serial, after.color];
		})
		.toEqual([`${m}-Pro`, '356789104512346', 'Синий']);
	// The header shows the new model.
	await expect(card.getByText(`Тест ${m}-Pro`)).toBeVisible();

	// The panel keeps the device as text.
	await page.goto(`/orders?q=${encodeURIComponent(m)}&chips=all`);
	await rowOf(page, o.id).getByRole('gridcell').nth(3).click();
	const panel = panelOf(page, o.id);
	await expect(panel.getByRole('textbox', { name: 'Неисправность' })).toBeVisible();
	await expect(panel.getByText(`Смартфон · Тест · ${m}-Pro · Синий`)).toBeVisible();
	await expect(panel.getByRole('textbox', { name: 'Модель' })).toHaveCount(0);
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
});
