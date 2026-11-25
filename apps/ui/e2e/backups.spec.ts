import { expect, test as base } from '@playwright/test';
import { ownOrigin } from './support/fresh';

// «Настройки → Резервные копии» on a server of its own per engine (the copies are the base's own): a copy is
// made, restoring is requested and cancelled, a copy is deleted.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(ownOrigin('backups', browserName))
});

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

test('makes_restores_and_deletes_a_copy', async ({ page }) => {
	await page.goto('/settings');
	await page.getByRole('link', { name: 'Резервные копии' }).click();
	await expect(page.getByRole('heading', { name: 'Резервные копии', level: 1 })).toBeVisible();
	await expect(page.getByText('Копий пока нет')).toBeVisible();

	await page.getByRole('button', { name: 'Сделать копию сейчас' }).click();
	await expect(page.getByText('Копия сохранена')).toBeVisible();
	const list = page.getByRole('list', { name: 'Копии' });
	await expect(list.getByRole('listitem')).toHaveCount(1);
	await expect(list.getByText('Вручную')).toBeVisible();

	await list.getByRole('button', { name: /^Восстановить копию от/ }).click();
	const dialog = page.getByRole('alertdialog');
	await expect(dialog.getByText('Восстановить данные из копии?')).toBeVisible();
	await dialog.getByRole('button', { name: 'Восстановить', exact: true }).click();
	await expect(dialog).toBeHidden();
	// The callout's own text: the closing dialog says «при следующем запуске» too.
	const pending = page.getByText(/^Данные заменятся копией от .* при следующем запуске программы/);
	await expect(pending).toBeVisible();
	await expect(
		page.getByText('Перезапустите сервер mw-server, чтобы восстановить данные.')
	).toBeVisible();

	await page.getByRole('button', { name: 'Отменить восстановление' }).click();
	await expect(pending).toHaveCount(0);

	await list.getByRole('button', { name: /^Удалить копию от/ }).click();
	const confirm = page.getByRole('alertdialog');
	await confirm.getByRole('button', { name: 'Удалить', exact: true }).click();
	await expect(confirm).toBeHidden();
	await expect(page.getByText('Вручную')).toHaveCount(0);
	await expect(page.getByText('Копий пока нет')).toBeVisible();
});
