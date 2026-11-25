import { expect, test, type Browser, type Page } from '@playwright/test';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, sessionToken, uniq } from './support/api';

// «Настройки → Роли» on the server with an administrator (4174): a role with two rights is made with
// checkboxes, given to a user, who then sees only what it allows; an empty role is deleted; the last
// «Пользователи и права» stays and the server's text is seen. Names are unique: the server is shared.

const ORIGIN = 'http://127.0.0.1:4174';
test.use({
	baseURL: ORIGIN,
	screen: { width: 1280, height: 720 },
	viewport: { width: 1280, height: 720 }
});

/** Test password of the user made here (fictional). */
const USER_PASSWORD = 'e2e-roles-password';

async function signInAdmin(page: Page) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(E2E_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(E2E_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

async function addRole(page: Page, name: string, rights: string[]) {
	await page.goto('/settings/roles');
	await page.getByRole('button', { name: 'Добавить роль' }).click();
	const dialog = page.getByRole('dialog', { name: 'Новая роль' });
	await dialog.getByRole('textbox', { name: 'Название' }).fill(name);
	for (const right of rights) await dialog.getByRole('checkbox', { name: right }).check();
	await dialog.getByRole('button', { name: 'Добавить', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('button', { name: `Изменить: ${name}` })).toBeVisible();
}

async function signInAs(browser: Browser, login: string) {
	const context = await browser.newContext({ baseURL: ORIGIN });
	const page = await context.newPage();
	await page.goto('/login');
	await page.getByLabel('Логин').fill(login);
	await page.getByLabel('Пароль').fill(USER_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
	return { page, context };
}

test('a role made with checkboxes is given to a user who sees only what it allows', async ({
	page,
	browser
}) => {
	await signInAdmin(page);
	const name = uniq('Роль');
	await addRole(page, name, ['Добавление заказов', 'Изменение заказов']);

	// The rights are kept: the role opens with the two ticked.
	await page.getByRole('button', { name: `Изменить: ${name}` }).click();
	const dialog = page.getByRole('dialog', { name: 'Роль', exact: true });
	await expect(dialog.getByRole('checkbox', { name: 'Добавление заказов' })).toBeChecked();
	await expect(dialog.getByRole('checkbox', { name: 'Изменение заказов' })).toBeChecked();
	await expect(dialog.getByRole('checkbox', { name: 'Настройки', exact: true })).not.toBeChecked();
	await page.keyboard.press('Escape');
	await expect(dialog).toBeHidden();

	// A user in the role, through the API: signs in and is refused the pages of an administrator.
	const token = (await sessionToken(page)) ?? '';
	const headers = { Authorization: `Bearer ${token}` };
	const roles = (await (await page.request.get('/api/v1/roles', { headers })).json()) as {
		id: number;
		name: string;
	}[];
	const roleId = roles.find((r) => r.name === name)?.id;
	const login = uniq('user');
	const made = await page.request.post('/api/v1/users', {
		headers,
		data: {
			display_name: login,
			login,
			password: USER_PASSWORD,
			role_id: roleId,
			can_sign_in: true,
			is_master: false
		}
	});
	expect(made.ok()).toBe(true);

	// The role now has a user: it cannot be deleted and says why.
	await page.reload();
	await expect(page.getByRole('button', { name: `Изменить: ${name}` })).toContainText(
		'1 пользователь'
	);
	await page.getByRole('button', { name: `Изменить: ${name}` }).click();
	await expect(dialog.getByRole('button', { name: 'Удалить роль' })).toBeDisabled();
	await expect(dialog.getByText(/пока в ней есть пользователи: 1/)).toBeVisible();

	const user = await signInAs(browser, login);
	try {
		await user.page.goto('/settings/users');
		await expect(user.page.getByText('Пользователей настраивает администратор')).toBeVisible();
		await expect(user.page.getByRole('button', { name: 'Добавить пользователя' })).toHaveCount(0);
		await user.page.goto('/settings/roles');
		await expect(user.page.getByText('Роли настраивает администратор')).toBeVisible();
		await expect(user.page.getByRole('button', { name: 'Добавить роль' })).toHaveCount(0);
		await user.page.goto('/settings/general');
		await expect(user.page.getByText('Изменять настройки может администратор')).toBeVisible();

		// An allowed action works: the intake button is there and the server answers the client list.
		await user.page.goto('/orders');
		await user.page
			.getByRole('button', { name: /^Принять/ })
			.first()
			.click();
		await expect(user.page.getByRole('dialog', { name: 'Новый заказ' })).toBeVisible();
		const own = { Authorization: `Bearer ${(await sessionToken(user.page)) ?? ''}` };
		expect((await user.page.request.get('/api/v1/clients', { headers: own })).status()).toBe(200);
		// The rights that role lacks are refused by the server itself, not only hidden.
		expect((await user.page.request.get('/api/v1/users', { headers: own })).status()).toBe(403);
		expect(
			(
				await user.page.request.post('/api/v1/roles', {
					headers: own,
					data: { name: uniq('x'), permissions: [] }
				})
			).status()
		).toBe(403);
		expect(
			(await user.page.request.patch('/api/v1/settings', { headers: own, data: {} })).status()
		).toBe(403);
	} finally {
		await user.context.close();
	}
});

test('an empty role is renamed and deleted', async ({ page }) => {
	await signInAdmin(page);
	const name = uniq('Пустая');
	await addRole(page, name, []);
	await page.getByRole('button', { name: `Изменить: ${name}` }).click();
	const dialog = page.getByRole('dialog', { name: 'Роль', exact: true });
	const renamed = `${name}-2`;
	await dialog.getByRole('textbox', { name: 'Название' }).fill(renamed);
	await expect(page.getByRole('button', { name: `Изменить: ${renamed}` })).toBeVisible();
	await dialog.getByRole('button', { name: 'Удалить роль' }).click();
	await page.getByRole('alertdialog').getByRole('button', { name: 'Удалить', exact: true }).click();
	await expect(dialog).toBeHidden();
	await expect(page.getByRole('button', { name: `Изменить: ${renamed}` })).toHaveCount(0);
});

test('the last «Пользователи и права» stays: the text of the server is shown and the box comes back', async ({
	page
}) => {
	await signInAdmin(page);
	await page.goto('/settings/roles');
	await page.getByRole('button', { name: 'Изменить: Администратор' }).click();
	const dialog = page.getByRole('dialog', { name: 'Роль', exact: true });
	const box = dialog.getByRole('checkbox', { name: 'Пользователи и права' });
	await expect(box).toBeChecked();
	await box.uncheck();
	await expect(dialog.getByRole('alert')).toContainText(/Нельзя/);
	await expect(box).toBeChecked();
});

test('each right has a hint that says what it allows now, and Esc closes it', async ({ page }) => {
	await signInAdmin(page);
	await page.goto('/settings/roles');
	await page.getByRole('button', { name: 'Изменить: Мастер' }).click();
	const dialog = page.getByRole('dialog', { name: 'Роль', exact: true });
	await expect(dialog.getByRole('button', { name: /^Что такое «/ })).toHaveCount(18);
	await dialog.getByRole('button', { name: 'Что такое «Просмотр склада»' }).click();
	const hint = page.getByRole('dialog', { name: 'Просмотр склада', exact: true });
	await expect(hint).toContainText('Открывать раздел «Склад»');
	await page.keyboard.press('Escape');
	await expect(hint).toBeHidden();
	await expect(dialog).toBeVisible();
});
