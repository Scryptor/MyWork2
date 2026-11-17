import { expect, test, type Page } from '@playwright/test';
import type { DictionaryItem } from '$api/DictionaryItem';
import type { OrderPage } from '$api/OrderPage';
import {
	createOrder,
	fetchOrder,
	patchOrder,
	setStatus,
	statusId,
	uniq,
	uniqPhone
} from './support/api';

// The server on 4173 runs without sign-in: every right, so «+ своё» also fills the dictionaries. Its
// settings keep «intake_required_fields» on: the brand and the model are required too.

/** The modifier the app reads as Mod: ⌘ where navigator.platform says Mac, Ctrl elsewhere. */
async function mod(page: Page): Promise<'Meta' | 'Control'> {
	const mac = await page.evaluate(() => /Mac|iPhone|iPad/.test(navigator.platform));
	return mac ? 'Meta' : 'Control';
}

const sheetOf = (page: Page) => page.getByRole('dialog', { name: 'Новый заказ' });
const phoneOf = (page: Page) => sheetOf(page).getByRole('textbox', { name: 'Телефон' });
const nameOf = (page: Page) => sheetOf(page).getByRole('textbox', { name: 'ФИО' });
const typeOf = (page: Page) => sheetOf(page).getByRole('combobox', { name: /^Тип устройства/ });
const malfunctionOf = (page: Page) =>
	sheetOf(page).getByRole('textbox', { name: /^Неисправность/ });
const prepaymentOf = (page: Page) => sheetOf(page).getByRole('textbox', { name: 'Предоплата' });
const summaryOf = (page: Page) => sheetOf(page).getByRole('complementary', { name: 'Сводка' });
const toastsOf = (page: Page) => page.getByRole('status', { name: 'Уведомления' });
const panelOf = (page: Page, id: number) =>
	page.getByRole('dialog', { name: new RegExp(`^№ ${id}(?!\\d)`) });
const rowOf = (page: Page, id: number) =>
	page
		.getByRole('row')
		.filter({ has: page.getByRole('gridcell', { name: String(id), exact: true }) });

async function openOrders(page: Page) {
	await page.goto('/orders');
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
}

/** N on the orders screen: the sheet with «Телефон» focused. */
async function openIntake(page: Page) {
	await page.keyboard.press('n');
	await expect(sheetOf(page)).toBeVisible();
	await expect(phoneOf(page)).toBeFocused();
}

/** The device of an order that can be accepted: type, brand and model. */
async function fillDevice(page: Page, type = 'Телефон') {
	await typeOf(page).fill(type);
	await sheetOf(page)
		.getByRole('combobox', { name: /^Бренд/ })
		.fill('Тест');
	await sheetOf(page)
		.getByRole('combobox', { name: /^Модель/ })
		.fill('Модель 1');
}

/**
 * The device and «Неисправность» field by field with a click, as a person moves: each click closes the
 * suggestions of the field before, so Esc is left for the sheet.
 */
async function fillByHand(page: Page, malfunction: string) {
	const fields: [RegExp, string][] = [
		[/^Тип устройства/, 'Телефон'],
		[/^Бренд/, 'Тест'],
		[/^Модель/, 'Модель 1']
	];
	for (const [name, value] of fields) {
		const box = sheetOf(page).getByRole('combobox', { name });
		await box.click();
		await box.fill(value);
	}
	await malfunctionOf(page).click();
	await expect(sheetOf(page).getByRole('combobox', { expanded: true })).toHaveCount(0);
	await malfunctionOf(page).fill(malfunction);
}

/** «Принят № N» of the toast. */
async function acceptedNumber(page: Page): Promise<number> {
	const text = toastsOf(page).getByText(/^Принят № \d+$/);
	await expect(text).toBeVisible();
	return Number((await text.textContent())?.replace(/\D/g, ''));
}

test.beforeEach(async ({ page }) => {
	await openOrders(page);
});

test('N opens intake, a new client order appears in the list', async ({ page, request }) => {
	const name = `Тестов ${uniq('П')}`;
	const phone = uniqPhone();
	const chip = uniq('Чехол');
	await openIntake(page);
	// min(70rem, 100vw − 4rem) at 1280px: 70rem.
	await expect.poll(async () => (await sheetOf(page).boundingBox())?.width).toBe(1120);
	await phoneOf(page).fill(phone);
	await nameOf(page).fill(name);
	await fillDevice(page);
	await malfunctionOf(page).fill('Не заряжается');
	// «+ своё» in «Комплектность»: into the order and the dictionary.
	const completeness = sheetOf(page).getByRole('group', { name: 'Комплектность' });
	await completeness.getByRole('button', { name: '+ своё' }).click();
	await completeness.getByRole('textbox', { name: '+ своё' }).fill(chip);
	await page.keyboard.press('Enter');
	await expect(completeness.getByRole('button', { name: chip })).toHaveAttribute(
		'aria-pressed',
		'true'
	);
	await expect(summaryOf(page)).toContainText('Всё обязательное заполнено');
	// Without sign-in nobody is named as the receiver.
	await expect(sheetOf(page).getByText(/^Приёмщик/)).toHaveCount(0);
	await page.keyboard.press(`${await mod(page)}+Enter`);

	await expect(sheetOf(page)).toBeHidden();
	const id = await acceptedNumber(page);
	await expect(rowOf(page, id)).toContainText(name);
	await toastsOf(page).getByRole('button', { name: 'Открыть' }).click();
	await expect(panelOf(page, id)).toContainText(name);

	const order = await fetchOrder(request, id);
	expect(order.completeness).toBe(chip);
	expect(order.malfunction).toBe('Не заряжается');
	const dictionary = (await (
		await request.get('/api/v1/dictionaries/completeness')
	).json()) as DictionaryItem[];
	expect(dictionary.map((d) => d.value)).toContain(chip);

	// Accepted: the next intake starts empty.
	await page.keyboard.press('Escape');
	await expect(panelOf(page, id)).toBeHidden();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
	await expect(phoneOf(page)).toHaveValue('');
});

test('an existing client is found by a second phone', async ({ page, request }) => {
	const name = `Тестов ${uniq('В')}`;
	const [first, second] = [uniqPhone(), uniqPhone()];
	const past = await createOrder(request, {
		client: {
			new: {
				full_name: name,
				phones: [
					{ phone: first, note: '' },
					{ phone: second, note: 'жена' }
				],
				address: '',
				note: 'Тестовая заметка',
				source: '',
				blacklisted: false
			}
		},
		model: uniq('M')
	});
	await openIntake(page);
	await page.keyboard.type(second);
	const card = sheetOf(page)
		.getByRole('list', { name: 'Найденные клиенты' })
		.getByRole('listitem')
		.filter({ hasText: name });
	const shown = (phone: string) =>
		`+7 ${phone.slice(2, 5)} ${phone.slice(5, 8)}-${phone.slice(8, 10)}-${phone.slice(10)}`;
	await expect(card).toContainText(`${shown(first)} · основной`);
	await expect(card).toContainText(`${shown(second)} · жена`);
	await expect(card).toContainText('1 заказ');
	await expect(card).toContainText('Тестовая заметка');
	// A client found: no fields of a new one until «Новый клиент».
	await expect(nameOf(page)).toHaveCount(0);
	await card.getByRole('button', { name: 'Это он' }).click();

	const chosen = sheetOf(page).getByRole('region', { name: 'Клиент заказа' });
	await expect(chosen).toContainText(name);
	const pastOrders = summaryOf(page).getByRole('list', { name: 'Прошлые заказы' });
	await expect(pastOrders).toContainText(`№ ${past.id}`);

	await fillDevice(page, 'Ноутбук');
	await malfunctionOf(page).fill('Не включается');
	await sheetOf(page)
		.getByRole('button', { name: /^Принять/ })
		.click();
	const id = await acceptedNumber(page);
	expect((await fetchOrder(request, id)).client_id).toBe(past.client_id);
});

test('«Новый клиент» opens the form with the typed phone', async ({ page, request }) => {
	const phone = uniqPhone();
	await createOrder(request, { phone, model: uniq('M') });
	await openIntake(page);
	await phoneOf(page).fill(phone);
	const found = sheetOf(page).getByRole('list', { name: 'Найденные клиенты' });
	await expect(found.getByRole('listitem')).toHaveCount(1);
	await sheetOf(page).getByRole('button', { name: 'Новый клиент' }).click();
	await expect(found).toHaveCount(0);
	await expect(nameOf(page)).toBeVisible();
	await expect(phoneOf(page)).toHaveValue(new RegExp(`${phone.slice(-2)}$`));
});

test('draft survives a reload', async ({ page }) => {
	await openIntake(page);
	await phoneOf(page).fill('70001234567');
	await malfunctionOf(page).fill('Разбит экран');
	await prepaymentOf(page).click();
	// An amount typed right before Esc is kept too.
	await page.keyboard.type('1500');
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();

	await page.reload();
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
	await page.keyboard.press('n');
	await expect(malfunctionOf(page)).toHaveValue('Разбит экран');
	await expect(phoneOf(page)).toHaveValue('+7 000 123-45-67');
	await expect(prepaymentOf(page)).toHaveValue(/^1\s500$/);

	// «Отмена» drops the draft.
	await sheetOf(page).getByRole('button', { name: 'Отмена' }).click();
	await expect(sheetOf(page)).toBeHidden();
	await page.reload();
	await expect(page.getByRole('heading', { name: 'Заказы', level: 1 })).toBeVisible();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
});

test('missing required fields block «Принять» and are listed', async ({ page }) => {
	await openIntake(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeVisible();
	await expect(summaryOf(page)).toContainText(
		'Не заполнено: Телефон или ФИО, Тип устройства, Бренд, Модель, Неисправность'
	);
	await expect(phoneOf(page)).toHaveAttribute('aria-invalid', 'true');
	await expect(typeOf(page)).toHaveAttribute('aria-invalid', 'true');
	await expect(malfunctionOf(page)).toHaveAttribute('aria-invalid', 'true');
	await expect(phoneOf(page)).toBeFocused();

	// The button says the same and takes focus to the first field left.
	await phoneOf(page).fill(uniqPhone());
	await sheetOf(page)
		.getByRole('button', { name: /^Принять/ })
		.click();
	await expect(typeOf(page)).toBeFocused();
	await expect(summaryOf(page)).toContainText(
		'Не заполнено: Тип устройства, Бренд, Модель, Неисправность'
	);
	await expect(toastsOf(page)).not.toContainText('Принят');
});

test('a wrong amount blocks «Принять» until fixed', async ({ page }) => {
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillDevice(page);
	await malfunctionOf(page).fill('Не включается');
	const prepayment = prepaymentOf(page);
	await prepayment.fill('15O0');
	await malfunctionOf(page).focus();
	await expect(summaryOf(page)).toContainText('Неверная сумма: Предоплата');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeVisible();
	await expect(prepayment).toBeFocused();
	await expect(prepayment).toHaveAttribute('aria-invalid', 'true');

	await prepayment.fill('1500');
	await expect(summaryOf(page).getByText(/^Неверная сумма/)).toHaveCount(0);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeHidden();
	const order = await fetchOrder(page.request, await acceptedNumber(page));
	expect(order.prepayment).toBe(150_000);
});

test('no connection: «Принять» keeps the draft and says why above the button', async ({ page }) => {
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillDevice(page);
	await malfunctionOf(page).fill('Не включается');
	await page.route('**/api/v1/orders', (route) =>
		route.request().method() === 'POST' ? route.abort() : route.fallback()
	);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByRole('alert')).toContainText('нет связи с сервером');
	await expect(malfunctionOf(page)).toHaveValue('Не включается');

	await page.unroute('**/api/v1/orders');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeHidden();
	await acceptedNumber(page);
});

test('Esc while «Принять» is on its way and N again: one order, the reopened sheet keeps its text', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	// The server gets the order only when the test lets it go.
	let letGo: () => void = () => {};
	const held = new Promise<void>((resolve) => (letGo = resolve));
	let posts = 0;
	await page.route('**/api/v1/orders', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		posts += 1;
		await held;
		await route.fallback();
	});
	const answered = page.waitForResponse(
		(r) => r.request().method() === 'POST' && new URL(r.url()).pathname === '/api/v1/orders'
	);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByText('Принимаю…')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();

	// Opened again, the sheet still waits for that order: «Принять» sends nothing.
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await expect(sheetOf(page).getByText('Принимаю…')).toBeVisible();
	await page.keyboard.press(`${await mod(page)}+Enter`);
	const comment = sheetOf(page).getByRole('textbox', { name: 'Комментарий (только для сервиса)' });
	await comment.fill('Позвонить вечером');
	letGo();
	await answered;

	// The answer is about the earlier opening: this one stays as it is and says what happened.
	await expect(sheetOf(page).getByText(/^Принят № \d+\./)).toBeVisible();
	await expect(sheetOf(page).getByText('Принимаю…')).toHaveCount(0);
	await expect(sheetOf(page)).toBeVisible();
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await expect(comment).toHaveValue('Позвонить вечером');
	expect(posts).toBe(1);
	const found = (await (
		await request.get(`/api/v1/orders?query=${encodeURIComponent(marker)}`)
	).json()) as OrderPage;
	expect(found.total).toBe(1);

	// «Отмена» clears it for the next client.
	await sheetOf(page).getByRole('button', { name: 'Отмена' }).click();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
});

test('Esc while «Принять» is on its way: the accepted draft is not offered again', async ({
	page,
	request
}) => {
	const marker = uniq('Не заряжается');
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	let letGo: () => void = () => {};
	const held = new Promise<void>((resolve) => (letGo = resolve));
	await page.route('**/api/v1/orders', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		await held;
		await route.fallback();
	});
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	letGo();
	const id = await acceptedNumber(page);
	expect((await fetchOrder(request, id)).malfunction).toBe(marker);
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
});

test('«Этот IMEI уже был» marks a past order under warranty', async ({ page, request }) => {
	const serial = uniq('IMEI');
	const past = await createOrder(request, {
		serial,
		master: 'Тестов',
		warranty: '30 дней',
		model: uniq('M')
	});
	await patchOrder(request, past.id, { work_done: 'Замена разъёма' });
	await setStatus(request, past.id, await statusId(request, 'Выдан'));

	await openIntake(page);
	await sheetOf(page).getByRole('textbox', { name: 'IMEI или серийный номер' }).fill(serial);
	const seen = summaryOf(page).getByRole('list', { name: 'Этот IMEI уже был' });
	await expect(seen).toContainText(`№ ${past.id}`);
	await expect(seen).toContainText(/на гарантии до \d{2}\.\d{2}\.\d{4}/);
	// A click opens the order; the draft waits for the next N.
	await seen.getByRole('button', { name: new RegExp(`№ ${past.id}(?!\\d)`) }).click();
	await expect(sheetOf(page)).toBeHidden();
	await expect(panelOf(page, past.id)).toBeVisible();
});

test('typing right after opening keeps the caret in WebKit', async ({ page }) => {
	await page.keyboard.press('n');
	await expect(sheetOf(page)).toBeVisible();
	await page.keyboard.type('8921');
	await expect(phoneOf(page)).toHaveValue('8921');
});
