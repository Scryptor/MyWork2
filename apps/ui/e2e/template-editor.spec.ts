import {
	expect,
	test as base,
	type APIRequestContext,
	type Locator,
	type Page,
	type PlaywrightWorkerArgs,
	type TestInfo
} from '@playwright/test';
import type { Block } from '$api/Block';
import type { DocumentKind } from '$api/DocumentKind';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import type { LoginResponse } from '$api/LoginResponse';
import type { Role } from '$api/Role';
import type { Template } from '$api/Template';
import type { User } from '$api/User';
import { ownOrigin } from './support/fresh';
import { E2E_ADMIN_LOGIN, E2E_ADMIN_PASSWORD, createOrder, uniq } from './support/api';

// The template editor (plan 7.1): opened from «Документы и реквизиты», blocks added and typed into are
// saved by themselves, Ctrl/⌘Z outside the fields undoes the template's edit, «Сбросить к стандартному»
// makes it standard again. Templates are the server's, shared by every test on it: each browser edits its
// own document (the projects may run at once), and the standard one is put back after every test.
// The templates are one per base, and the printing specs (print, bulk-print) render the acts and stickers of
// the server of 4173: so the editor has a server of its own per engine, and no spec of printing sees a
// template made here (or the editor one made there).
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(ownOrigin('templates', browserName))
});

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

const props = (page: Page) => page.getByRole('region', { name: 'Свойства' });
/** The blocks of the template of a kind, flattened, in reading order. */
const allBlocks = (t: Template): Block[] =>
	t.blocks.flatMap((b) => (b.body.type === 'columns' ? [b, ...b.body.columns.flat()] : [b]));

const blocks = (page: Page) => page.getByRole('region', { name: 'Блоки' });
/** A row of the list of blocks: its button is named as the row reads, «Текст · …». */
const item = (page: Page, label: string) =>
	blocks(page).getByRole('button', { name: label, exact: true });

async function openEditor(page: Page, info: TestInfo) {
	const { kind } = documentOf(info);
	await page.goto(`/templates/${kind}`);
	await expect(blocks(page).getByRole('button', { name: 'Страница', exact: true })).toBeVisible();
}

/** A choice of a segmented control. */
async function choose(radio: Locator) {
	await radio.click();
	await expect(radio).toBeChecked();
}

/** «Добавить блок» → the type; the new block is chosen. */
async function addBlock(page: Page, type: string) {
	await blocks(page).getByRole('button', { name: 'Добавить блок' }).click();
	await page.getByRole('menuitem', { name: type, exact: true }).click();
	await expect(props(page).getByRole('heading', { name: type, exact: true })).toBeVisible();
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

	test('leaving_right_after_typing_saves_the_template_first', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const mark = uniq('Строка');
		await openEditor(page, info);
		// A slow server: leaving must wait for the answer.
		await page.route(`**/api/v1/document-templates/${kind}`, async (route) => {
			if (route.request().method() === 'PUT') await new Promise((r) => setTimeout(r, 700));
			await route.continue();
		});
		await addText(page, mark);
		await page.getByRole('link', { name: 'Документы и реквизиты' }).first().click();
		await expect(page).toHaveURL(/\/settings\/documents$/);
		expect(textsOf((await fetchTemplate(request, kind)).template)).toContain(mark);
	});

	test('a_failed_save_keeps_the_user_in_the_editor', async ({ page }, info) => {
		const { kind } = documentOf(info);
		await openEditor(page, info);
		await page.route(`**/api/v1/document-templates/${kind}`, (route) =>
			route.request().method() === 'PUT' ? route.abort('internetdisconnected') : route.continue()
		);
		await addText(page, uniq('Строка'));
		await page.getByRole('link', { name: 'Документы и реквизиты' }).first().click();
		await expect(page.getByText(/Шаблон не сохранён/).first()).toBeVisible();
		await expect(page).toHaveURL(new RegExp(`/templates/${kind}$`));
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

	test('edits_every_block_type', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const sign = uniq('Подпись');
		const row = uniq('Строка');
		await openEditor(page, info);
		const p = props(page);
		const number = (name: string) => p.getByRole('textbox', { name, exact: true });

		await addBlock(page, 'Поля');
		await p.getByRole('button', { name: 'Добавить строку' }).click();
		await p
			.getByRole('group', { name: 'Строка 2' })
			.getByRole('textbox', { name: 'Название' })
			.fill(row);
		const second = p.getByRole('group', { name: 'Строка 2' });
		// The insert button is named after its row, and every row has «Жирная».
		await expect(
			p.getByRole('button', { name: `Вставить поле: ${row}`, exact: true })
		).toBeVisible();
		await expect(
			p.getByRole('button', { name: 'Вставить поле: Название', exact: true })
		).toBeVisible();
		await expect(second.getByRole('switch', { name: 'Жирная' })).toBeVisible();
		await addBlock(page, 'Штрихкод');
		await number('Высота, мм').fill('20');
		await addBlock(page, 'Подписи');
		await p.getByRole('textbox', { name: 'Справа', exact: true }).fill(sign);
		await addBlock(page, 'Отступ');
		await number('Отступ, мм').fill('8');
		await addBlock(page, 'Колонки');
		await choose(p.getByRole('radio', { name: '3', exact: true }));
		await number('Промежуток, мм').fill('9');
		await addBlock(page, 'Подпись экземпляра');
		await choose(p.getByRole('radio', { name: 'По центру' }));
		await addBlock(page, 'Разделитель');

		const check = (t: Template) => {
			const all = allBlocks(t);
			const of = <T extends Block['body']['type']>(type: T) =>
				all
					.map((b) => b.body)
					.filter((b): b is Extract<Block['body'], { type: T }> => b.type === type);
			// The standard act has some of these blocks too: the ones added are the last of their kind.
			expect(of('fields').at(-1)?.rows[1]?.label).toBe(row);
			expect(of('barcode').at(-1)?.height_mm).toBe(20);
			expect(of('signatures').at(-1)?.right).toBe(sign);
			expect(of('spacer').at(-1)?.mm).toBe(8);
			expect(of('columns').at(-1)?.columns.length).toBe(3);
			expect(of('columns').at(-1)?.gap_mm).toBe(9);
			expect(of('copy_label').at(-1)?.align).toBe('center');
			expect(of('separator').length).toBeGreaterThan(0);
		};
		await expect
			.poll(async () => {
				try {
					check((await fetchTemplate(request, kind)).template);
					return true;
				} catch {
					return false;
				}
			})
			.toBe(true);

		await page.reload();
		await item(page, 'Отступ · 8 мм').click();
		await expect(number('Отступ, мм')).toHaveValue('8');
		await item(page, 'Колонки · 3').click();
		await expect(number('Промежуток, мм')).toHaveValue('9');
		await expect(p.getByRole('radio', { name: '3', exact: true })).toBeChecked();
		await item(page, 'Штрихкод').last().click();
		await expect(number('Высота, мм')).toHaveValue('20');
	});

	test('insert_field_into_text', async ({ page }, info) => {
		await openEditor(page, info);
		await addText(page, 'abcd');
		const p = props(page);
		const field = p.getByRole('textbox', { name: 'Текст', exact: true });
		await field.evaluate((el: HTMLTextAreaElement) => {
			el.focus();
			el.setSelectionRange(2, 2);
		});
		await p.getByRole('button', { name: 'Вставить поле' }).click();
		await p.getByRole('combobox', { name: 'Поле' }).fill('номер');
		await page.getByRole('option', { name: 'Номер заказа' }).click();
		await expect(field).toHaveValue('ab{заказ.номер}cd');
		// The focus is back in the field, the caret right after what was put in.
		await expect(field).toBeFocused();
		expect(await field.evaluate((el: HTMLTextAreaElement) => el.selectionStart)).toBe(
			'ab{заказ.номер}'.length
		);
	});

	test('export_and_import_file', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const mark = uniq('Строка');
		await openEditor(page, info);
		const before = (await fetchTemplate(request, kind)).template;

		const saving = page.waitForEvent('download');
		await page.getByRole('button', { name: 'Файл' }).click();
		await page.getByRole('menuitem', { name: 'Сохранить в файл…' }).click();
		const download = await saving;
		expect(download.suggestedFilename()).toBe(
			kind === 'warranty_intake_act'
				? 'akt-priema-garantiya.mwtemplate'
				: 'akt-vydachi-garantiya.mwtemplate'
		);
		const path = await download.path();

		await addText(page, mark);
		await expect(item(page, `Текст · ${mark}`)).toBeVisible();
		const loading = page.waitForEvent('filechooser');
		await page.getByRole('button', { name: 'Файл' }).click();
		await page.getByRole('menuitem', { name: 'Загрузить из файла…' }).click();
		await (await loading).setFiles(path);
		await expect(page.getByText('Шаблон загружен из файла')).toBeVisible();
		await expect(item(page, `Текст · ${mark}`)).toHaveCount(0);
		await expect
			.poll(async () => textsOf((await fetchTemplate(request, kind)).template))
			.toEqual(textsOf(before));

		// The load is a step of the history: undone, the block is back.
		await page.getByRole('heading', { name: 'Редактор шаблонов' }).click();
		await page.keyboard.press('ControlOrMeta+z');
		await expect(item(page, `Текст · ${mark}`)).toBeVisible();
	});

	test('bad_file_leaves_editor_unchanged', async ({ page }, info) => {
		await openEditor(page, info);
		const handles = blocks(page).getByRole('button', { name: /^Переместить / });
		const count = await handles.count();
		const loading = page.waitForEvent('filechooser');
		await page.getByRole('button', { name: 'Файл' }).click();
		await page.getByRole('menuitem', { name: 'Загрузить из файла…' }).click();
		await (
			await loading
		).setFiles({
			name: 'bad.mwtemplate',
			mimeType: 'application/json',
			buffer: Buffer.from('{"format":"x"}')
		});
		await expect(page.getByText('Это не файл шаблона MyWork')).toBeVisible();
		await expect(handles).toHaveCount(count);
	});

	test('block_menu_actions', async ({ page, request }, info) => {
		const { kind } = documentOf(info);
		const a = uniq('Один');
		const b = uniq('Два');
		await openEditor(page, info);
		await addText(page, a);
		await addText(page, b);
		await expect(item(page, `Текст · ${b}`)).toBeVisible();
		const menu = async (label: string, action: string | RegExp) => {
			await blocks(page)
				.getByRole('button', { name: `Действия: ${label}` })
				.first()
				.click();
			await page
				.getByRole(
					'menuitem',
					typeof action === 'string' ? { name: action, exact: true } : { name: action }
				)
				.last()
				.click();
		};
		const texts = async () => textsOf((await fetchTemplate(request, kind)).template);

		// «Дублировать»: a copy right after, and it is the one chosen.
		await menu(`Текст · ${a}`, 'Дублировать');
		await expect(item(page, `Текст · ${a}`)).toHaveCount(2);
		await expect.poll(async () => (await texts()).filter((t) => t === a).length).toBe(2);

		// «В колонку 1»: into the first column of a new columns block, and «Вынести из колонок» back.
		await addBlock(page, 'Колонки');
		// The standard act has columns of its own: the menu names the block, and the new one is the last.
		await menu(`Текст · ${b}`, /^В колонку 1 блока \d+$/);
		await expect
			.poll(async () => {
				const t = (await fetchTemplate(request, kind)).template;
				const cols = t.blocks.findLast((x) => x.body.type === 'columns')?.body;
				return cols?.type === 'columns' ? textsOf({ ...t, blocks: cols.columns[0] ?? [] }) : [];
			})
			.toEqual([b]);
		await menu(`Текст · ${b}`, 'Вынести из колонок');
		await expect
			.poll(async () => {
				const t = (await fetchTemplate(request, kind)).template;
				const cols = t.blocks.findLast((x) => x.body.type === 'columns')?.body;
				return cols?.type === 'columns' ? cols.columns.flat().length : -1;
			})
			.toBe(0);

		// «Удалить»: both copies of the first text go one by one.
		await menu(`Текст · ${a}`, 'Удалить');
		await expect(item(page, `Текст · ${a}`)).toHaveCount(1);
		await menu(`Текст · ${a}`, 'Удалить');
		await expect(item(page, `Текст · ${a}`)).toHaveCount(0);
		await expect.poll(async () => (await texts()).includes(a)).toBe(false);
		expect(await texts()).toContain(b);
	});

	test('sample_order_search', async ({ page, request }, info) => {
		const client = uniq('Образцов');
		const order = await createOrder(request, { clientName: client });
		await openEditor(page, info);
		const preview = page.getByRole('region', { name: 'Превью' });
		await expect(preview.getByRole('img', { name: /^Страница 1 из \d+$/ })).toBeVisible();
		const field = page.getByRole('combobox', { name: 'Заказ для примера' });
		const rendered = page.waitForRequest(
			(r) => r.url().includes('/preview') && (r.postData() ?? '').includes(`"order_id":${order.id}`)
		);
		await field.fill(client);
		await page.getByRole('option', { name: new RegExp(`№ ${order.id}\\b`) }).click();
		await rendered;
		await expect(field).toHaveValue(new RegExp(`№ ${order.id}\\b`));
		await expect(preview.getByRole('img', { name: /^Страница 1 из \d+$/ })).toBeVisible();

		// «Пример» again: the sample order, the orders found are not offered any more.
		await field.fill('Пр');
		await page.getByRole('option', { name: 'Пример' }).click();
		await expect(field).toHaveValue('Пример');
	});

	test('sidebar_keeps_settings_current', async ({ page }, info) => {
		await openEditor(page, info);
		await expect(
			page.getByRole('navigation').getByRole('link', { name: 'Настройки' })
		).toHaveAttribute('aria-current', 'page');
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
