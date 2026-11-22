import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import type { AppSettings } from '$api/AppSettings';
import { uniq } from './support/api';

// «Настройки → Документы и реквизиты» are the server's: the firm's phone, details and legal details and
// the terms of the acts, saved as they are typed. «Настройки → Печать» is this computer's: in a browser
// it explains that printing goes through a PDF and keeps the label size in the browser's storage.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

const FIELDS = {
	firm_phone: 'Телефон сервиса',
	firm_details: 'Адрес и часы работы',
	firm_legal_details: 'Реквизиты',
	intake_terms: 'Условия приёма',
	issue_terms: 'Условия выдачи'
} as const;
type Field = keyof typeof FIELDS;

async function currentSettings(request: APIRequestContext): Promise<AppSettings> {
	const response = await request.get('/api/v1/settings');
	expect(response.ok()).toBe(true);
	return (await response.json()) as AppSettings;
}

const fieldOf = (page: Page, f: Field) =>
	page.getByRole('textbox', { name: FIELDS[f], exact: true });

// The server's settings are shared by every test on the server: what is changed here is put back.
test('the details and the terms are saved as they are typed and are there after a reload', async ({
	page,
	request
}) => {
	const before = await currentSettings(request);
	const mark = uniq('Реквизиты');
	const values: Record<Field, string> = {
		firm_phone: '+7 900 000-00-01',
		firm_details: `${mark}\nг. Энск, ул. Мастеров, 5\nЕжедневно 10–20`,
		firm_legal_details: `ИП ${mark}, ИНН 000000000000`,
		intake_terms: `${mark}: устройство принято без вскрытия.\nСрок ремонта — до 45 дней.`,
		issue_terms: `${mark}: устройство получено, претензий нет.`
	};
	try {
		await page.goto('/settings/documents');
		await expect(page.getByRole('heading', { name: 'Документы и реквизиты' })).toBeVisible();
		await expect(
			page
				.getByRole('navigation', { name: 'Разделы настроек' })
				.getByRole('link', { name: 'Документы и реквизиты' })
		).toHaveAttribute('aria-current', 'page');
		for (const f of Object.keys(FIELDS) as Field[]) {
			await expect(fieldOf(page, f)).toBeEnabled();
			await fieldOf(page, f).fill(values[f]);
		}
		// The last one is saved when it is left, without waiting for the pause.
		await fieldOf(page, 'issue_terms').blur();
		await expect
			.poll(async () => {
				const s = await currentSettings(request);
				return Object.fromEntries((Object.keys(FIELDS) as Field[]).map((f) => [f, s[f]]));
			})
			.toEqual(values);
		await expect(page.getByRole('status').filter({ hasText: 'Сохранено' })).toBeVisible();

		await page.reload();
		for (const f of Object.keys(FIELDS) as Field[]) {
			await expect(fieldOf(page, f)).toHaveValue(values[f]);
		}
	} finally {
		const restore = Object.fromEntries((Object.keys(FIELDS) as Field[]).map((f) => [f, before[f]]));
		expect((await request.patch('/api/v1/settings', { data: restore })).ok()).toBe(true);
	}
});

test('«Название сервиса» and the currency say where they are printed', async ({ page }) => {
	await page.goto('/settings/general');
	await page.getByRole('button', { name: 'Что такое «Название сервиса»' }).click();
	const hint = page.getByRole('dialog', { name: 'Название сервиса', exact: true });
	await expect(hint).toContainText('в шапке актов');
	await expect(hint).not.toContainText('только хранится');
	await page.keyboard.press('Escape');
	await page.getByRole('button', { name: 'Что такое «Валюта»' }).click();
	const currency = page.getByRole('dialog', { name: 'Валюта', exact: true });
	await expect(currency).toContainText('«руб.»');
	await expect(currency).not.toContainText('только хранится');
});

test.describe('«Печать» in a browser', () => {
	const labelSize = (page: Page) => page.getByRole('radiogroup', { name: 'Размер наклейки' });

	test('says that printing goes through a PDF and has no printers to choose', async ({ page }) => {
		await page.goto('/settings/print');
		await expect(page.getByRole('heading', { name: 'Печать', exact: true })).toBeVisible();
		await expect(page.getByText('Печать из браузера — через PDF')).toBeVisible();
		await expect(page.getByText('Только на этом компьютере')).toBeVisible();
		await expect(page.getByRole('combobox', { name: 'Принтер актов' })).toHaveCount(0);
		await expect(page.getByRole('combobox', { name: 'Принтер наклеек' })).toHaveCount(0);
		await expect(labelSize(page).getByRole('radio', { name: '58 × 40 мм' })).toBeChecked();
	});

	test('keeps the label size on this computer, «Свой» too', async ({ page }) => {
		await page.goto('/settings/print');
		await labelSize(page).getByRole('radio', { name: '40 × 30 мм' }).check({ force: true });
		await page.reload();
		await expect(labelSize(page).getByRole('radio', { name: '40 × 30 мм' })).toBeChecked();

		await labelSize(page).getByRole('radio', { name: 'Свой' }).check({ force: true });
		const width = page.getByRole('textbox', { name: 'Ширина, мм' });
		const height = page.getByRole('textbox', { name: 'Высота, мм' });
		await expect(width).toHaveValue('40');
		await width.fill('70');
		await height.fill('50,5');
		// More than a label can be: said under the field and not kept.
		await width.fill('250');
		await width.blur();
		await expect(page.getByText('От 15 до 200 мм')).toBeVisible();
		await page.reload();
		await expect(labelSize(page).getByRole('radio', { name: 'Свой' })).toBeChecked();
		await expect(page.getByRole('textbox', { name: 'Ширина, мм' })).toHaveValue('70');
		await expect(page.getByRole('textbox', { name: 'Высота, мм' })).toHaveValue('50,5');
		const stored = await page.evaluate(() => localStorage.getItem('mw.print'));
		expect(JSON.parse(stored!)).toMatchObject({ labelMm: [70, 50.5] });
	});
});
