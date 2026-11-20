import { expect, test, type APIRequestContext, type Browser, type Page } from '@playwright/test';
import type { LoginResponse } from '$api/LoginResponse';
import type { MasterUser } from '$api/MasterUser';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, uniq } from './support/api';

// «Настройки → Пользователи» on the server with an administrator (4174): a master without sign-in is a
// master of orders; a user with sign-in signs in; the last administrator cannot lose «Пользователи и права»
// and the server's text is seen. Names are unique: the server is shared.

const ORIGIN = 'http://127.0.0.1:4174';
test.use({
	baseURL: ORIGIN,
	screen: { width: 1280, height: 720 },
	viewport: { width: 1280, height: 720 }
});

/** Test password of the user made here (fictional). */
const USER_PASSWORD = 'e2e-user-password';

async function signInAdmin(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

/** A role with order rights only, made through the API with the administrator's token of the page. */
async function makeRole(page: Page): Promise<string> {
	const token = await page.evaluate(() => {
		const raw = sessionStorage.getItem('mw.session') ?? localStorage.getItem('mw.session');
		return (JSON.parse(raw ?? '{}') as { token?: string }).token ?? '';
	});
	const name = uniq('Роль');
	const response = await page.request.post('/api/v1/roles', {
		headers: { Authorization: `Bearer ${token}` },
		data: { name, permissions: ['orders_create', 'orders_edit'] }
	});
	expect(response.ok()).toBe(true);
	return name;
}

async function pickRole(page: Page, name: string) {
	const dialog = page.getByRole('dialog');
	await dialog.getByRole('combobox', { name: /^Роль/ }).click();
	await page.getByRole('option', { name, exact: true }).click();
}

async function mastersOf(page: Page): Promise<MasterUser[]> {
	const token = await page.evaluate(() => {
		const raw = sessionStorage.getItem('mw.session') ?? localStorage.getItem('mw.session');
		return (JSON.parse(raw ?? '{}') as { token?: string }).token ?? '';
	});
	const response = await page.request.get('/api/v1/masters', {
		headers: { Authorization: `Bearer ${token}` }
	});
	return (await response.json()) as MasterUser[];
}

test('a master without sign-in is added and is a master of orders', async ({ page }) => {
	await signInAdmin(page);
	const role = await makeRole(page);
	const name = uniq('Мастер');
	await page.goto('/settings/users');
	await page.getByRole('button', { name: 'Добавить пользователя' }).click();
	const dialog = page.getByRole('dialog', { name: 'Новый пользователь' });
	await dialog.getByRole('textbox', { name: 'Имя' }).fill(name);
	await expect(dialog.getByLabel('Логин')).toHaveCount(0);
	await pickRole(page, role);
	await dialog.getByRole('switch', { name: 'Мастер', exact: true }).check();
	await dialog.getByRole('button', { name: 'Добавить', exact: true }).click();
	await expect(dialog).toBeHidden();
	const row = page.getByRole('button', { name: `Изменить: ${name}` });
	await expect(row).toContainText('без входа');
	await expect(row).toContainText('Мастер');
	expect((await mastersOf(page)).some((m) => m.display_name === name)).toBe(true);
});

test('a user with sign-in is added, signs in, and a taken login is refused at the field', async ({
	page,
	browser
}) => {
	await signInAdmin(page);
	const role = await makeRole(page);
	const login = uniq('user');
	await page.goto('/settings/users');
	const add = async (password: string) => {
		await page.getByRole('button', { name: 'Добавить пользователя' }).click();
		const dialog = page.getByRole('dialog', { name: 'Новый пользователь' });
		await dialog.getByRole('switch', { name: 'Может входить в программу' }).check();
		await dialog.getByRole('textbox', { name: /^Логин/ }).fill(login);
		await dialog.getByLabel(/^Пароль/).fill(password);
		await pickRole(page, role);
		await dialog.getByRole('button', { name: 'Добавить', exact: true }).click();
		return dialog;
	};
	await expect(await add(USER_PASSWORD).then((d) => d)).toBeHidden();
	await expect(page.getByRole('button', { name: `Изменить: ${login}` })).toBeVisible();

	const second = await add(USER_PASSWORD);
	await expect(second.getByText('Такое значение уже есть')).toBeVisible();
	await second.getByRole('button', { name: 'Отмена' }).click();

	await signInAs(browser, login);
});

async function signInAs(browser: Browser, login: string) {
	const context = await browser.newContext({ baseURL: ORIGIN });
	try {
		const page = await context.newPage();
		await page.goto('/login');
		await page.getByLabel('Логин').fill(login);
		await page.getByLabel('Пароль').fill(USER_PASSWORD);
		await page.getByRole('button', { name: 'Войти' }).click();
		await expect(page).toHaveURL(/\/orders/);
	} finally {
		await context.close();
	}
}

test('the last administrator keeps «Пользователи и права»: the text of the server is shown', async ({
	page
}) => {
	await signInAdmin(page);
	const role = await makeRole(page);
	await page.goto('/settings/users');
	await page
		.getByRole('button', { name: /^Изменить:/ })
		.filter({ hasText: E2E_ADMIN_LOGIN })
		.click();
	const dialog = page.getByRole('dialog', { name: 'Пользователь' });
	await pickRole(page, role);
	await expect(dialog.getByText('Нельзя оставить программу без администратора')).toBeVisible();
	await expect(dialog.getByRole('combobox', { name: /^Роль/ })).toHaveText(/Администратор/);
	await dialog.getByRole('button', { name: 'Отключить учётную запись', exact: true }).click();
	await expect(dialog.getByText('Нельзя оставить программу без администратора')).toBeVisible();
});

test('a user can be disabled, enabled again, and sign-in turned off and on', async ({ page }) => {
	await signInAdmin(page);
	const role = await makeRole(page);
	const name = uniq('Сотрудник');
	await page.goto('/settings/users');
	await page.getByRole('button', { name: 'Добавить пользователя' }).click();
	const add = page.getByRole('dialog', { name: 'Новый пользователь' });
	await add.getByRole('textbox', { name: 'Имя' }).fill(name);
	await pickRole(page, role);
	await add.getByRole('button', { name: 'Добавить', exact: true }).click();
	await expect(add).toBeHidden();

	await page.getByRole('button', { name: `Изменить: ${name}` }).click();
	const dialog = page.getByRole('dialog', { name: 'Пользователь' });
	await dialog.getByRole('button', { name: 'Отключить учётную запись', exact: true }).click();
	await expect(dialog.getByText('Учётная запись отключена')).toBeVisible();
	await dialog.getByRole('button', { name: 'Включить учётную запись', exact: true }).click();
	await expect(dialog.getByText('Учётная запись отключена')).toBeHidden();

	// Sign-in on: the login and the password are asked for.
	await dialog.getByRole('switch', { name: 'Может входить в программу' }).check();
	await dialog.getByRole('button', { name: 'Включить вход' }).click();
	await expect(dialog.getByText('Введите логин')).toBeVisible();
	await dialog.getByRole('textbox', { name: /^Логин/ }).fill(uniq('in'));
	await dialog.getByLabel(/^Пароль/).fill(USER_PASSWORD);
	await dialog.getByRole('button', { name: 'Включить вход' }).click();
	await expect(dialog.getByRole('button', { name: 'Сменить пароль' })).toBeVisible();
	await dialog.getByRole('switch', { name: 'Может входить в программу' }).uncheck();
	await expect(dialog.getByRole('switch', { name: 'Может входить в программу' })).not.toBeChecked();
});

export type { APIRequestContext, LoginResponse };

test('the four hints of a user open and close with Esc', async ({ page }) => {
	await signInAdmin(page);
	await page.goto('/settings/users');
	await page
		.getByRole('button', { name: /^Изменить:/ })
		.filter({ hasText: E2E_ADMIN_LOGIN })
		.click();
	const dialog = page.getByRole('dialog', { name: 'Пользователь', exact: true });
	for (const title of ['Роль', 'Мастер', 'Может входить в программу', 'Отключить учётную запись']) {
		const icon = dialog.getByRole('button', { name: `Что такое «${title}»` });
		await icon.click();
		const hint = page.getByRole('dialog', { name: title, exact: true });
		await expect(hint).toBeVisible();
		await page.keyboard.press('Escape');
		await expect(hint).toBeHidden();
		await expect(dialog).toBeVisible();
	}
});
