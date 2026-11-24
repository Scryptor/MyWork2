import {
	expect,
	test as base,
	type BrowserContext,
	type Page,
	type PlaywrightWorkerArgs
} from '@playwright/test';
import {
	createOrder,
	E2E_ADMIN_LOGIN,
	E2E_ADMIN_PASSWORD,
	sessionToken,
	uniq
} from './support/api';
import { adminOrigin } from './support/fresh';

// A server with an administrator, of this spec's own per engine (e2e/support/fresh.ts): every screen of the
// program asks to sign in first. Not the shared one on 4174: the limiter of attempts counts an attempt when
// it is admitted and gives it back only after a success, so the wrong passwords of the tests here and the
// sign-ins of the other specs running at the same time (the same account, the same address) would add up
// to «Слишком много попыток» and a sign-in would fail.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(adminOrigin('login', browserName))
});
const origin = () => adminOrigin('login', test.info().project.name);

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
	await expect(page).toHaveURL(/127\.0\.0\.1:\d+\/orders$/);
});

/**
 * Signs in, makes an order through the API with the same session and opens its card; PATCH of the order
 * fails, so an edit stays unsaved.
 */
async function cardWithUnsavedEdit(
	page: Page,
	playwright: PlaywrightWorkerArgs['playwright'],
	answer: 'abort' | 401
) {
	await page.goto('/orders');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = await sessionToken(page);
	const api = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	const o = await createOrder(api, { model: uniq('M') });
	await api.dispose();
	await page.goto(`/orders/${o.id}`);
	const card = page.getByRole('main');
	const work = card.getByRole('textbox', { name: 'Выполненные работы' });
	await expect(work).toBeVisible();
	await page.route(
		(url) => url.pathname === `/api/v1/orders/${o.id}`,
		(route) =>
			route.request().method() !== 'PATCH'
				? route.continue()
				: answer === 'abort'
					? route.abort('internetdisconnected')
					: route.fulfill({
							status: 401,
							json: {
								code: 'unauthenticated',
								message: 'Войдите в программу',
								fields: [],
								current_version: null,
								permission: null,
								retry_after_secs: null,
								diagnostics: []
							}
						})
	);
	await work.fill('Замена кнопки');
	return o;
}

test('signing out with an unsaved edit on the order card goes to login', async ({
	page,
	playwright
}) => {
	await cardWithUnsavedEdit(page, playwright, 'abort');
	await expect(page.getByRole('main').getByText('Не сохранено')).toBeVisible();
	await page.getByRole('button', { name: /Администратор/ }).click();
	await page.getByRole('menuitem', { name: 'Выйти' }).click();
	// The edit cannot be sent: the program asks instead of dropping it.
	const dialog = page.getByRole('dialog', { name: 'Есть несохранённые правки' });
	await expect(dialog).toBeVisible();
	await expect(page).toHaveURL(/\/orders\//);
	// «Сохранить» tries again; the send still fails: the window says so and stays.
	await dialog.getByRole('button', { name: 'Сохранить' }).click();
	await expect(dialog.getByRole('alert')).toHaveText('Не удалось сохранить — проверьте связь');
	await expect(page).toHaveURL(/\/orders\//);
	await dialog.getByRole('button', { name: 'Выйти без сохранения' }).click();
	await expect(page).toHaveURL(/\/login$/);
	await expect(page.getByRole('button', { name: 'Войти' })).toBeVisible();
});

test('an expired session with an unsaved edit on the order card goes to login', async ({
	page,
	playwright
}) => {
	const o = await cardWithUnsavedEdit(page, playwright, 401);
	await expect(page).toHaveURL(new RegExp(`/login\\?next=%2Forders%2F${o.id}$`));
	await expect(page.getByRole('button', { name: 'Войти' })).toBeVisible();
});

/** Test password of the second user made here (fictional). */
const SECOND_PASSWORD = 'e2e-second-password';

/** A user with order rights, made through the API by the administrator; the login and the shown name. */
async function makeSecondUser(playwright: PlaywrightWorkerArgs['playwright'], adminToken: string) {
	const admin = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${adminToken}` }
	});
	try {
		const role = await admin.post('/api/v1/roles', {
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
		});
		expect(role.ok()).toBe(true);
		const { id } = (await role.json()) as { id: number };
		const login = uniq('second');
		const display = uniq('Второй');
		const user = await admin.post('/api/v1/users', {
			data: {
				login,
				display_name: display,
				role_id: id,
				password: SECOND_PASSWORD,
				can_sign_in: true,
				is_master: false
			}
		});
		expect(user.ok()).toBe(true);
		return { login, display };
	} finally {
		await admin.dispose();
	}
}

test('an edit goes out before signing out, and the next user sees nothing of the first', async ({
	page,
	playwright
}) => {
	await page.goto('/orders');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = (await sessionToken(page)) as string;
	const second = await makeSecondUser(playwright, token);
	const api = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	const o = await createOrder(api, { model: uniq('M') });
	await page.goto(`/orders/${o.id}`);
	const work = page.getByRole('main').getByRole('textbox', { name: 'Выполненные работы' });
	await expect(work).toBeVisible();
	const text = uniq('Замена');
	await work.fill(text);
	// At once, without waiting for the pause of the autosave.
	await page.getByRole('button', { name: /Администратор/ }).click();
	await page.getByRole('menuitem', { name: 'Выйти' }).click();
	await expect(page).toHaveURL(/\/login$/);

	// It was sent as the administrator, before the token went (that token is closed now: a new one).
	await api.dispose();
	const anonymous = await playwright.request.newContext({ baseURL: origin() });
	const fresh = (await (
		await anonymous.post('/api/v1/auth/login', {
			data: { login: E2E_ADMIN_LOGIN, password: E2E_ADMIN_PASSWORD, device_label: '' }
		})
	).json()) as { token: string };
	await anonymous.dispose();
	const reader = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${fresh.token}` }
	});
	const saved = (await (await reader.get(`/api/v1/orders/${o.id}`)).json()) as {
		work_done: string;
	};
	expect(saved.work_done).toBe(text);
	const history = (await (await reader.get(`/api/v1/orders/${o.id}/history`)).json()) as {
		items: { user_name: string; action: string }[];
	};
	expect(history.items.some((e) => e.user_name === 'Администратор')).toBe(true);
	expect(history.items.every((e) => e.user_name !== second.display)).toBe(true);
	await reader.dispose();

	// Another user signs in on the same device: nothing of the first one is shown or sent.
	const writes: string[] = [];
	page.on('request', (r) => {
		if (r.method() !== 'GET' && r.url().includes('/api/v1/orders')) writes.push(r.url());
	});
	await page.getByLabel('Логин').fill(second.login);
	await page.getByLabel('Пароль').fill(SECOND_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByRole('button', { name: new RegExp(second.display) })).toBeVisible();
	await expect(page.getByText('Администратор')).toHaveCount(0);
	await expect(page.getByRole('status', { name: 'Уведомления' })).toHaveText('');
	await expect(page.getByRole('dialog')).toHaveCount(0);
	await page.waitForTimeout(1500);
	expect(writes).toEqual([]);
});

/**
 * Signs in on `page` and opens the card of a new order in another tab of the same browser (the same
 * remembered session). There an edit cannot go (no connection): it waits in that tab's queue.
 */
async function otherTabWithPendingEdit(
	page: Page,
	context: BrowserContext,
	playwright: PlaywrightWorkerArgs['playwright']
) {
	await page.goto('/orders');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = (await sessionToken(page)) as string;
	const api = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	const o = await createOrder(api, { model: uniq('M') });
	await api.dispose();

	const other = await context.newPage();
	const writes: { url: string; auth: string }[] = [];
	other.on('request', (r) => {
		if (r.method() !== 'GET')
			writes.push({ url: r.url(), auth: r.headers()['authorization'] ?? '' });
	});
	await other.goto(`/orders/${o.id}`);
	const work = other.getByRole('main').getByRole('textbox', { name: 'Выполненные работы' });
	await expect(work).toBeVisible();
	const offline = (url: URL) => url.pathname === `/api/v1/orders/${o.id}`;
	await other.route(offline, (route) =>
		route.request().method() === 'PATCH' ? route.abort('internetdisconnected') : route.continue()
	);
	const text = uniq('Замена');
	await work.fill(text);
	await expect(other.getByRole('main').getByText('Не сохранено')).toBeVisible();
	return { token, o, other, writes, text, online: () => other.unroute(offline) };
}

/** The order and its history, read with `token`. */
async function readOrder(
	playwright: PlaywrightWorkerArgs['playwright'],
	token: string,
	id: number
) {
	const reader = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	try {
		const order = (await (await reader.get(`/api/v1/orders/${id}`)).json()) as {
			work_done: string;
		};
		const history = (await (await reader.get(`/api/v1/orders/${id}/history`)).json()) as {
			items: { user_name: string; action: string }[];
		};
		return { order, history: history.items };
	} finally {
		await reader.dispose();
	}
}

test('signing out and in as another user in one tab sends the other tab to the login, with nothing sent', async ({
	page,
	context,
	playwright
}) => {
	const { token, o, other, writes, online } = await otherTabWithPendingEdit(
		page,
		context,
		playwright
	);
	const second = await makeSecondUser(playwright, token);

	// This tab signs out: the other one asks to sign in, the first user's screens and edits are gone.
	await page.getByRole('button', { name: /Администратор/ }).click();
	await page.getByRole('menuitem', { name: 'Выйти' }).click();
	await expect(page).toHaveURL(/\/login$/);
	await expect(other).toHaveURL(/\/login/);
	await expect(other.getByRole('button', { name: 'Войти' })).toBeVisible();
	await expect(other.getByRole('textbox', { name: 'Выполненные работы' })).toHaveCount(0);
	await expect(other.getByRole('status', { name: 'Уведомления' })).toContainText(
		'В другой вкладке выполнен выход или вход другим пользователем'
	);

	// Another user signs in here: the other tab, without a session, takes theirs and goes back where it
	// was — as that user, with nothing of the first one.
	await page.getByLabel('Логин').fill(second.login);
	await page.getByLabel('Пароль').fill(SECOND_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	const secondToken = (await sessionToken(page)) as string;
	expect(secondToken).not.toBeNull();
	await expect(other).toHaveURL(new RegExp(`/orders/${o.id}$`));
	const work = other.getByRole('main').getByRole('textbox', { name: 'Выполненные работы' });
	await expect(work).toHaveValue('');

	// The connection is back: nothing of the first user goes out under the second one.
	await online();
	await other.waitForTimeout(1500);
	expect(writes.filter((w) => w.auth === `Bearer ${secondToken}`)).toEqual([]);
	const { order, history } = await readOrder(playwright, secondToken, o.id);
	expect(order.work_done).toBe('');
	expect(history.every((e) => e.user_name !== second.display)).toBe(true);
	await other.close();
});

test('signing in again as the same user in one tab keeps the other tab, and its edit goes with the new token', async ({
	page,
	context,
	playwright
}) => {
	const { token, o, other, writes, text, online } = await otherTabWithPendingEdit(
		page,
		context,
		playwright
	);

	// This tab signs in again as the same user (the login page opened by hand): a new token.
	await page.goto('/login');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const fresh = (await sessionToken(page)) as string;
	expect(fresh).not.toBe(token);

	// The other tab goes on: the card and the unsaved edit stay, no notice.
	await page.waitForTimeout(500);
	await expect(other).toHaveURL(new RegExp(`/orders/${o.id}$`));
	const work = other.getByRole('main').getByRole('textbox', { name: 'Выполненные работы' });
	await expect(work).toHaveValue(text);
	await expect(other.getByRole('status', { name: 'Уведомления' })).not.toContainText(
		'В другой вкладке'
	);

	// The connection is back: «Повторить» saves the edit as the same user, with the new token.
	await online();
	await other.getByRole('main').getByRole('button', { name: 'Повторить' }).click();
	await expect(other.getByRole('main').getByText('✓ Сохранено')).toBeVisible();
	const saves = writes.filter((w) => new URL(w.url).pathname === `/api/v1/orders/${o.id}`);
	expect(saves.at(-1)?.auth).toBe(`Bearer ${fresh}`);
	const { order, history } = await readOrder(playwright, fresh, o.id);
	expect(order.work_done).toBe(text);
	expect(history[0]?.user_name).toBe('Администратор');
	await other.close();
});

/** The status of `GET /auth/me` with `token`: 200 while the session lives, 401 once it is closed. */
async function meStatus(
	playwright: PlaywrightWorkerArgs['playwright'],
	token: string
): Promise<number> {
	const api = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	try {
		return (await api.get('/api/v1/auth/me')).status();
	} finally {
		await api.dispose();
	}
}

test('a session that expired in one tab: the other tab says it expired, not that another user signed in', async ({
	page,
	context,
	playwright
}) => {
	await page.goto('/orders');
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = (await sessionToken(page)) as string;
	const other = await context.newPage();
	await other.goto('/orders');
	await expect(other.getByRole('navigation', { name: 'Разделы' })).toContainText('Заказы');

	// The server closes the session (as its expiry would); this tab finds out at its next load.
	const api = await playwright.request.newContext({
		baseURL: origin(),
		extraHTTPHeaders: { Authorization: `Bearer ${token}` }
	});
	expect((await api.post('/api/v1/auth/logout', { data: {} })).status()).toBe(204);
	await api.dispose();
	await page.reload();
	await expect(page).toHaveURL(/\/login\?next=/);

	await expect(other).toHaveURL(/\/login/);
	await expect(other.getByRole('status', { name: 'Уведомления' })).toContainText(
		'Сеанс истёк — войдите снова'
	);
	await expect(other.getByRole('status', { name: 'Уведомления' })).not.toContainText(
		'В другой вкладке'
	);
	await other.close();
});

test('a tab signed in without «Запомнить» is reset when another user signs in elsewhere; its token is revoked', async ({
	page,
	context,
	playwright
}) => {
	await page.goto('/login');
	await page.getByLabel('Запомнить на этом устройстве').uncheck();
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = (await sessionToken(page)) as string;
	expect(await page.evaluate(() => sessionStorage.getItem('mw.session'))).not.toBeNull();
	expect(await page.evaluate(() => localStorage.getItem('mw.session'))).toBeNull();
	const second = await makeSecondUser(playwright, token);

	// Another tab: another user signs in with «Запомнить» (the shared storage).
	const other = await context.newPage();
	await other.goto('/login');
	await other.getByLabel('Логин').fill(second.login);
	await other.getByLabel('Пароль').fill(SECOND_PASSWORD);
	await other.getByRole('button', { name: 'Войти' }).click();
	await expect(other).toHaveURL(/\/orders$/);

	// The first tab is on the login page, with nothing of the administrator.
	await expect(page).toHaveURL(/\/login/);
	await expect(page.getByText('Администратор')).toHaveCount(0);
	await expect(page.getByRole('status', { name: 'Уведомления' })).toContainText(
		'В другой вкладке выполнен выход или вход другим пользователем'
	);
	expect(await page.evaluate(() => sessionStorage.getItem('mw.session'))).toBeNull();
	// The administrator's token is closed on the server.
	await expect.poll(() => meStatus(playwright, token)).toBe(401);
	await other.close();
});

test('a tab signed in without «Запомнить» is not touched when the same user signs in elsewhere', async ({
	page,
	context
}) => {
	await page.goto('/login');
	await page.getByLabel('Запомнить на этом устройстве').uncheck();
	await signIn(page);
	await expect(page).toHaveURL(/\/orders$/);
	const token = (await sessionToken(page)) as string;

	const other = await context.newPage();
	await other.goto('/login');
	await signIn(other);
	await expect(other).toHaveURL(/\/orders$/);

	await page.waitForTimeout(500);
	await expect(page).toHaveURL(/\/orders$/);
	expect(await sessionToken(page)).toBe(token);
	await expect(page.getByRole('status', { name: 'Уведомления' })).not.toContainText(
		'В другой вкладке'
	);
	await other.close();
});
