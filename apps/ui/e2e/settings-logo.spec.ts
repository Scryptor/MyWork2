import { expect, test as base } from '@playwright/test';
import { ownOrigin } from './support/fresh';

// The firm's logo («Настройки → Документы и реквизиты»): one per base, shared by every act. So the spec has
// a server of its own per engine (the two engines run at the same time) and puts the base back as it was.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(ownOrigin('logo', browserName))
});

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

// A 1 × 1 PNG, as bytes of the test itself.
const PNG = Buffer.from(
	'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
	'base64'
);

test.afterEach(async ({ request }) => {
	const response = await request.delete('/api/v1/settings/logo', {
		headers: { 'Content-Type': 'application/json' }
	});
	expect([204, 404]).toContain(response.status());
});

test('uploads_and_removes_logo', async ({ page }) => {
	await page.goto('/settings/documents');
	await expect(page.getByRole('heading', { name: 'Логотип' })).toBeVisible();
	await expect(page.getByText('Логотип не загружен')).toBeVisible();

	await page.getByLabel('Файл логотипа').setInputFiles({
		name: 'logo.png',
		mimeType: 'image/png',
		buffer: PNG
	});
	const logo = page.getByRole('img', { name: 'Логотип сервиса' });
	await expect(logo).toBeVisible();
	await expect(logo).toHaveAttribute('src', /^blob:/);
	await expect
		.poll(() => logo.evaluate((el) => (el as HTMLImageElement).naturalWidth))
		.toBeGreaterThan(0);

	// The sample of the act is drawn with the logo.
	await page.getByRole('button', { name: 'Образец: Акт приёма', exact: true }).click();
	const preview = page.getByRole('dialog', { name: 'Образец · Акт приёма', exact: true });
	const sheet = preview.getByRole('img', { name: /^Страница 1 из \d+$/ });
	await expect(sheet).toBeVisible();
	await expect
		.poll(() => sheet.evaluate((el) => (el as HTMLImageElement).naturalWidth))
		.toBeGreaterThan(0);
	await page.keyboard.press('Escape');
	await expect(preview).toBeHidden();

	await page.getByRole('button', { name: 'Удалить', exact: true }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: 'Удалить', exact: true }).click();
	await expect(page.getByText('Логотип не загружен')).toBeVisible();
	await expect(logo).toHaveCount(0);
});

test('a file that is not a picture is refused with the server’s text', async ({ page }) => {
	await page.goto('/settings/documents');
	await page.getByLabel('Файл логотипа').setInputFiles({
		name: 'logo.txt',
		mimeType: 'text/plain',
		buffer: Buffer.from('not a picture')
	});
	await expect(page.getByRole('alert')).toBeVisible();
	await expect(page.getByText('Логотип не загружен')).toBeVisible();
});
