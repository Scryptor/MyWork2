import {
	expect,
	test,
	type APIRequestContext,
	type Page,
	type PlaywrightWorkerArgs
} from '@playwright/test';
import type { LoginResponse } from '$api/LoginResponse';
import type { Role } from '$api/Role';
import type { User } from '$api/User';
import {
	addDictionaryItem,
	createOrder,
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	uniq
} from './support/api';

// «Настройки» for a user without «settings_manage» (an imported user of MyWork 2, or a role of plan 8). The
// server of 4174 has an administrator: through the API it makes a role without the right and a user with
// it. Every section opens, the fields are disabled and say so, no request of the screen fails in a toast,
// and the counts that need the right are not shown as 0.

const ORIGIN = 'http://127.0.0.1:4174';
test.use({ baseURL: ORIGIN });

/** Test password of the user made here (fictional). */
const CLERK_PASSWORD = 'e2e-clerk-password';

async function ok<T>(response: Awaited<ReturnType<APIRequestContext['post']>>): Promise<T> {
	if (!response.ok()) {
		throw new Error(`${response.status()} ${response.url()}: ${await response.text()}`);
	}
	return (await response.json()) as T;
}

/** A role without «settings_manage», a user with it, and a brand in one order; the login and the brand. */
async function makeClerk(playwright: PlaywrightWorkerArgs['playwright']['request']) {
	const anonymous = await playwright.newContext({ baseURL: ORIGIN });
	const { token } = await ok<LoginResponse>(
		await anonymous.post('/api/v1/auth/login', {
			data: { login: E2E_ADMIN_LOGIN, password: E2E_ADMIN_PASSWORD, device_label: '' }
		})
	);
	await anonymous.dispose();
	const admin = await playwright.newContext({
		baseURL: ORIGIN,
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	try {
		const role = await ok<Role>(
			await admin.post('/api/v1/roles', {
				data: {
					name: uniq('Приёмщик'),
					permissions: [
						'orders_create',
						'orders_edit',
						'clients_view',
						'clients_create',
						'clients_edit'
					]
				}
			})
		);
		const login = uniq('clerk');
		await ok<User>(
			await admin.post('/api/v1/users', {
				data: { login, display_name: 'Приёмщик', role_id: role.id, password: CLERK_PASSWORD }
			})
		);
		const brand = uniq('Бренд');
		await addDictionaryItem(admin, 'brand', brand);
		await createOrder(admin, { brand });
		return { login, brand };
	} finally {
		await admin.dispose();
	}
}

async function signIn(page: Page, login: string) {
	await page.goto('/login');
	await page.getByLabel('Логин').fill(login);
	await page.getByLabel('Пароль').fill(CLERK_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders/);
}

const callout = (page: Page) => page.getByText('Изменять настройки может администратор');
const toasts = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
/** «в 3 заказах», «в 0 заказах», «у 1 клиента»: a count of usage. */
const COUNT = /^(в|у) \d+ (заказ|клиент)/;

test('without «settings_manage» every section opens read-only, without failures and without counts', async ({
	page,
	playwright
}) => {
	const { login, brand } = await makeClerk(playwright.request);
	await signIn(page, login);
	const quiet = () => expect(toasts(page)).toHaveText('');

	await page.goto('/settings/general');
	await expect(callout(page)).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Название сервиса' })).toBeDisabled();
	await expect(page.getByRole('textbox', { name: 'Процент мастера' })).toBeDisabled();
	// The counter needs the right to be read: there is no empty field for it.
	await expect(page.getByRole('textbox', { name: 'Следующий номер заказа' })).toHaveCount(0);
	await quiet();

	await page.goto('/settings/orders');
	await expect(callout(page)).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Долгая диагностика, дней' })).toBeDisabled();
	await quiet();

	await page.goto('/settings/dictionaries?kind=brand');
	await expect(callout(page)).toBeVisible();
	// The brand is in an order: not «в 0 заказах», no count at all.
	await expect(page.getByRole('main').getByText(brand, { exact: true })).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Добавить значение' })).toHaveCount(0);
	await expect(page.getByRole('button', { name: 'Удалить' })).toHaveCount(0);
	await expect(page.getByRole('main').getByText(COUNT)).toHaveCount(0);
	await quiet();

	await page.goto('/settings/statuses');
	await expect(callout(page)).toBeVisible();
	const kinds = page.getByRole('main').getByRole('combobox', { name: /^Вид статуса/ });
	await expect(kinds.first()).toBeDisabled();
	await expect(page.getByRole('main').getByText(COUNT)).toHaveCount(0);
	await quiet();

	await page.goto('/settings/points');
	await expect(callout(page)).toBeVisible();
	await expect(page.getByRole('textbox', { name: 'Название новой точки' })).toHaveCount(0);
	await expect(page.getByRole('main').getByText(COUNT)).toHaveCount(0);
	await quiet();

	const info = (await (await page.request.get('/api/v1/server-info')).json()) as {
		version: string;
	};
	await page.goto('/settings/about');
	await expect(callout(page)).toBeVisible();
	await expect(page.getByText(info.version, { exact: true })).toBeVisible();
	await expect(page.getByText('Не удалось загрузить сведения')).toHaveCount(0);
	await quiet();
});
