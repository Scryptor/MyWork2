import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import type { ServicePoint } from '$api/ServicePoint';
import type { Status } from '$api/Status';
import { createOrder, uniq } from './support/api';

// «Настройки → Статусы» and «Точки». Statuses and points are global on the shared servers and other specs
// rely on the built-in ones: only statuses made here are touched, and they go to the archive at the end
// (so the picker of the order panel does not grow for the other specs).

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

const rowOf = (page: Page, text: string) =>
	page.getByRole('main').getByRole('listitem').filter({ hasText: text });

async function openStatuses(page: Page) {
	await page.goto('/settings/statuses');
	await expect(page.getByRole('heading', { name: 'Статусы' })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Название нового статуса' })).toBeVisible();
}

async function addStatus(page: Page, name: string, hue?: string) {
	await page.getByRole('textbox', { name: 'Название нового статуса' }).fill(name);
	if (hue) {
		await page
			.getByRole('button', { name: /^Цвет „.*“: / })
			.first()
			.click();
		// A click, not `check()`: the choice closes the palette, and `check()` would wait for the radio it
		// can no longer find to show the new state.
		const palette = page.getByRole('dialog', { name: 'Цвет' });
		await palette.getByRole('radio', { name: hue }).click();
		await expect(palette).toBeHidden();
		await expect(
			page.getByRole('button', { name: new RegExp(`^Цвет „.*“: ${hue.toLowerCase()}$`) }).first()
		).toBeVisible();
	}
	await page.getByRole('button', { name: 'Добавить статус' }).click();
	await expect(rowOf(page, name)).toBeVisible();
}

async function archiveAll(request: APIRequestContext, ids: string[]) {
	const all = (await (await request.get('/api/v1/statuses')).json()) as Status[];
	for (const s of all.filter((x) => ids.some((id) => x.name.includes(id)))) {
		await request.patch(`/api/v1/statuses/${s.id}`, { data: { archived: true } });
	}
}

test('the colour palette: arrows move the focus and keep it open, Enter chooses, Esc closes unchanged', async ({
	page
}) => {
	await openStatuses(page);
	const button = page.getByRole('button', { name: /^Цвет „.*“: / }).first();
	const before = await button.getAttribute('aria-label');
	await button.click();
	const palette = page.getByRole('dialog', { name: 'Цвет' });
	await expect(palette).toBeVisible();
	await palette.getByRole('radio', { checked: true }).focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('ArrowLeft');
	await expect(palette).toBeVisible();
	await expect(button).toHaveAttribute('aria-label', before!);
	await page.keyboard.press('Escape');
	await expect(palette).toBeHidden();
	await expect(button).toHaveAttribute('aria-label', before!);

	await button.click();
	await expect(palette).toBeVisible();
	await palette.getByRole('radio', { checked: true }).focus();
	await page.keyboard.press('ArrowRight');
	await page.keyboard.press('Enter');
	await expect(palette).toBeHidden();
	await expect(button).not.toHaveAttribute('aria-label', before!);
});

test('a new status shows in the filters at once; a change of kind warns and Cancel keeps it', async ({
	page,
	request
}) => {
	const id = uniq();
	const name = `Ждёт клиента-${id}`;
	try {
		await openStatuses(page);
		await addStatus(page, name, 'Синий');
		await expect(
			rowOf(page, name).getByRole('button', { name: /^Цвет „.*“: синий$/ })
		).toBeVisible();
		await expect(rowOf(page, name)).toContainText('заказ в работе');

		// No reload: «Заказы» by the menu, the filter has the status.
		await page
			.getByRole('navigation', { name: 'Разделы' })
			.locator('a[href="/orders"]')
			.first()
			.click();
		await page.getByRole('button', { name: 'Фильтры' }).click();
		await expect(page.getByRole('checkbox', { name })).toBeVisible();
		await page.keyboard.press('Escape');

		// An order in the status through the API: the count, then the warning.
		const status = ((await (await request.get('/api/v1/statuses')).json()) as Status[]).find(
			(s) => s.name === name
		)!;
		await createOrder(request, { status_id: status.id });
		await openStatuses(page);
		await expect(rowOf(page, name)).toContainText('в 1 заказе');

		const kind = rowOf(page, name).getByRole('combobox', { name: /^Вид статуса/ });
		await kind.click();
		await page.getByRole('option', { name: 'Выдан' }).click();
		const dialog = page.getByRole('alertdialog', { name: `Сменить вид статуса „${name}“?` });
		await expect(dialog).toContainText('1 заказ станет выданным и попадёт в отчёты');
		await dialog.getByRole('button', { name: 'Отмена' }).click();
		await expect(dialog).toBeHidden();
		await expect(kind).toHaveText('Открыт');

		// Archived: it leaves the filters, the row goes under «Архив».
		await rowOf(page, name).getByRole('button', { name: 'В архив' }).click();
		await expect(page.getByRole('heading', { name: 'Архив' })).toBeVisible();
		await expect(rowOf(page, name).getByRole('button', { name: 'Вернуть' })).toBeVisible();
	} finally {
		await archiveAll(request, [id]);
	}
});

test('an archived status and point keep their orders: «В работе», the badge, the point name', async ({
	page,
	request
}) => {
	const id = uniq();
	const statusName = `Ждёт запчасть-${id}`;
	const pointName = `Склад-${id}`;
	const status = (await (
		await request.post('/api/v1/statuses', {
			data: { name: statusName, kind: 'open', color: 'indigo' }
		})
	).json()) as Status;
	const point = (await (
		await request.post('/api/v1/service-points', { data: { name: pointName } })
	).json()) as ServicePoint;
	const model = uniq('M');
	const order = await createOrder(request, {
		status_id: status.id,
		service_point_id: point.id,
		model
	});
	try {
		await request.patch(`/api/v1/statuses/${status.id}`, { data: { archived: true } });
		await request.patch(`/api/v1/service-points/${point.id}`, { data: { archived: true } });

		// «В работе» (the default view) still lists the order, with the badge of its status.
		await page.goto(`/orders?q=${encodeURIComponent(model)}`);
		const grid = page.getByRole('grid', { name: 'Заказы' });
		const row = grid.getByRole('row').filter({ hasText: model });
		await expect(row).toHaveCount(1);
		await expect(row).toContainText(statusName);

		// The card names the point and shows the status as the current one.
		await page.goto(`/orders/${order.id}`);
		const card = page.getByRole('main');
		await expect(card.getByText(pointName, { exact: true })).toBeVisible();
		// A label of its own, or «Ещё» showing it when the picker has no room for it.
		const current = card
			.getByRole('radio', { name: statusName, exact: true })
			.or(card.getByRole('button', { name: `Ещё: ${statusName}` }));
		await expect(current).toBeVisible();
	} finally {
		await archiveAll(request, [id]);
		await request.patch(`/api/v1/service-points/${point.id}`, { data: { archived: true } });
	}
});

test('without the counts a change of kind still warns, and no count is shown', async ({
	page,
	request
}) => {
	const id = uniq();
	const name = `Без счёта-${id}`;
	try {
		await request.post('/api/v1/statuses', { data: { name, kind: 'open', color: 'gray' } });
		await page.route('**/api/v1/statuses/usage', (route) => route.abort('internetdisconnected'));
		await openStatuses(page);
		await expect(rowOf(page, name)).toBeVisible();
		await expect(rowOf(page, name)).not.toContainText(/в \d+ заказ/);

		const kind = rowOf(page, name).getByRole('combobox', { name: /^Вид статуса/ });
		await kind.click();
		await page.getByRole('option', { name: 'Выдан' }).click();
		const dialog = page.getByRole('alertdialog', { name: `Сменить вид статуса „${name}“?` });
		await expect(dialog).toContainText(
			'Не удалось узнать, сколько заказов в статусе. Все они сменят вид на „Выдан“'
		);
		await dialog.getByRole('button', { name: 'Отмена' }).click();
		await expect(dialog).toBeHidden();
		await expect(kind).toHaveText('Открыт');
	} finally {
		await archiveAll(request, [id]);
	}
});

test('the last «Выдан» cannot lose its kind', async ({ page }) => {
	await openStatuses(page);
	const kinds = page.getByRole('main').getByRole('combobox', { name: /^Вид статуса/ });
	// The list is loaded before it is counted.
	await expect(kinds.first()).toBeVisible();
	const issued = page.getByRole('main').getByRole('listitem').filter({ hasText: 'заказ выдан' });
	// Only if exactly one live status is «Выдан» does the refusal show; otherwise the change is just allowed.
	test.skip((await issued.count()) !== 1, 'more than one «Выдан» status on this server');
	const one = issued.getByRole('combobox', { name: /^Вид статуса/ });
	await one.click();
	await page.getByRole('option', { name: 'Открыт' }).click();
	// Orders issued by other specs: the warning comes first, then the server refuses; with none, only the refusal.
	const warn = page.getByRole('alertdialog');
	const refusal = page.getByText('Нужен хотя бы один статус вида „Выдан“');
	await expect(warn.or(refusal)).toBeVisible();
	if (await warn.isVisible()) await warn.getByRole('button', { name: 'Сменить вид' }).click();
	await expect(refusal).toBeVisible();
	await expect(one).toHaveText('Выдан');
});

test('dragging changes the order of the statuses and it survives a reload', async ({
	page,
	request
}) => {
	const id = uniq();
	try {
		await openStatuses(page);
		await addStatus(page, `Первый-${id}`);
		await addStatus(page, `Второй-${id}`);
		const rows = page.getByRole('main').getByRole('listitem').filter({ hasText: id });
		await expect(rows.first()).toContainText(`Первый-${id}`);
		await rows.last().scrollIntoViewIfNeeded();
		const handle = rows.last().getByRole('button', { name: `Переместить „Второй-${id}“` });
		const from = await handle.boundingBox();
		const to = await rows.first().boundingBox();
		if (!from || !to) throw new Error('no boxes');
		// The list shows the new order at once; the reload waits for the server to have taken it (a refusal
		// because another spec added a status meanwhile is sent again).
		const saved = page.waitForResponse(
			(r) => r.url().endsWith('/api/v1/statuses/order') && r.request().method() === 'PUT' && r.ok(),
			{ timeout: 10_000 }
		);
		await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
		await page.mouse.down();
		await page.mouse.move(from.x + from.width / 2, to.y + 2, { steps: 8 });
		await page.mouse.up();
		await expect(rows.first()).toContainText(`Второй-${id}`);
		await saved;

		await page.reload();
		await expect(rows.first()).toContainText(`Второй-${id}`);
		await expect(rows.last()).toContainText(`Первый-${id}`);

		// The pickers follow: the filters list them in the new order.
		await page
			.getByRole('navigation', { name: 'Разделы' })
			.locator('a[href="/orders"]')
			.first()
			.click();
		await page.getByRole('button', { name: 'Фильтры' }).click();
		const checks = page.getByRole('checkbox', { name: new RegExp(`-${id}$`) });
		await expect(checks).toHaveCount(2);
		await expect(checks.first()).toHaveAccessibleName(`Второй-${id}`);
	} finally {
		await archiveAll(request, [id]);
	}
});

test('a point is added, archived and brought back', async ({ page }) => {
	const id = uniq();
	const name = `Филиал-${id}`;
	await page.goto('/settings/points');
	await expect(page.getByRole('heading', { name: 'Точки' })).toBeVisible();
	await expect(page.getByRole('link', { name: 'Точка по умолчанию — в „Заказах“' })).toBeVisible();
	const field = page.getByRole('textbox', { name: 'Название новой точки' });
	await field.fill(name);
	await field.press('Enter');
	await expect(rowOf(page, name)).toContainText('в 0 заказах');
	await expect(rowOf(page, name).getByRole('combobox')).toHaveCount(0);

	await rowOf(page, name).getByRole('button', { name: 'В архив' }).click();
	await rowOf(page, name).getByRole('button', { name: 'Вернуть' }).click();
	await expect(rowOf(page, name).getByRole('button', { name: 'В архив' })).toBeVisible();
	await rowOf(page, name).getByRole('button', { name: 'В архив' }).click();
	await expect(rowOf(page, name).getByRole('button', { name: 'Вернуть' })).toBeVisible();
});
