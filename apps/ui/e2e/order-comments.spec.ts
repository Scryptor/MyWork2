import { expect, test } from '@playwright/test';
import type { OrderCommentPage } from '$api/OrderCommentPage';
import { createOrder, patchOrder, uniq } from './support/api';

test('a note from the panel and a reply from the card keep their authors, times and the plain comment', async ({
	page,
	request
}) => {
	const model = uniq('Модель');
	const order = await createOrder(request, { model });
	await patchOrder(request, order.id, { internal_comment: 'Оставить в приёмке' });
	await page.goto(`/orders?q=${encodeURIComponent(model)}&chips=all`);
	await page.getByRole('gridcell', { name: String(order.id), exact: true }).click();
	const panel = page.getByRole('dialog', { name: new RegExp(`^№ ${order.id}(?!\\d)`) });
	const section = { name: 'Комментарии мастеров', exact: true };
	const notes = panel.getByRole('region', section);
	await notes.getByRole('textbox', { name: 'Новая запись' }).fill('Проверил разъём');
	await notes.getByRole('button', { name: 'Добавить запись' }).click();
	await expect(notes.getByRole('listitem')).toContainText('Проверил разъём');
	await panel.getByRole('button', { name: 'Открыть карточку' }).click();
	const card = page.getByRole('main');
	const feed = card.getByRole('region', section);
	await expect(feed.getByRole('listitem')).toContainText('Проверил разъём');
	await feed.getByRole('textbox', { name: 'Новая запись' }).fill('Деталь пришла');
	await feed.getByRole('button', { name: 'Добавить запись' }).click();
	await expect(feed.getByRole('listitem').first()).toContainText('Деталь пришла');
	await expect(card.getByRole('region', { name: 'История изменений' })).toContainText(
		'Добавлен комментарий к заказу'
	);
	await page.reload();
	await expect(feed.getByRole('listitem')).toHaveCount(2);
	await expect(
		card.getByRole('textbox', { name: 'Комментарий (только для сервиса)', exact: true })
	).toHaveValue('Оставить в приёмке');
	const response = await request.get(`/api/v1/orders/${order.id}/comments`);
	expect(response.ok()).toBe(true);
	const { items } = (await response.json()) as OrderCommentPage;
	for (const [index, item] of items.entries()) {
		const row = feed.getByRole('listitem').nth(index);
		expect(item.author_name).not.toBe('');
		await expect(row).toContainText(item.author_name);
		await expect(row.locator('time')).toHaveAttribute('datetime', item.created_at);
	}
});
