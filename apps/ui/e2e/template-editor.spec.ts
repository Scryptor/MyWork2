import {
	expect,
	test,
	type APIRequestContext,
	type Page,
	type PlaywrightWorkerArgs,
	type TestInfo
} from '@playwright/test';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { LoginResponse } from '$api/LoginResponse';
import type { Role } from '$api/Role';
import type { Template } from '$api/Template';
import type { User } from '$api/User';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, uniq } from './support/api';

// The template editor (plan 7.1): opened from «Документы и реквизиты», blocks added and typed into are
// saved by themselves, Ctrl/⌘Z outside the fields undoes the template's edit, «Сбросить к стандартному»
// makes it standard again. Templates are the server's, shared by every test on it: each browser edits its
// own document (the projects may run at once), and the standard one is put back after every test.

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

const KINDS: Record<string, { kind: DocumentKind; name: string }> = {
	chromium: { kind: 'warranty_intake_act', name: 'Акт приёма по гарантии' },
	webkit: { kind: 'warranty_issue_act', name: 'Акт выдачи после гарантии' }
};
const documentOf = (info: TestInfo) => KINDS[info.project.name] ?? KINDS.chromium!;

async function fetchTemplate(request: APIRequestContext, kind: DocumentKind) {
	const response = await request.get(`/api/v1/document-templates/${kind}`);
	expect(response.ok()).toBe(true);
	return (await response.json()) as DocumentTemplate;
}

const textsOf = (t: Template): string[] =>
	t.blocks.flatMap((b) => {
		if (b.body.type === 'text') return [b.body.text];
		if (b.body.type === 'columns')
			return b.body.columns.flat().flatMap((i) => (i.body.type === 'text' ? [i.body.text] : []));
		return [];
	});

const blocks = (page: Page) => page.getByRole('region', { name: 'Блоки' });
/** A row of the list of blocks: its button is named as the row reads, «Текст · …». */
const item = (page: Page, label: string) =>
	blocks(page).getByRole('button', { name: label, exact: true });

async function openEditor(page: Page, info: TestInfo) {
	const { kind } = documentOf(info);
	await page.goto(`/templates/${kind}`);
	await expect(blocks(page).getByRole('button', { name: 'Страница', exact: true })).toBeVisible();
}

/** «Добавить блок» → «Текст», with `text` typed into it. */
async function addText(page: Page, text: string) {
	await blocks(page).getByRole('button', { name: 'Добавить блок' }).click();
	await page.getByRole('menuitem', { name: 'Текст', exact: true }).click();
	const field = page
		.getByRole('region', { name: 'Свойства' })
		.getByRole('textbox', { name: 'Текст', exact: true });
	await expect(field).toHaveValue('Текст');
	await field.fill(text);
}

test.describe('the editor', () => {
	test.afterEach(async ({ request }, info) => {
		const { kind } = documentOf(info);
		const current = await fetchTemplate(request, kind);
		const response = await request.put(`/api/v1/document-templates/${kind}`, {
			data: { template: current.standard, version: current.version }
		});
		expect(response.ok()).toBe(true);
	});

	test('opens_from_documents_settings', async ({ page }, info) => {
		const { kind, name } = documentOf(info);
		await page.goto('/settings/documents');
		await page.getByRole('link', { name: `Изменить шаблон: ${name}`, exact: true }).click();
		await expect(page).toHaveURL(new RegExp(`/templates/${kind}$`));
		await expect(blocks(page).getByRole('button', { name: 'Страница', exact: true })).toBeVisible();
		// The standard act starts with its head: several blocks under «Страница», each can be dragged.
		const handles = blocks(page).getByRole('button', { name: /^Переместить / });
		await expect(handles.first()).toBeVisible();
		expect(await handles.count()).toBeGreaterThan(3);
		await expect(
			page.getByRole('region', { name: 'Превью' }).getByRole('img', { name: /^Страница 1 из \d+$/ })
		).toBeVisible();
		await expect(page.getByRole('combobox', { name: 'Документ' })).toContainText(name);
	});

	test('adds_text_block_and_saves', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const mark = uniq('Строка');
		await openEditor(page, info);
		await addText(page, mark);
		await expect
			.poll(async () => textsOf((await fetchTemplate(request, kind)).template))
			.toContain(mark);
		expect((await fetchTemplate(request, kind)).custom).toBe(true);

		await page.reload();
		await expect(item(page, `Текст · ${mark}`)).toBeVisible();
	});

	test('undo_hotkey_undoes_template_edit', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const mark = uniq('Строка');
		await openEditor(page, info);
		await addText(page, mark);
		await expect
			.poll(async () => textsOf((await fetchTemplate(request, kind)).template))
			.toContain(mark);
		const added = item(page, `Текст · ${mark}`);
		await expect(added).toBeVisible();

		// Out of the field: the key is the editor's, not the field's own undo.
		await page.getByRole('heading', { name: 'Редактор шаблонов' }).click();
		await page.keyboard.press('ControlOrMeta+z');
		await expect(added).toHaveCount(0);
		await expect
			.poll(async () => textsOf((await fetchTemplate(request, kind)).template))
			.not.toContain(mark);

		await page.reload();
		await expect(blocks(page).getByRole('button', { name: 'Страница', exact: true })).toBeVisible();
		// The block itself stays: only the text typed into it is undone.
		await expect(item(page, 'Текст · Текст')).toBeVisible();
		await expect(added).toHaveCount(0);
	});

	test('reset_to_standard', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const mark = uniq('Строка');
		await openEditor(page, info);
		const reset = page.getByRole('button', { name: 'Сбросить к стандартному' });
		await addText(page, mark);
		await expect(reset).toBeEnabled();
		await expect.poll(async () => (await fetchTemplate(request, kind)).custom).toBe(true);

		await reset.click();
		await expect(reset).toBeDisabled();
		await expect(item(page, `Текст · ${mark}`)).toHaveCount(0);
		await expect.poll(async () => (await fetchTemplate(request, kind)).custom).toBe(false);
	});
});

// The server with an administrator (4174): a role without «settings_manage» and a user in it.
test.describe('without the right', () => {
	const ORIGIN = 'http://127.0.0.1:4174';
	test.use({ baseURL: ORIGIN });

	/** Test password of the user made here (fictional). */
	const CLERK_PASSWORD = 'e2e-templates-password';

	async function ok<T>(response: Awaited<ReturnType<APIRequestContext['post']>>): Promise<T> {
		if (!response.ok()) {
			throw new Error(`${response.status()} ${response.url()}: ${await response.text()}`);
		}
		return (await response.json()) as T;
	}

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
					data: { name: uniq('Приёмщик'), permissions: ['orders_create', 'orders_edit'] }
				})
			);
			const login = uniq('clerk');
			await ok<User>(
				await admin.post('/api/v1/users', {
					data: {
						login,
						display_name: 'Приёмщик',
						role_id: role.id,
						password: CLERK_PASSWORD,
						can_sign_in: true,
						is_master: false
					}
				})
			);
			return login;
		} finally {
			await admin.dispose();
		}
	}

	test('no_editor_without_permission', async ({ page, playwright }, info) => {
		const { kind, name } = documentOf(info);
		const login = await makeClerk(playwright.request);
		await page.goto('/login');
		await page.getByLabel('Логин').fill(login);
		await page.getByLabel('Пароль').fill(CLERK_PASSWORD);
		await page.getByRole('button', { name: 'Войти' }).click();
		await expect(page).toHaveURL(/\/orders/);

		await page.goto('/settings/documents');
		await expect(page.getByRole('button', { name: `Образец: ${name}`, exact: true })).toBeVisible();
		await expect(page.getByRole('link', { name: /^Изменить шаблон: / })).toHaveCount(0);

		await page.goto(`/templates/${kind}`);
		await expect(
			page.getByText('Изменять шаблоны может пользователь с правом «Настройки»')
		).toBeVisible();
		await expect(page.getByRole('region', { name: 'Блоки' })).toHaveCount(0);
		await expect(page.getByRole('button', { name: 'Сбросить к стандартному' })).toHaveCount(0);
	});
});
