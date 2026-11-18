import { expect, test } from '@playwright/test';

const CAMERA_TEXTS = [
	'Нет доступа к камере. Разрешите его в настройках системы или добавьте фото файлами',
	'Камера недоступна — добавьте фото файлами',
	'Камера не запустилась — добавьте фото файлами'
];

test.describe('media', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design');
	});

	test('camera shoots twice in a row', async ({ page, browserName }) => {
		test.skip(browserName !== 'chromium', 'no fake camera in WebKit');
		await page.addInitScript(() => {
			const w = window as unknown as { __gum: number };
			w.__gum = 0;
			const md = navigator.mediaDevices;
			const original = md.getUserMedia.bind(md);
			md.getUserMedia = (c) => {
				w.__gum += 1;
				return original(c);
			};
		});
		await page.reload();
		const section = page.locator('#media');
		await section.getByRole('button', { name: 'Открыть камеру' }).click();
		const dialog = page.getByRole('dialog', { name: 'Камера' });
		await expect(dialog).toBeVisible();
		const video = dialog.locator('video');
		await expect
			.poll(() => video.evaluate((v: HTMLVideoElement) => v.readyState >= 2 && !v.paused))
			.toBe(true);
		await dialog.getByRole('button', { name: 'Снять' }).click();
		await expect(section.getByText('Снято: 1')).toBeVisible();
		// Focus off the button, so Space goes through the dialog's own handler.
		await dialog.evaluate((el) => (el as HTMLElement).focus());
		await page.keyboard.press('Space');
		await expect(section.getByText('Снято: 2')).toBeVisible();
		// One camera, one start per open.
		expect(await page.evaluate(() => (window as unknown as { __gum: number }).__gum)).toBe(1);
		await dialog.getByRole('button', { name: 'Готово' }).click();
		await expect(dialog).toBeHidden();
	});

	test('camera failure says what to do and closes', async ({ page, browserName }) => {
		test.skip(browserName === 'chromium', 'chromium has the fake camera');
		await page.locator('#media').getByRole('button', { name: 'Открыть камеру' }).click();
		const dialog = page.getByRole('dialog', { name: 'Камера' });
		const alert = dialog.getByRole('alert');
		await expect(alert).toBeVisible();
		const text = (await alert.textContent())?.trim() ?? '';
		expect(CAMERA_TEXTS).toContain(text);
		await dialog.getByRole('button', { name: 'Закрыть' }).first().click();
		await expect(dialog).toBeHidden();
	});

	test('viewer pages with arrows, Esc closes, caption saves on Enter', async ({ page }) => {
		const section = page.locator('#media');
		await section.getByRole('button', { name: /^Открыть фото: Экран/ }).click();
		const viewer = page.getByRole('dialog', { name: 'Просмотр фото' });
		await expect(viewer).toBeVisible();
		await expect(viewer.getByText('1 из 3')).toBeVisible();
		await page.keyboard.press('ArrowRight');
		await expect(viewer.getByText('2 из 3')).toBeVisible();
		await page.keyboard.press('ArrowLeft');
		await expect(viewer.getByText('1 из 3')).toBeVisible();
		const caption = viewer.getByRole('textbox', { name: 'Подпись' });
		await caption.fill('Трещина слева');
		await caption.press('Enter');
		await expect(section.getByText('Подпись сохранена: Трещина слева (1)')).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(viewer).toBeHidden();
	});

	test('a caption typed and closed with Esc is still saved, once', async ({ page }) => {
		const section = page.locator('#media');
		await section.getByRole('button', { name: /^Открыть фото: Экран/ }).click();
		const viewer = page.getByRole('dialog', { name: 'Просмотр фото' });
		await viewer.getByRole('textbox', { name: 'Подпись' }).fill('Скол в углу');
		await page.keyboard.press('Escape');
		await expect(viewer).toBeHidden();
		await expect(section.getByText('Подпись сохранена: Скол в углу (1)')).toBeVisible();
	});

	test('file list shows name, size, date and the missing mark', async ({ page }) => {
		const list = page.locator('#media').getByRole('list', { name: 'Файлы' });
		await expect(list.getByText('Акт.pdf')).toBeVisible();
		await expect(list.getByText('1,2 МБ')).toBeVisible();
		await expect(list.getByText('Файл не найден')).toBeVisible();
	});
});
