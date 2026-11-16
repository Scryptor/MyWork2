import { expect, test, type Page } from '@playwright/test';

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const sheetOf = (page: Page) => page.getByRole('dialog', { name: 'Новый заказ' });
const phoneOf = (page: Page) => sheetOf(page).getByRole('textbox', { name: 'Телефон' });
const typeOf = (page: Page) => sheetOf(page).getByRole('combobox', { name: /^Тип устройства/ });
const malfunctionOf = (page: Page) =>
	sheetOf(page).getByRole('textbox', { name: /^Неисправность/ });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });

test.describe('intake demo', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design/intake');
		await expect(sheetOf(page)).toBeVisible();
	});

	test('opens the wide panel «Новый заказ» with «Телефон» focused', async ({ page }) => {
		await expect(phoneOf(page)).toBeFocused();
		// min(70rem, 100vw − 4rem) at 1280px: 70rem.
		await expect.poll(async () => (await sheetOf(page).boundingBox())?.width).toBe(1120);
	});

	test('an empty form: Ctrl/⌘+Enter does not accept, the summary lists what is missing, fields show errors', async ({
		page
	}) => {
		const sheet = sheetOf(page);
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(sheet).toBeVisible();
		await expect(
			sheet.getByText('Не заполнено: Телефон или ФИО, Тип устройства, Неисправность')
		).toBeVisible();
		await expect(phoneOf(page)).toHaveAttribute('aria-invalid', 'true');
		await expect(typeOf(page)).toHaveAttribute('aria-invalid', 'true');
		await expect(malfunctionOf(page)).toHaveAttribute('aria-invalid', 'true');
		await expect(toastsOf(page)).not.toContainText('принят');
	});

	test('the phone 8 000 000-00-01 shows the card of the found fictional client', async ({
		page
	}) => {
		await phoneOf(page).pressSequentially('8 000 000-00-01');
		const card = sheetOf(page).getByRole('region', { name: 'Найденный клиент' });
		await expect(card).toBeVisible();
		await expect(card).toContainText('Елкин Пётр Сергеевич');
		await expect(card).toContainText('+7 000 000-00-01');
		await expect(card).toContainText(/\d+ заказ/);
		await expect(sheetOf(page).getByRole('textbox', { name: 'ФИО' })).toHaveCount(0);
	});

	test('a click on the chip «Чехол» in «Комплектность» marks it', async ({ page }) => {
		const chip = sheetOf(page)
			.getByRole('group', { name: 'Комплектность' })
			.getByRole('button', { name: 'Чехол' });
		await expect(chip).toHaveAttribute('aria-pressed', 'false');
		await chip.click();
		await expect(chip).toHaveAttribute('aria-pressed', 'true');
	});

	test('with the required fields filled, Ctrl/⌘+Enter accepts: the panel closes, «Заказ № … принят»', async ({
		page
	}) => {
		await phoneOf(page).fill('70000000001');
		await typeOf(page).fill('Телефон');
		await malfunctionOf(page).fill('Не включается');
		await expect(sheetOf(page).getByText(/^Не заполнено/)).toHaveCount(0);
		await page.keyboard.press(`${await mod(page)}+Enter`);
		await expect(sheetOf(page)).toBeHidden();
		const toasts = toastsOf(page);
		await expect(toasts).toContainText(/Заказ № \d+ принят/);
		// The page is usable again: the toast can be closed.
		await toasts.getByRole('button', { name: 'Закрыть' }).click();
		await expect(toasts).not.toContainText('принят');

		await page.getByRole('button', { name: 'Новый заказ' }).click();
		await expect(malfunctionOf(page)).toHaveValue('');
	});

	test('a draft: Esc closes the panel, opening it again restores what was typed, also after a reload', async ({
		page
	}) => {
		await phoneOf(page).fill('70001234567');
		await malfunctionOf(page).fill('Разбит экран');
		await page.keyboard.press('Escape');
		await expect(sheetOf(page)).toBeHidden();

		await page.getByRole('button', { name: 'Новый заказ' }).click();
		await expect(sheetOf(page)).toBeVisible();
		await expect(malfunctionOf(page)).toHaveValue('Разбит экран');
		await expect(phoneOf(page)).toHaveValue('+7 000 123-45-67');

		await page.reload();
		await expect(sheetOf(page)).toBeVisible();
		await expect(malfunctionOf(page)).toHaveValue('Разбит экран');
		await expect(phoneOf(page)).toHaveValue('+7 000 123-45-67');
	});
});
