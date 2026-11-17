import { expect, test, type Page } from '@playwright/test';

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const panelOf = (page: Page) => page.getByRole('dialog', { name: /^Запись №/ });

test.describe('layers', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design');
	});

	test('slide-over keeps the page live; Esc closes it and focus returns to the opener', async ({
		page
	}) => {
		const section = page.locator('#layers');
		const opener = section.getByRole('button', { name: 'Открыть панель' });
		await opener.focus();
		await page.keyboard.press('Enter');
		const panel = panelOf(page);
		await expect(panel).toBeVisible();
		await expect(panel.getByRole('heading', { level: 2 })).toHaveText('Запись № 1');
		await expect(panel).toBeFocused();

		const counter = section.getByRole('button', { name: /^Счётчик/ });
		await counter.click();
		await counter.click();
		await expect(counter).toHaveText('Счётчик: 2');
		await expect(panel).toBeVisible();

		await panel.focus();
		await page.keyboard.press('Escape');
		await expect(panel).toBeHidden();
		await expect(opener).toBeFocused();
	});

	test('slide-over: Esc with focus moved to the page closes the panel and leaves focus there', async ({
		page
	}) => {
		const section = page.locator('#layers');
		await section.getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		await expect(panel).toBeVisible();
		const counter = section.getByRole('button', { name: /^Счётчик/ });
		await counter.focus();
		await page.keyboard.press('Escape');
		await expect(panel).toBeHidden();
		await expect(counter).toBeFocused();
	});

	test('Esc closes the select first, then the panel', async ({ page }) => {
		await page.locator('#layers').getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		await panel.getByRole('combobox', { name: 'Мастер' }).click();
		const list = page.getByRole('listbox');
		await expect(list).toBeVisible();

		await page.keyboard.press('Escape');
		await expect(list).toBeHidden();
		await expect(panel).toBeVisible();

		await page.keyboard.press('Escape');
		await expect(panel).toBeHidden();
	});

	test('slide-over with onprev/onnext: ‹ › and arrows move, arrows in a field do not, Alt+arrows do', async ({
		page
	}) => {
		await page.locator('#layers').getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		const title = panel.getByRole('heading', { level: 2 });
		await expect(title).toHaveText('Запись № 1');

		await panel.getByRole('button', { name: 'Следующая запись' }).click();
		await expect(title).toHaveText('Запись № 2');
		await panel.getByRole('button', { name: 'Предыдущая запись' }).click();
		await expect(title).toHaveText('Запись № 1');

		await panel.focus();
		await page.keyboard.press('ArrowDown');
		await expect(title).toHaveText('Запись № 2');
		await page.keyboard.press('ArrowDown');
		await expect(title).toHaveText('Запись № 3');
		await page.keyboard.press('ArrowUp');
		await expect(title).toHaveText('Запись № 2');

		const note = panel.getByRole('textbox', { name: 'Заметка' });
		await note.focus();
		await page.keyboard.press('ArrowDown');
		await expect(title).toHaveText('Запись № 2');
		await page.keyboard.press('Alt+ArrowDown');
		await expect(title).toHaveText('Запись № 3');
		await expect(note).toBeFocused();
		await page.keyboard.press('Alt+ArrowUp');
		await expect(title).toHaveText('Запись № 2');
	});

	test('slide-over: an arrow from ‹ or › onto the first or last record keeps focus in the panel', async ({
		page
	}) => {
		await page.locator('#layers').getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		const title = panel.getByRole('heading', { level: 2 });
		const prev = panel.getByRole('button', { name: 'Предыдущая запись' });
		const next = panel.getByRole('button', { name: 'Следующая запись' });
		for (let i = 0; i < 3; i++) await next.click();
		await expect(title).toHaveText('Запись № 4');

		// Alt+↓ from › onto the last record: › turns disabled, focus goes to the panel, ↑ still works.
		await next.focus();
		await page.keyboard.press('Alt+ArrowDown');
		await expect(title).toHaveText('Запись № 5');
		await expect(next).toBeDisabled();
		await expect(panel).toBeFocused();
		await page.keyboard.press('ArrowUp');
		await expect(title).toHaveText('Запись № 4');

		// ↑ from ‹ onto the first record, the same.
		for (let i = 0; i < 2; i++) await page.keyboard.press('ArrowUp');
		await expect(title).toHaveText('Запись № 2');
		await prev.focus();
		await page.keyboard.press('ArrowUp');
		await expect(title).toHaveText('Запись № 1');
		await expect(prev).toBeDisabled();
		await expect(panel).toBeFocused();
		await page.keyboard.press('ArrowDown');
		await expect(title).toHaveText('Запись № 2');
	});

	test('sheet: scrim, focus stays inside, Esc closes, Ctrl/⌘+Enter accepts', async ({ page }) => {
		const section = page.locator('#layers');
		const opener = section.getByRole('button', { name: 'Открыть широкую панель' });
		await opener.focus();
		await page.keyboard.press('Enter');
		const sheet = page.getByRole('dialog', { name: 'Новый заказ' });
		await expect(sheet).toBeVisible();

		const scrim = page.locator('.mw-scrim');
		await expect(scrim).toBeVisible();
		const [actual, expected] = await scrim.evaluate((el) => {
			const probe = document.createElement('div');
			probe.style.background = getComputedStyle(document.documentElement).getPropertyValue(
				'--scrim'
			);
			document.body.append(probe);
			const e = getComputedStyle(probe).backgroundColor;
			probe.remove();
			return [getComputedStyle(el).backgroundColor, e];
		});
		expect(actual).toBe(expected);

		// The first field takes focus; the close button is the last stop, Tab wraps both ways.
		const phone = sheet.getByRole('textbox', { name: 'Телефон' });
		const close = sheet.getByRole('button', { name: 'Закрыть' });
		await expect(phone).toBeFocused();
		await close.focus();
		await page.keyboard.press('Tab');
		await expect(phone).toBeFocused();
		await page.keyboard.press('Shift+Tab');
		await expect(close).toBeFocused();

		await page.keyboard.press('Escape');
		await expect(sheet).toBeHidden();
		await expect(opener).toBeFocused();

		await page.keyboard.press('Enter');
		await expect(sheet).toBeVisible();
		await phone.fill('70000000001');
		await page.keyboard.press('Enter');
		await expect(sheet).toBeVisible();
		await expect(section.getByText('Принято: 0')).toBeVisible();
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(sheet).toBeHidden();
		await expect(section.getByText('Принято: 1')).toBeVisible();
	});

	test('one Esc closes a panel even when a tooltip is open in it', async ({ page }) => {
		await page.locator('#layers').getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		await expect(panel).toBeVisible();
		await panel.getByRole('button', { name: 'Следующая запись' }).hover();
		const tip = page.getByRole('tooltip', { name: 'Следующая запись' });
		await expect(tip).toBeVisible();

		await page.keyboard.press('Escape');
		await expect(panel).toBeHidden();
		await expect(tip).toBeHidden();
	});

	test('toast undo is clickable while a dialog is open', async ({ page }) => {
		await page.locator('#layers').getByRole('button', { name: 'Открыть диалог' }).click();
		const dialog = page.getByRole('dialog', { name: 'Комментарий к заказу' });
		const comment = dialog.getByRole('textbox', { name: 'Комментарий' });
		await comment.fill('Перезвонить после обеда');
		await dialog.getByRole('button', { name: 'Очистить' }).click();
		await expect(comment).toHaveValue('');

		await page
			.getByRole('status', { name: 'Уведомления' })
			.getByRole('button', { name: 'Отменить' })
			.click();
		await expect(dialog.getByText('Отменено: 1')).toBeVisible();
		await expect(comment).toHaveValue('Перезвонить после обеда');
		await expect(dialog).toBeVisible();
		await expect
			.poll(() => dialog.evaluate((el) => el.contains(document.activeElement)))
			.toBe(true);
		// The dialog still has the keyboard: Esc closes it.
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
	});

	test('dialog returns focus to main when its opener is gone', async ({ page }) => {
		// With the mouse: a click focuses the button in Chromium; WebKit leaves focus on <main> around it.
		const section = page.locator('#layers');
		const opener = section.getByRole('button', { name: 'Черновик заказа' });
		const dialog = page.getByRole('dialog', { name: 'Черновик заказа' });
		await opener.click();
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'Удалить черновик' }).click();
		await expect(dialog).toBeHidden();
		await expect(opener).toHaveCount(0);
		await expect(page.getByRole('main', { name: 'Дизайн-система' })).toBeFocused();

		// From the keyboard, the same.
		await section.getByRole('button', { name: 'Вернуть черновик' }).click();
		await opener.focus();
		await page.keyboard.press('Enter');
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'Удалить черновик' }).focus();
		await page.keyboard.press('Enter');
		await expect(dialog).toBeHidden();
		await expect(page.getByRole('main', { name: 'Дизайн-система' })).toBeFocused();
	});

	test('dialog returns focus to its opener, opened with the mouse or the keyboard', async ({
		page
	}) => {
		const opener = page.locator('#layers').getByRole('button', { name: 'Черновик заказа' });
		const dialog = page.getByRole('dialog', { name: 'Черновик заказа' });
		await opener.click();
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'Закрыть' }).first().click();
		await expect(dialog).toBeHidden();
		await expect(opener).toBeFocused();

		await opener.click();
		await expect(dialog).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
		await expect(opener).toBeFocused();

		await opener.focus();
		await page.keyboard.press('Enter');
		await expect(dialog).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(dialog).toBeHidden();
		await expect(opener).toBeFocused();
	});

	test('confirm dialog: the confirm button is busy while onconfirm waits, then the dialog closes', async ({
		page
	}) => {
		const section = page.locator('#layers');
		await section.getByRole('button', { name: 'Удалить навсегда' }).click();
		const dialog = page.getByRole('alertdialog', { name: 'Удалить заказ № 10481 навсегда' });
		await expect(dialog).toBeVisible();
		await dialog.getByRole('button', { name: 'Удалить', exact: true }).click();
		await expect(dialog.locator('button[aria-busy="true"]')).toHaveCount(1);
		await expect(dialog).toBeHidden();
		await expect(section.getByText('Удалено: 1')).toBeVisible();
	});

	test('status toast: «Отменить» restores the status, and so does Ctrl/⌘+Z outside a field', async ({
		page
	}) => {
		const section = page.locator('#layers');
		const status = section.getByText(/^Статус заказа № 10481:/);
		const change = section.getByRole('button', { name: 'Статус → Готов' });
		const toasts = page.getByRole('status', { name: 'Уведомления' });
		await expect(status).toContainText('Диагностика');

		await change.click();
		await expect(status).toContainText('Готов');
		await expect(toasts).toContainText('№ 10481 → Готов');
		await toasts.getByRole('button', { name: 'Отменить' }).click();
		await expect(status).toContainText('Диагностика');
		await expect(toasts.getByRole('button', { name: 'Отменить' })).toHaveCount(0);

		await change.click();
		await expect(status).toContainText('Готов');
		const key = `${await mod(page)}+z`;
		await page.getByRole('textbox', { name: 'Обычное' }).focus();
		await page.keyboard.press(key);
		await expect(status).toContainText('Готов');
		await change.focus();
		await page.keyboard.press(key);
		await expect(status).toContainText('Диагностика');
		await expect(toasts.getByRole('button', { name: 'Отменить' })).toHaveCount(0);
	});

	test('dropdown menu: items with a disabled one, a pick, Esc closes', async ({ page }) => {
		const section = page.locator('#layers');
		const trigger = section.getByRole('button', { name: 'Действия' });
		await trigger.click();
		const menu = page.getByRole('menu');
		await expect(menu).toBeVisible();
		await expect(menu.getByRole('menuitem', { name: 'Вернуть в работу' })).toHaveAttribute(
			'aria-disabled',
			'true'
		);
		await menu.getByRole('menuitem', { name: 'Дублировать' }).click();
		await expect(menu).toBeHidden();
		await expect(section.getByText('Выбрано: Дублировать')).toBeVisible();

		await trigger.click();
		await expect(menu).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(menu).toBeHidden();
	});

	test('popover opens from its trigger and Esc closes it', async ({ page }) => {
		const trigger = page.locator('#layers').getByRole('button', { name: 'Подробнее' });
		await trigger.click();
		const text = page.getByText('Гарантия 30 дней с даты выдачи.');
		await expect(text).toBeVisible();
		// A named popover is a dialog with that name.
		await expect(page.getByRole('dialog', { name: 'Гарантия' })).toContainText('Гарантия 30 дней');
		await page.keyboard.press('Escape');
		await expect(text).toBeHidden();
	});

	test('an icon button is a popover trigger and keeps its tooltip: «Колонки»', async ({ page }) => {
		await page.goto('/orders');
		await expect(page.getByRole('grid', { name: 'Заказы' })).toBeVisible();
		const trigger = page.getByRole('button', { name: 'Колонки' });
		await trigger.click();
		await expect(trigger).toHaveAttribute('aria-expanded', 'true');
		await expect(page.getByRole('checkbox', { name: 'Статус' }).first()).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(trigger).toHaveAttribute('aria-expanded', 'false');
		await trigger.hover();
		await expect(page.getByRole('tooltip', { name: 'Колонки' })).toBeVisible();
	});
});

test.describe('toaster', () => {
	test('the region is polite and not atomic: a new toast is read alone, not with the others', async ({
		page
	}) => {
		await page.goto('/design');
		const toasts = page.getByRole('status', { name: 'Уведомления' });
		await expect(toasts).toHaveAttribute('aria-live', 'polite');
		await expect(toasts).toHaveAttribute('aria-atomic', 'false');
	});

	test('a toast waits while hovered or focused, then leaves 6 s later', async ({ page }) => {
		await page.clock.install();
		await page.goto('/design');
		const section = page.locator('#layers');
		const toasts = page.getByRole('status', { name: 'Уведомления' });
		await section.getByRole('button', { name: 'Успех' }).click();
		const saved = toasts.getByText('Сохранено');
		await expect(saved).toBeVisible();

		await saved.hover();
		await page.clock.fastForward(10_000);
		await expect(saved).toBeVisible();

		await page.mouse.move(0, 0);
		await toasts.getByRole('button', { name: 'Закрыть' }).focus();
		await page.clock.fastForward(10_000);
		await expect(saved).toBeVisible();

		await section.getByRole('button', { name: 'Успех' }).focus();
		await page.clock.fastForward(6_000);
		await expect(saved).toBeHidden();
	});
});

test.describe('phone width', () => {
	test.use({ viewport: { width: 390, height: 844 } });

	test('the slide-over and a dialog fill the screen; the panel has «Назад»', async ({ page }) => {
		await page.goto('/design');
		const section = page.locator('#layers');
		const full = { x: 0, y: 0, width: 390, height: 844 };

		await section.getByRole('button', { name: 'Открыть панель' }).click();
		const panel = panelOf(page);
		await expect(panel).toBeVisible();
		await expect.poll(() => panel.boundingBox()).toEqual(full);
		await expect(panel.getByRole('button', { name: 'Закрыть' })).toHaveCount(0);
		await panel.getByRole('button', { name: 'Назад' }).click();
		await expect(panel).toBeHidden();

		await section.getByRole('button', { name: 'Открыть диалог' }).click();
		const dialog = page.getByRole('dialog', { name: 'Комментарий к заказу' });
		await expect(dialog).toBeVisible();
		await expect.poll(() => dialog.boundingBox()).toEqual(full);
	});
});
