import { expect, test, type Page } from '@playwright/test';
import {
	addDictionaryItem,
	createOrder,
	fetchOrder,
	patchOrder,
	statusId,
	uniq
} from './support/api';

// Bulk actions of the selection bar on the server of 4173 (open mode: every permission). Each test
// makes its own orders with a unique model and selects them in the list narrowed to that model.

const gridOf = (page: Page) => page.getByRole('grid', { name: 'Заказы' });
const rowOf = (page: Page, id: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });
const barOf = (page: Page) => page.getByRole('region', { name: 'Выбранные строки' });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

/** Opens the list narrowed to the marker (all statuses) and selects every row of it. */
async function selectAll(page: Page, marker: string, ids: number[]) {
	await page.goto(`/orders?q=${encodeURIComponent(marker)}&chips=all`);
	for (const id of ids) await expect(rowOf(page, id)).toBeVisible();
	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await expect(barOf(page)).toContainText(`Выбрано ${ids.length}`);
}

/** An order ready to be issued: price, master and work done are filled in. */
async function readyOrder(request: Parameters<typeof createOrder>[0], model: string) {
	const o = await createOrder(request, {
		model,
		status_id: await statusId(request, 'Готов'),
		master: 'Тестов'
	});
	return patchOrder(request, o.id, { work_done: 'Чистка', final_price: 100000 });
}

test('bulk issue without date leaves the issue date empty', async ({ page, request }) => {
	const m = uniq('M');
	const issued = await statusId(request, 'Выдан');
	const a = await readyOrder(request, m);
	const b = await readyOrder(request, m);
	await selectAll(page, m, [a.id, b.id]);

	await barOf(page).getByRole('button', { name: 'Статус' }).click();
	await page.getByRole('menuitem', { name: 'Выдан' }).click();
	const dialog = page.getByRole('dialog', { name: 'Дата выдачи' });
	await expect(dialog).toBeVisible();
	await dialog.getByRole('button', { name: 'Не ставить' }).click();
	await expect(dialog).toBeHidden();

	await expect(toastsOf(page)).toContainText('Статус «Выдан» у 2 заказов');
	await expect(barOf(page)).toBeHidden();
	await expect(rowOf(page, a.id)).toContainText('Выдан');
	await expect(rowOf(page, b.id)).toContainText('Выдан');
	for (const o of [a, b]) {
		const after = await fetchOrder(request, o.id);
		expect(after.status_id).toBe(issued);
		expect(after.issued_at).toBeNull();
	}
});

test('bulk issue now stamps the date and reports an order it could not issue', async ({
	page,
	request
}) => {
	const m = uniq('M');
	const ready = await statusId(request, 'Готов');
	const issued = await statusId(request, 'Выдан');
	const good = await readyOrder(request, m);
	// No price, master and work done: the server does not issue it.
	const bad = await createOrder(request, { model: m, status_id: ready });
	await selectAll(page, m, [good.id, bad.id]);

	await barOf(page).getByRole('button', { name: 'Статус' }).click();
	await page.getByRole('menuitem', { name: 'Выдан' }).click();
	await page
		.getByRole('dialog', { name: 'Дата выдачи' })
		.getByRole('button', { name: 'Сейчас' })
		.click();

	await expect(toastsOf(page)).toContainText('Статус «Выдан» у 1 заказа');
	await expect(toastsOf(page)).toContainText(
		`Не удалось у 1 заказа: № ${bad.id} — заполните: итоговая цена, мастер, выполненные работы`
	);
	const done = await fetchOrder(request, good.id);
	expect(done.status_id).toBe(issued);
	expect(done.issued_at).not.toBeNull();
	expect((await fetchOrder(request, bad.id)).status_id).toBe(ready);

	// Undo brings the issued one back and takes the date away again.
	await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
	await expect
		.poll(async () => {
			const back = await fetchOrder(request, good.id);
			return [back.status_id, back.issued_at];
		})
		.toEqual([ready, null]);
});

test('bulk master change and undo', async ({ page, request }) => {
	const m = uniq('M');
	const master = uniq('Мастер');
	await addDictionaryItem(request, 'master', master);
	const a = await createOrder(request, { model: m, master: 'Антон' });
	const b = await createOrder(request, { model: m });
	await selectAll(page, m, [a.id, b.id]);

	await barOf(page).getByRole('button', { name: 'Мастер' }).click();
	await page.getByRole('menuitem', { name: master }).click();
	await expect(toastsOf(page)).toContainText(`Мастер «${master}» у 2 заказов`);
	await expect(rowOf(page, a.id)).toContainText(master);
	await expect(rowOf(page, b.id)).toContainText(master);
	expect((await fetchOrder(request, a.id)).master).toBe(master);
	expect((await fetchOrder(request, b.id)).master).toBe(master);

	await toastsOf(page).getByRole('button', { name: 'Отменить' }).click();
	await expect
		.poll(async () => [
			(await fetchOrder(request, a.id)).master,
			(await fetchOrder(request, b.id)).master
		])
		.toEqual(['Антон', '']);
	await expect(rowOf(page, a.id)).toContainText('Антон');

	// «Снять мастера» clears it.
	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await barOf(page).getByRole('button', { name: 'Мастер' }).click();
	await page.getByRole('menuitem', { name: 'Снять мастера' }).click();
	await expect(toastsOf(page)).toContainText('Мастер снят у 2 заказов');
	expect((await fetchOrder(request, a.id)).master).toBe('');
});

test('bulk marks are set and cleared', async ({ page, request }) => {
	const m = uniq('M');
	const a = await createOrder(request, { model: m });
	const b = await createOrder(request, { model: m, need_call: true });
	await selectAll(page, m, [a.id, b.id]);

	await barOf(page).getByRole('button', { name: 'Отметки' }).click();
	await page.getByRole('menuitem', { name: 'Позвонить: поставить' }).click();
	await expect(toastsOf(page)).toContainText('Отметка «Позвонить» у 2 заказов');
	expect((await fetchOrder(request, a.id)).need_call).toBe(true);
	expect((await fetchOrder(request, b.id)).need_call).toBe(true);

	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await barOf(page).getByRole('button', { name: 'Отметки' }).click();
	await page.getByRole('menuitem', { name: 'Заказать запчасть: поставить' }).click();
	await expect(toastsOf(page)).toContainText('Отметка «Заказать запчасть» у 2 заказов');
	expect((await fetchOrder(request, a.id)).need_parts).toBe(true);

	await gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' }).click();
	await barOf(page).getByRole('button', { name: 'Отметки' }).click();
	await page.getByRole('menuitem', { name: 'Позвонить: снять' }).click();
	await expect(toastsOf(page)).toContainText('Отметка «Позвонить» снята у 2 заказов');
	expect((await fetchOrder(request, a.id)).need_call).toBe(false);
	expect((await fetchOrder(request, b.id)).need_call).toBe(false);
});
