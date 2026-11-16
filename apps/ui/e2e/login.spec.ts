import { expect, test, type Page } from '@playwright/test';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD } from './support/api';

// The server on 4174 has an administrator: every screen of the program asks to sign in first.
test.use({ baseURL: 'http://127.0.0.1:4174' });

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

async function signIn(page: Page, password = E2E_ADMIN_PASSWORD) {
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(password);
	await page.getByRole('button', { name: 'Войти' }).click();
}

test('login returns to the page that asked for it', async ({ page }) => {
	await page.goto('/orders?chips=all');
	await expect(page).toHaveURL(/\/login\?next=/);
	await expect(page.getByLabel('Запомнить на этом устройстве')).toBeChecked();
	await signIn(page, 'неверно');
	await expect(page.getByText('Неверный логин или пароль')).toBeVisible();
	await signIn(page);
	await expect(page).toHaveURL(/\/orders\?chips=all$/);
	await expect(page.getByRole('navigation', { name: 'Разделы' })).toContainText('Заказы');
});

test('logout from the user menu goes to login', async ({ page }) => {
	await page.goto('/orders');
	await expect(page).toHaveURL(/\/login\?next=%2Forders$/);
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	await page.getByRole('button', { name: /Администратор/ }).click();
	await expect(page.getByRole('menuitem', { name: 'Сменить пароль' })).toBeVisible();
	await page.getByRole('menuitem', { name: 'Выйти' }).click();
	await expect(page).toHaveURL(/\/login$/);
	// The token is gone: the program asks to sign in again.
	await page.goto('/orders');
	await expect(page).toHaveURL(/\/login\?next=%2Forders$/);
});

test('password change: a wrong current password is shown at the field; success asks to sign in again', async ({
	page
}) => {
	await page.goto('/orders');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	await page.getByRole('button', { name: /Администратор/ }).click();
	await page.getByRole('menuitem', { name: 'Сменить пароль' }).click();
	const dialog = page.getByRole('dialog', { name: 'Смена пароля' });
	await dialog.getByLabel('Текущий пароль').fill('неверно');
	await dialog.getByLabel(/^Новый пароль/).fill('новый-пароль');
	await dialog.getByLabel('Повторите новый пароль').fill('новый-пароль');
	await dialog.getByRole('button', { name: /^Сменить пароль/ }).click();
	await expect(dialog.getByText('Неверный текущий пароль')).toBeVisible();
	await expect(dialog.getByLabel('Текущий пароль')).toHaveAttribute('aria-invalid', 'true');

	// The other tests sign in as this administrator: the change itself is a stubbed answer, the server
	// keeps the password. The program then signs out and asks to sign in with the new one.
	await page.route('**/api/v1/auth/password', (route) => route.fulfill({ status: 204 }));
	await dialog.getByLabel('Текущий пароль').fill(E2E_ADMIN_PASSWORD);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(page).toHaveURL(/\/login\?next=%2Forders$/);
	await expect(page.getByRole('status', { name: 'Уведомления' })).toContainText(
		'Пароль изменён. Войдите с новым паролем'
	);
	expect(await page.evaluate(() => localStorage.getItem('mw.session'))).toBeNull();
});

test('a link to another site in next is not followed', async ({ page }) => {
	await page.goto('/login?next=//example.com/');
	await signIn(page);
	await expect(page).toHaveURL(/127\.0\.0\.1:4174\/orders$/);
});
