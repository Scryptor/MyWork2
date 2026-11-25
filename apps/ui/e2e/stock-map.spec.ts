import {
	expect,
	test as base,
	type APIRequestContext,
	type Locator,
	type Page
} from '@playwright/test';
import type { DocumentTemplate } from '$api/DocumentTemplate';
import { ownOrigin } from './support/fresh';
import { uniq } from './support/api';

// «Склад → Карта» (plan 8.6) on a server of its own per engine (the racks and the template of the cell sticker
// are the base's own): a rack is added, a position is put into a cell, found by the code of the cell (in any
// case and in the Russian layout), the cell is opened on the map and its sticker previewed, a rack with parts
// is neither made smaller nor deleted, and the editor of the sticker offers the fields of a cell.
const test = base.extend({
	baseURL: async ({ browserName }, use) => use(ownOrigin('stockMap', browserName))
});

test.use({ screen: { width: 1280, height: 720 }, viewport: { width: 1280, height: 720 } });

/** The sticker is the template of its own kind: put back as it was made, whatever the test did. */
async function restoreSticker(request: APIRequestContext) {
	await expect(async () => {
		const current = (await (
			await request.get('/api/v1/document-templates/cell_sticker')
		).json()) as DocumentTemplate;
		const response = await request.put('/api/v1/document-templates/cell_sticker', {
			data: { template: current.standard, version: current.version }
		});
		expect(response.status(), await response.text()).toBe(200);
	}).toPass({ timeout: 15_000 });
}

const stockTabs = (page: Page) => page.getByRole('tablist');
const grid = (page: Page) => page.getByRole('grid', { name: 'Склад' });
const filter = (page: Page) => page.getByRole('searchbox', { name: 'Фильтр списка' });

/** The tab of the section «Склад» («Список» or «Карта»). */
async function openTab(page: Page, name: 'Список' | 'Карта') {
	const tab = stockTabs(page).getByRole('tab', { name, exact: true });
	await tab.click();
	await expect(tab).toHaveAttribute('aria-selected', 'true');
}

test.afterEach(({ request }) => restoreSticker(request));

test('places_finds_maps_and_prints_a_cell', async ({ page, request }) => {
	const digits = String(Math.floor(10000 + Math.random() * 90000));
	const code = `B${digits}`;
	const russian = `И${digits}`;
	const cell = `${code}-2-3`;
	const name = `Дисплей-${uniq()}`;
	const made = await request.post('/api/v1/stock', {
		data: {
			name,
			device_type: '',
			part_type: '',
			brand: '',
			model: '',
			color: '',
			note: '',
			quantity: 5,
			price: 100000,
			restock_threshold: 0
		}
	});
	expect(made.ok(), await made.text()).toBe(true);

	// 1. A rack: 3 shelves of 4 cells.
	await page.goto('/orders');
	await page
		.getByRole('navigation', { name: 'Разделы' })
		.getByRole('link', { name: /^Склад/ })
		.click();
	await expect(page).toHaveURL(/\/stock/);
	await openTab(page, 'Карта');
	const arrange = page.getByRole('switch', { name: 'Изменить расстановку' });
	// A base that has racks already (a server kept from an earlier run) shows the switch first.
	await expect(arrange.or(page.getByRole('button', { name: 'Добавить шкаф' }))).toBeVisible();
	if (await arrange.isVisible()) {
		await arrange.click();
		await expect(arrange).toBeChecked();
	}
	await page.getByRole('button', { name: 'Добавить шкаф' }).click();
	const rackDialog = page.getByRole('dialog', { name: 'Новый шкаф' });
	await rackDialog.getByRole('textbox', { name: 'Код', exact: true }).fill(code);
	await rackDialog.getByRole('textbox', { name: 'Подпись' }).fill('Дисплеи');
	await rackDialog.getByRole('textbox', { name: 'Полок' }).fill('3');
	await rackDialog.getByRole('textbox', { name: 'Ячеек на полке' }).fill('4');
	await rackDialog.getByRole('button', { name: 'Добавить', exact: true }).click();
	await expect(rackDialog).toBeHidden();
	const rackGroup = page.getByRole('group', { name: `Ячейки шкафа ${code}` });
	await expect(page.getByRole('heading', { name: `${code} · Дисплеи` })).toBeVisible();
	await expect(rackGroup.getByRole('button')).toHaveCount(12);

	// 2. The position is put into a cell; «Не разложено» is what is left.
	await openTab(page, 'Список');
	await filter(page).fill(name);
	const row = grid(page).getByRole('row').filter({ hasText: name });
	await expect(row).toHaveCount(1);
	await row.getByRole('gridcell', { name }).click();
	const panel = page.getByRole('dialog', { name });
	await expect(panel.getByText('Не разложено: 5')).toBeVisible();
	await panel.getByRole('button', { name: 'Положить…' }).click();
	const putDialog = page.getByRole('dialog', { name: 'Положить в ячейку' });
	await putDialog.getByRole('combobox', { name: 'Ячейка' }).fill(cell);
	await page.getByRole('option', { name: cell }).click();
	await putDialog.getByRole('textbox', { name: 'Количество' }).fill('3');
	await putDialog.getByRole('button', { name: 'Положить', exact: true }).click();
	await expect(putDialog).toBeHidden();
	await expect(panel.getByText(`${cell} · 3 шт.`)).toBeVisible();
	await expect(panel.getByText('Не разложено: 2')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(panel).toBeHidden();
	await expect(row.getByRole('gridcell', { name: cell, exact: true })).toBeVisible();

	// 3. The code of the cell in lower case is a place; «Искать как текст» takes it back to the text.
	await filter(page).fill(cell.toLowerCase());
	await expect(page.getByText(`Место ${cell}`)).toBeVisible();
	await expect(row).toHaveCount(1);
	await page.getByRole('button', { name: 'Искать как текст' }).click();
	await expect(page).toHaveURL(/text=1/);
	await expect(row).toHaveCount(0);
	await expect(page.getByText(`Место ${cell}`)).toHaveCount(0);

	// 4. The same in the Russian layout, typed into the field; a new text lets «Искать как текст» go.
	await filter(page).fill(`${russian}-2-3`);
	await expect(page.getByText(`Место ${cell}`)).toBeVisible();
	await expect(row).toHaveCount(1);
	await expect(page).not.toHaveURL(/text=1/);

	// 5. The map: the cell says it holds a position, its panel shows it, the sticker is previewed.
	await filter(page).fill('');
	await openTab(page, 'Карта');
	await page.getByRole('button', { name: `${cell}, 1 позиция`, exact: true }).click();
	const cellPanel = page.getByRole('dialog', { name: `Ячейка ${cell}` });
	await expect(cellPanel.getByText(name)).toBeVisible();
	await cellPanel.getByRole('button', { name: 'Печать наклейки' }).click();
	const preview = page.getByRole('dialog', { name: `Наклейка ячейки ${cell}`, exact: true });
	const sheet = preview.getByRole('img', { name: 'Страница 1 из 1' });
	await expect(sheet).toBeVisible();
	await expect(sheet).toHaveAttribute('src', /^blob:/);
	await expect(preview.getByRole('img', { name: /^Страница \d+ из / })).toHaveCount(1);
	await page.keyboard.press('Escape');
	await expect(preview).toBeHidden();
	await page.keyboard.press('Escape');
	await expect(cellPanel).toBeHidden();

	// 6. A rack with parts is neither made smaller nor deleted.
	const refusal = `В ячейке ${cell} лежат запчасти — переложите их`;
	await page.getByRole('switch', { name: 'Изменить расстановку' }).click();
	await page.getByRole('button', { name: `Изменить…: шкаф ${code}`, exact: true }).click();
	const edit = page.getByRole('dialog', { name: `Шкаф ${code}` });
	await edit.getByRole('textbox', { name: 'Полок' }).fill('1');
	await edit.getByRole('button', { name: 'Сохранить', exact: true }).click();
	await expect(edit.getByText(refusal)).toBeVisible();
	await edit.getByRole('button', { name: 'Отмена', exact: true }).click();
	await expect(edit).toBeHidden();

	await page.getByRole('button', { name: `Удалить: шкаф ${code}`, exact: true }).click();
	const confirm = page.getByRole('alertdialog');
	await confirm.getByRole('button', { name: 'Удалить', exact: true }).click();
	await expect(confirm).toBeHidden();
	await expect(page.getByText(refusal)).toBeVisible();
	await expect(page.getByRole('heading', { name: `${code} · Дисплеи` })).toBeVisible();

	// 7. The editor of the sticker offers the fields of a cell, not of an order.
	await page.goto('/templates/cell_sticker');
	const blocks = page.getByRole('region', { name: 'Блоки' });
	await blocks.getByRole('button', { name: 'Добавить блок' }).click();
	await page.getByRole('menuitem', { name: 'Текст', exact: true }).click();
	const props = page.getByRole('region', { name: 'Свойства' });
	await expect(props.getByRole('textbox', { name: 'Текст', exact: true })).toBeVisible();
	await props.getByRole('button', { name: 'Вставить поле' }).click();
	const field = props.getByRole('combobox', { name: 'Поле' });
	const options: Locator = page.getByRole('option');
	await field.fill('ячейки');
	await expect(options.filter({ hasText: 'Код ячейки' })).toHaveCount(1);
	await field.fill('номер');
	await expect(options.filter({ hasText: 'Номер заказа' })).toHaveCount(0);
});
