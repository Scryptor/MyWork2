import { expect, test, type Page } from '@playwright/test';

// The «Таблица» section shows a DataTable over makeOrders(10000): numbers 10001…20000, newest first,
// arriving in pages of 2 500 like from a server.
const NEWEST = 20000;
const OLDEST = 10001;
const PAGE = 2500;

const sectionOf = (page: Page) => page.locator('#table');
const gridOf = (page: Page) => sectionOf(page).getByRole('grid', { name: 'Заказы' });
const cellOf = (page: Page, n: number) =>
	gridOf(page).getByRole('gridcell', { name: String(n), exact: true });
const rowOf = (page: Page, n: number) =>
	gridOf(page)
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(n), exact: true }) });
const headerOf = (page: Page, name: string) =>
	gridOf(page).getByRole('columnheader', { name, exact: true });

async function headerTexts(page: Page): Promise<string[]> {
	const texts = await gridOf(page).getByRole('columnheader').allInnerTexts();
	return texts.map((t) => t.trim()).filter((t) => t !== '');
}

async function widthOf(page: Page, name: string): Promise<number> {
	return (await headerOf(page, name).boundingBox())?.width ?? 0;
}

test.describe('table', () => {
	test.beforeEach(async ({ page }) => {
		await page.goto('/design');
		await expect(cellOf(page, NEWEST)).toBeVisible();
		await gridOf(page).scrollIntoViewIfNeeded();
	});

	test('keeps fewer than 100 rows in the DOM', async ({ page }) => {
		const rows = await gridOf(page).getByRole('row').count();
		expect(rows).toBeGreaterThan(5);
		expect(rows).toBeLessThan(100);
		await expect(gridOf(page)).toHaveAttribute('aria-rowcount', String(PAGE + 1));
	});

	test('scrolling to the end loads the rest and shows the last number', async ({ page }) => {
		const grid = gridOf(page);
		await expect(async () => {
			await grid.evaluate((el) => el.scrollTo({ top: el.scrollHeight }));
			await expect(cellOf(page, OLDEST)).toBeVisible({ timeout: 500 });
		}).toPass({ timeout: 20_000 });
		await expect(cellOf(page, OLDEST)).toBeInViewport();
		await expect(grid).toHaveAttribute('aria-rowcount', String(NEWEST - OLDEST + 2));
		expect(await grid.getByRole('row').count()).toBeLessThan(100);
	});

	test('a click on a cell makes the row current (not selected) and opens it', async ({ page }) => {
		const grid = gridOf(page);
		const row = rowOf(page, NEWEST - 1);
		await row.getByRole('gridcell').nth(3).click();
		await expect(row).toHaveAttribute('aria-current', 'true');
		await expect(row).toHaveAttribute('aria-selected', 'false');
		await expect(rowOf(page, NEWEST)).not.toHaveAttribute('aria-current');
		await expect(grid.locator('[role="row"][aria-current="true"]')).toHaveCount(1);
		await expect(sectionOf(page).getByText(`Открыт № ${NEWEST - 1}`)).toBeVisible();
		await expect(grid).toBeFocused();
		const id = await row.getAttribute('id');
		await expect(grid).toHaveAttribute('aria-activedescendant', id ?? '');
	});

	test('↓ ↓ Enter opens the order two rows below and keeps the active row in view', async ({
		page
	}) => {
		const grid = gridOf(page);
		await cellOf(page, NEWEST).click();
		await expect(sectionOf(page).getByText(`Открыт № ${NEWEST}`)).toBeVisible();
		await expect(grid).toBeFocused();

		await page.keyboard.press('ArrowDown');
		await page.keyboard.press('ArrowDown');
		await page.keyboard.press('Enter');
		await expect(sectionOf(page).getByText(`Открыт № ${NEWEST - 2}`)).toBeVisible();
		const active = rowOf(page, NEWEST - 2);
		await expect(active).toHaveAttribute('aria-current', 'true');
		await expect(active).toBeInViewport();

		for (let i = 0; i < 3; i++) await page.keyboard.press('PageDown');
		const deep = grid.locator('[role="row"][aria-current="true"]');
		await expect(deep).toBeInViewport();
		const deepNumber = Number(await deep.getByRole('gridcell').nth(1).innerText());
		expect(deepNumber).toBeLessThan(NEWEST - 20);

		await page.keyboard.press('Home');
		await expect(rowOf(page, NEWEST)).toHaveAttribute('aria-current', 'true');
		await expect(rowOf(page, NEWEST)).toBeInViewport();

		await page.keyboard.press('ArrowUp');
		await expect(rowOf(page, NEWEST)).toHaveAttribute('aria-current', 'true');
		await expect(grid.locator('[role="row"][aria-selected="true"]')).toHaveCount(0);
	});

	test('Shift+click selects a range, the selection bar counts it and clears it', async ({
		page
	}) => {
		const section = sectionOf(page);
		const grid = gridOf(page);
		const checked = grid.getByRole('checkbox', { name: 'Выбрать строку', checked: true });
		const selectedRows = grid.locator('[role="row"][aria-selected="true"]');
		await expect(grid).toHaveAttribute('aria-multiselectable', 'true');

		await rowOf(page, NEWEST).getByRole('checkbox').click();
		await expect(section.getByText('Выбрано 1')).toBeVisible();
		await expect(rowOf(page, NEWEST)).toHaveAttribute('aria-selected', 'true');
		await rowOf(page, NEWEST - 3)
			.getByRole('checkbox')
			.click({ modifiers: ['Shift'] });
		await expect(section.getByText('Выбрано 4')).toBeVisible();
		await expect(checked).toHaveCount(4);
		await expect(selectedRows).toHaveCount(4);

		await section.getByRole('button', { name: 'Снять выделение' }).click();
		await expect(section.getByText(/^Выбрано/)).toHaveCount(0);
		await expect(checked).toHaveCount(0);
		await expect(selectedRows).toHaveCount(0);

		// Shift+click on a row ranges from the row opened last.
		await cellOf(page, NEWEST - 1).click();
		await cellOf(page, NEWEST - 5).click({ modifiers: ['Shift'] });
		await expect(section.getByText('Выбрано 5')).toBeVisible();
		await expect(selectedRows).toHaveCount(5);
		await expect(section.getByText(`Открыт № ${NEWEST - 1}`)).toBeVisible();
		await expect(rowOf(page, NEWEST - 1)).toHaveAttribute('aria-current', 'true');
	});

	test('Space selects the active row, the header checkbox selects all loaded rows', async ({
		page
	}) => {
		const section = sectionOf(page);
		const row = rowOf(page, NEWEST - 2);
		await cellOf(page, NEWEST - 2).click();
		await page.keyboard.press('Space');
		await expect(section.getByText('Выбрано 1')).toBeVisible();
		await expect(row.getByRole('checkbox')).toBeChecked();
		await expect(row).toHaveAttribute('aria-selected', 'true');
		await expect(row).toHaveAttribute('aria-current', 'true');
		await page.keyboard.press('Space');
		await expect(section.getByText(/^Выбрано/)).toHaveCount(0);
		await expect(row).toHaveAttribute('aria-selected', 'false');

		const all = gridOf(page).getByRole('checkbox', { name: 'Выбрать все видимые' });
		await all.click();
		await expect(section.getByText('Выбрано 2 500')).toBeVisible();
		await all.click();
		await expect(section.getByText(/^Выбрано/)).toHaveCount(0);
	});

	test('sorted by «Принят», newest first; a header click toggles the order', async ({ page }) => {
		const number = headerOf(page, '№');
		const received = headerOf(page, 'Принят');
		await expect(received).toHaveAttribute('aria-sort', 'descending');
		await expect(number).not.toHaveAttribute('aria-sort');

		await received.getByRole('button').click();
		await expect(received).toHaveAttribute('aria-sort', 'ascending');
		await expect(cellOf(page, OLDEST)).toBeVisible();

		await received.getByRole('button').click();
		await expect(received).toHaveAttribute('aria-sort', 'descending');
		await expect(cellOf(page, NEWEST)).toBeVisible();

		// Another column starts newest first too.
		await number.getByRole('button').click();
		await expect(number).toHaveAttribute('aria-sort', 'descending');
		await expect(received).not.toHaveAttribute('aria-sort');
	});

	test('a list sorted anew asks for its next page again', async ({ page }) => {
		const grid = gridOf(page);
		await grid.evaluate((el) => el.scrollTo({ top: el.scrollHeight }));
		await expect(grid).toHaveAttribute('aria-rowcount', String(2 * PAGE + 1));

		await headerOf(page, 'Принят').getByRole('button').click();
		await expect(grid).toHaveAttribute('aria-rowcount', String(PAGE + 1));
		await grid.evaluate((el) => el.scrollTo({ top: el.scrollHeight }));
		await expect(grid).toHaveAttribute('aria-rowcount', String(2 * PAGE + 1));
	});

	test('dragging the «Клиент» header edge by +80px survives a reload; double click resets', async ({
		page
	}) => {
		const before = await widthOf(page, 'Клиент');
		const box = (await headerOf(page, 'Клиент').boundingBox())!;
		const x = box.x + box.width - 2;
		const y = box.y + box.height / 2;
		await page.mouse.move(x, y);
		await page.mouse.down();
		await page.mouse.move(x + 40, y);
		await page.mouse.move(x + 80, y);
		await page.mouse.up();
		await expect.poll(() => widthOf(page, 'Клиент')).toBeGreaterThan(before + 78);
		expect(await widthOf(page, 'Клиент')).toBeLessThan(before + 82);
		const after = await widthOf(page, 'Клиент');

		await page.reload();
		await expect(headerOf(page, 'Клиент')).toBeVisible();
		await expect.poll(() => widthOf(page, 'Клиент')).toBe(after);

		const edge = (await headerOf(page, 'Клиент').boundingBox())!;
		await page.mouse.dblclick(edge.x + edge.width - 2, edge.y + edge.height / 2);
		await expect.poll(() => widthOf(page, 'Клиент')).toBe(before);
	});

	test('«Колонки»: a hidden column and a new order survive a reload', async ({ page }) => {
		await gridOf(page).getByRole('button', { name: 'Колонки' }).click();
		await page.getByRole('checkbox', { name: 'Мастер', exact: true }).uncheck();
		await expect(headerOf(page, 'Мастер')).toHaveCount(0);
		await page.getByRole('button', { name: 'Выше: Клиент' }).click();
		await expect(page.getByRole('button', { name: 'Выше: Клиент' })).toBeFocused();
		await page.keyboard.press('Escape');

		const expected = [
			'№',
			'Клиент',
			'Принят',
			'Устройство',
			'Неисправность',
			'Статус',
			'Сумма',
			'Отметки'
		];
		await expect.poll(() => headerTexts(page)).toEqual(expected);

		await page.reload();
		await expect(headerOf(page, 'Клиент')).toBeVisible();
		await expect(headerOf(page, 'Мастер')).toHaveCount(0);
		await expect.poll(() => headerTexts(page)).toEqual(expected);
	});

	test('rows follow the density, column widths the text size', async ({ page }) => {
		const height = async () => (await rowOf(page, NEWEST).boundingBox())?.height;
		await expect.poll(height).toBe(40);
		await page.getByRole('radio', { name: 'Компактно' }).check({ force: true });
		await expect.poll(height).toBe(32);

		const normal = await widthOf(page, 'Клиент');
		await page.getByRole('radio', { name: 'Крупный' }).check({ force: true });
		await expect.poll(height).toBe(40);
		await expect.poll(() => widthOf(page, 'Клиент')).toBe(Math.round(normal * 1.25));
	});

	test('a row changed elsewhere flashes briefly', async ({ page }) => {
		const row = rowOf(page, NEWEST);
		await expect(row).not.toHaveAttribute('data-flash');
		await sectionOf(page).getByRole('button', { name: 'Изменить на другом компьютере' }).click();
		await expect(row).toHaveAttribute('data-flash');
		await expect(row).not.toHaveAttribute('data-flash', { timeout: 3000 });
	});
});

test.describe('table on a phone', () => {
	test.use({ viewport: { width: 390, height: 800 } });

	test('390: list cards instead of the table, more cards at the end of the list', async ({
		page
	}) => {
		await page.goto('/design');
		const section = sectionOf(page);
		const first = section.getByRole('button', { name: new RegExp(`^№ ${NEWEST}\\b`) });
		await expect(first).toBeVisible();
		await expect(section.getByRole('grid')).toHaveCount(0);

		await first.click();
		await expect(section.getByText(`Открыт № ${NEWEST}`)).toBeVisible();
		await expect(first).toHaveAttribute('aria-current', 'true');

		const cards = section.getByRole('listitem');
		const before = await cards.count();
		expect(before).toBeGreaterThan(0);
		await cards.last().scrollIntoViewIfNeeded();
		await expect.poll(() => cards.count()).toBeGreaterThan(before);
	});
});
