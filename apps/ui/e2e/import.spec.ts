import fs from 'node:fs/promises';
import path from 'node:path';
import { expect, test as base, type Page } from '@playwright/test';
import type { FolderCheck } from '$api/FolderCheck';
import type { ServerInfo } from '$api/ServerInfo';
import { freshOrigin } from './support/fresh';
import {
	emptyFolder,
	legacyFolder,
	MW2_ADMIN_LOGIN,
	MW2_ADMIN_PASSWORD,
	type LegacyFolder
} from './support/legacy';

// The transfer from MyWork 2 (spec 6.9) on a new base of its own (`--fresh`, one per engine). The folder is
// synthetic (`make_fixture`); its base has users, so once the transfer is written the program asks to sign in.
// The tests go in this order: a wrong folder on the new base, the transfer, then «Настройки» on its result.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(freshOrigin('import', browserName))
});
test.describe.configure({ mode: 'serial' });

let folder: LegacyFolder;
test.beforeAll(async () => {
	folder = await legacyFolder();
});
test.afterAll(async () => {
	await folder?.remove();
});

const STAGES = ['Чтение базы', 'Разбор', 'Копирование файлов', 'Запись', 'Установка файлов'];
const SIGN_IN =
	'Перенос записан. Чтобы увидеть отчёт, войдите с логином и паролем из MyWork 2: логин — имя пользователя, которое выбиралось из списка в MyWork 2';

const pathField = (page: Page) => page.getByRole('textbox', { name: 'Путь к папке MyWork 2' });

async function signIn(page: Page) {
	await page.getByLabel('Логин').fill(MW2_ADMIN_LOGIN);
	await page.getByLabel('Пароль').fill(MW2_ADMIN_PASSWORD);
	await page.getByRole('button', { name: 'Войти' }).click();
}

/** The progress went through to the written base; «Войти» leads to the sign-in and then to the report. */
async function signInForReport(page: Page): Promise<number> {
	const steps = page.getByRole('list', { name: 'Ход переноса' });
	await expect(steps.getByRole('listitem')).toHaveText(STAGES.map((s) => new RegExp(s)));
	await expect(page.getByText(SIGN_IN)).toBeVisible();
	await page.getByRole('button', { name: 'Войти' }).click();
	await expect(page).toHaveURL(/\/login\?next=%2Fsettings%2Fimport%2Flatest$/);
	await signIn(page);
	await expect(page).toHaveURL(/\/settings\/import\/\d+$/);
	return Number(new URL(page.url()).pathname.split('/').at(-1));
}

test('a wrong folder: «Проверить» says what is wrong, and nothing is transferred', async ({
	page,
	request
}) => {
	const empty = await emptyFolder();
	try {
		await page.goto('/');
		await expect(page).toHaveURL(/\/setup$/);
		await page.getByRole('button', { name: /Перенести из MyWork 2/ }).click();
		const check = page.getByRole('button', { name: 'Проверить' });

		await pathField(page).fill(path.join(empty.path, 'нет такой'));
		await check.click();
		await expect(page.getByRole('alert')).toContainText('Папка не найдена');
		await expect(page.getByRole('button', { name: 'Перенести' })).toHaveCount(0);

		await pathField(page).fill(empty.path);
		await check.click();
		await expect(page.getByRole('alert')).toContainText('В папке нет baza.sqlite');
		await expect(page.getByRole('button', { name: 'Перенести' })).toHaveCount(0);

		// A changed path forgets the old answer.
		await pathField(page).fill(folder.path);
		await expect(page.getByRole('alert')).toHaveCount(0);

		const info = (await (await request.get('/api/v1/server-info')).json()) as ServerInfo;
		expect(info.setup_required).toBe(true);
	} finally {
		await empty.remove();
	}
});

test('transfer: what was found, the progress, sign-in as a user of MyWork 2, the report and an order of a group', async ({
	page,
	request
}) => {
	// What the server finds there: the screen must show the same.
	const found = (await (
		await request.post('/api/v1/legacy-import/check', { data: { path: folder.path } })
	).json()) as FolderCheck;
	expect(found.ok).toBe(true);

	await page.goto('/');
	await expect(page).toHaveURL(/\/setup$/);
	await page.getByRole('button', { name: /Перенести из MyWork 2/ }).click();
	await pathField(page).fill(folder.path);
	await page.getByRole('button', { name: 'Проверить' }).click();
	const result = page.getByRole('region', { name: 'Что нашлось' });
	await expect(result.getByRole('definition')).toHaveText([
		found.schema_version!,
		String(found.orders),
		String(found.clients),
		new RegExp(`^${found.files} \\(.+\\)$`)
	]);

	await page.getByRole('button', { name: 'Перенести' }).click();
	const id = await signInForReport(page);

	// The report: the totals, the groups that need attention, the orders of a group.
	await expect(page.getByRole('heading', { name: 'Отчёт о переносе' })).toBeVisible();
	const totals = page.getByRole('list', { name: 'Итоги' });
	await expect(totals.getByRole('listitem').filter({ hasText: 'Заказов' })).toContainText(
		String(found.orders)
	);
	await expect(page.getByRole('heading', { name: 'Требует внимания' })).toBeVisible();
	const dates = page.getByRole('button', { name: /Не разобрана дата/ });
	await expect(dates).toHaveAttribute('aria-expanded', 'false');
	await dates.click();
	await expect(dates).toHaveAttribute('aria-expanded', 'true');
	const first = page
		.getByRole('list', { name: 'Заказы: Не разобрана дата' })
		.getByRole('button')
		.first();
	const number = Number((await first.textContent())!.replace(/\D/g, ''));
	expect(number).toBeGreaterThan(0);
	await first.click();
	const panel = page.getByRole('dialog', { name: new RegExp(`^№ ${number}(?!\\d)`) });
	await expect(panel).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(panel).toBeHidden();

	// «Сохранить отчёт»: the text of the report as a file.
	const saving = page.waitForEvent('download');
	await page.getByRole('button', { name: 'Сохранить отчёт' }).click();
	const download = await saving;
	expect(download.suggestedFilename()).toMatch(/^mywork2-import-\d{4}-\d{2}-\d{2}-\d{4}\.txt$/);
	const text = await fs.readFile((await download.path())!, 'utf8');
	expect(text).toContain('Не разобрана дата');

	await page.getByRole('link', { name: 'К заказам' }).click();
	await expect(page).toHaveURL(/\/orders$/);
	await expect(page.getByRole('heading', { name: 'Первый запуск' })).toHaveCount(0);
	expect(id).toBeGreaterThan(0);
});

test('«Настройки → Импорт из MyWork 2»: the past import with its report; «Перенести снова» replaces the data', async ({
	page
}) => {
	await page.goto('/settings/import');
	await expect(page).toHaveURL(/\/login\?next=%2Fsettings%2Fimport$/);
	await signIn(page);
	await expect(page.getByRole('heading', { name: 'Импорт из MyWork 2' })).toBeVisible();
	const past = page.getByRole('list', { name: 'Прошлые импорты' }).getByRole('listitem');
	await expect(past).toHaveCount(1);
	await expect(past.first()).toContainText(folder.path);
	await expect(past.first()).toContainText('Заказов: 11');
	await past.first().getByRole('link').click();
	await expect(page).toHaveURL(/\/settings\/import\/\d+$/);
	await expect(page.getByRole('heading', { name: 'Отчёт о переносе' })).toBeVisible();

	await page.goto('/settings/import');
	await page.getByRole('button', { name: 'Перенести снова' }).click();
	await pathField(page).fill(folder.path);
	await page.getByRole('button', { name: 'Проверить' }).click();
	await expect(page.getByRole('region', { name: 'Что нашлось' })).toBeVisible();
	await page.getByRole('button', { name: 'Перенести' }).click();
	const confirm = page.getByRole('alertdialog', { name: 'Заменить данные?' });
	await expect(confirm).toContainText('Пользователи и пароли станут такими, как в MyWork 2');
	// The folder of the copy is the server's own: `<data folder>/backups`.
	await expect(confirm).toContainText(/Нынешние данные сохранятся копией в папке \S+backups\./);
	await confirm.getByRole('button', { name: 'Заменить' }).click();

	// The replacement closed every session, this one too.
	await signInForReport(page);
	await expect(page.getByRole('heading', { name: 'Отчёт о переносе' })).toBeVisible();
	await expect(page.getByRole('main')).toContainText('с заменой');
	// The replaced base keeps no record of the imports before it.
	await page.goto('/settings/import');
	const now = page.getByRole('list', { name: 'Прошлые импорты' }).getByRole('listitem');
	await expect(now).toHaveCount(1);
	await expect(now.first()).toContainText('с заменой');
});
