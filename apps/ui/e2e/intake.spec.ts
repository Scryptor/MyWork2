import { expect, test, type APIRequestContext, type Page } from '@playwright/test';
import type { DictionaryItem } from '$api/DictionaryItem';
import type { OrderPage } from '$api/OrderPage';
import {
	createOrder,
	deleteOrder,
	fetchOrder,
	patchOrder,
	setStatus,
	statusId,
	tinyPng,
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
const photoZoneOf = (page: Page) => sheetOf(page).getByRole('group', { name: 'Область для фото' });
const photoTilesOf = (page: Page) =>
	sheetOf(page)
		.getByRole('group', { name: 'Фото устройства' })
		.getByRole('button', { name: /^Открыть фото/ });
const png = (name: string, seed: number) => ({
	name,
	mimeType: 'image/png',
	buffer: tinyPng(seed)
});

/** Ctrl/⌘+V of a picture: the paste event the browser sends, with the file in its clipboard data. */
async function pastePicture(page: Page, seed: number) {
	await page.evaluate(
		(bytes) => {
			const data = new DataTransfer();
			data.items.add(new File([new Uint8Array(bytes)], 'pasted.png', { type: 'image/png' }));
			(document.activeElement ?? document.body).dispatchEvent(
				new ClipboardEvent('paste', { clipboardData: data, bubbles: true, cancelable: true })
			);
		},
		[...tinyPng(seed)]
	);
}
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
	// The form was inert while the order was on its way: the field it was sent from has focus again.
	await expect(malfunctionOf(page)).toBeFocused();
	// The message is about what was sent: any edit hides it.
	await malfunctionOf(page).fill('Не включается совсем');
	await expect(sheetOf(page).getByRole('alert')).toHaveCount(0);

	await page.unroute('**/api/v1/orders');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeHidden();
	await acceptedNumber(page);
});

/** POST of a new order waits until the test lets it go; the count of those sent. */
async function holdOrders(page: Page) {
	let letGo: () => void = () => {};
	const held = new Promise<void>((resolve) => (letGo = resolve));
	let posts = 0;
	await page.route('**/api/v1/orders', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		posts += 1;
		await held;
		// The page may have called it off meanwhile.
		await route.fallback().catch(() => {});
	});
	const answered = page.waitForResponse(
		(r) => r.request().method() === 'POST' && new URL(r.url()).pathname === '/api/v1/orders'
	);
	// A test that calls the order off never sees the answer.
	answered.catch(() => {});
	return { letGo, answered, posts: () => posts };
}

/** A filled sheet whose «Принять» is on its way, then Esc and N: the sheet waits for that order. */
async function acceptEscReopen(page: Page, marker: string) {
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	const held = await holdOrders(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByText('Принимаю…')).toBeVisible();
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await expect(sheetOf(page).getByText('Принимаю…')).toBeVisible();
	// «Принять» sends nothing while that order is on its way.
	await page.keyboard.press(`${await mod(page)}+Enter`);
	return held;
}

async function ordersWith(request: APIRequestContext, marker: string): Promise<number> {
	const found = (await (
		await request.get(`/api/v1/orders?query=${encodeURIComponent(marker)}`)
	).json()) as OrderPage;
	return found.total;
}

const commentOf = (page: Page) =>
	sheetOf(page).getByRole('textbox', { name: 'Комментарий (только для сервиса)' });
const acceptedNote = (page: Page, id?: number) =>
	sheetOf(page).getByText(
		id === undefined ? /^Заказ № \d+ уже принят\./ : new RegExp(`^Заказ № ${id} уже принят\\.`)
	);

test('Esc and N while «Принять» is on its way, nothing changed: the answer closes the sheet, one order', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const held = await acceptEscReopen(page, marker);
	held.letGo();
	await held.answered;
	await expect(sheetOf(page)).toBeHidden();
	await acceptedNumber(page);
	// The accepted draft is gone: the next intake is empty, «Принять» asks for the fields.
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(summaryOf(page)).toContainText('Не заполнено');
	expect(held.posts()).toBe(1);
	expect(await ordersWith(request, marker)).toBe(1);
});

test('Esc and N while «Принять» is on its way, then an edit: the draft stays and says it is accepted', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const held = await acceptEscReopen(page, marker);
	await commentOf(page).fill('Позвонить вечером');
	held.letGo();
	await held.answered;

	// The answer is about the earlier opening: this one stays as it is and says what happened.
	await expect(acceptedNote(page)).toBeVisible();
	await expect(sheetOf(page).getByText('Принимаю…')).toHaveCount(0);
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await expect(commentOf(page)).toHaveValue('Позвонить вечером');
	// It still says so after Esc and N.
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	await openIntake(page);
	await expect(acceptedNote(page)).toBeVisible();
	await expect(commentOf(page)).toHaveValue('Позвонить вечером');
	expect(held.posts()).toBe(1);
	expect(await ordersWith(request, marker)).toBe(1);

	// «Отмена» clears it for the next client.
	await sheetOf(page).getByRole('button', { name: 'Отмена' }).click();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
	await expect(acceptedNote(page)).toHaveCount(0);
});

test('an edit after N, then Esc before the answer: the next N says the draft is accepted', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const held = await acceptEscReopen(page, marker);
	// The name edited (5.1 lost the mark then): still the same attempt, the mark holds.
	await nameOf(page).fill('Тестов Пётр');
	await commentOf(page).fill('Позвонить вечером');
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	held.letGo();
	const id = await acceptedNumber(page);
	await openIntake(page);
	await expect(acceptedNote(page, id)).toBeVisible();
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await expect(nameOf(page)).toHaveValue('Тестов Пётр');
	await expect(commentOf(page)).toHaveValue('Позвонить вечером');
	expect(await ordersWith(request, marker)).toBe(1);

	// «Принять» of this draft is the same attempt: the same order, not a second one, and the sheet keeps
	// the changes it does not have.
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByText('Принимаю…')).toHaveCount(0);
	await expect(acceptedNote(page, id)).toBeVisible();
	await expect(nameOf(page)).toHaveValue('Тестов Пётр');
	expect(await ordersWith(request, marker)).toBe(1);
	await sheetOf(page).getByRole('button', { name: 'Отмена' }).click();
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
	await expect(acceptedNote(page)).toHaveCount(0);
});

test('Esc while «Принять» is on its way: the accepted draft is not offered again', async ({
	page,
	request
}) => {
	const marker = uniq('Не заряжается');
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	const held = await holdOrders(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	held.letGo();
	const id = await acceptedNumber(page);
	expect((await fetchOrder(request, id)).malfunction).toBe(marker);
	await openIntake(page);
	await expect(malfunctionOf(page)).toHaveValue('');
});

test('no answer to «Принять» in 30 s: the sheet says so and can be used again', async ({
	page
}) => {
	const marker = uniq('Не включается');
	await page.clock.install();
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	const held = await holdOrders(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByText('Принимаю…')).toBeVisible();
	await expect(sheetOf(page).getByRole('button', { name: 'Отмена' })).toBeDisabled();
	await page.clock.fastForward(30_000);
	await expect(sheetOf(page).getByRole('alert')).toHaveText(
		'Не удалось принять: нет ответа от сервера. Введённое сохранено — нажмите «Принять» ещё раз, второго заказа не будет'
	);
	await expect(sheetOf(page).getByText('Принимаю…')).toHaveCount(0);
	await expect(sheetOf(page).getByRole('button', { name: 'Отмена' })).toBeEnabled();
	await expect(malfunctionOf(page)).toHaveValue(marker);
	held.letGo();
});

/**
 * The first POST of a new order reaches the server, which makes the order, but its answer comes only
 * after the page has stopped waiting; the next ones go as usual. The count of those sent.
 */
async function answerLate(page: Page) {
	let letGo: () => void = () => {};
	const held = new Promise<void>((resolve) => (letGo = resolve));
	let posts = 0;
	await page.route('**/api/v1/orders', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		posts += 1;
		if (posts > 1) return route.fallback();
		const response = await route.fetch();
		await held;
		// The page has called it off meanwhile.
		await route.fulfill({ response }).catch(() => {});
	});
	return { letGo, posts: () => posts };
}

/** A filled sheet whose «Принять» got no answer in 30 s, although the server made the order. */
async function acceptWithoutAnswer(page: Page, request: APIRequestContext, marker: string) {
	await page.clock.install();
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	const late = await answerLate(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect.poll(() => ordersWith(request, marker)).toBe(1);
	await page.clock.fastForward(30_000);
	await expect(sheetOf(page).getByRole('alert')).toContainText('нет ответа от сервера');
	return late;
}

test('«Принять» again after no answer gets the order the server made, not a second one', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const late = await acceptWithoutAnswer(page, request, marker);
	// Until it is known whether the order is there, photos would not go to it: none are taken.
	await expect(sheetOf(page).getByText(/^Пока неясно, принят ли заказ/)).toBeVisible();
	await expect(sheetOf(page).getByRole('button', { name: 'Снять камерой' })).toBeDisabled();
	await expect(sheetOf(page).getByRole('button', { name: 'Выбрать файлы' })).toBeDisabled();
	await pastePicture(page, 7);
	await expect(sheetOf(page).getByText('0 из 10')).toBeVisible();
	await expect(sheetOf(page).getByText('Загружаю…')).toHaveCount(0);

	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeHidden();
	const id = await acceptedNumber(page);
	late.letGo();
	expect(late.posts()).toBe(2);
	expect(await ordersWith(request, marker)).toBe(1);
	expect((await fetchOrder(request, id)).malfunction).toBe(marker);
});

test('«Принять» again after no answer and an edit: the order is that of the first attempt, the sheet says so', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const late = await acceptWithoutAnswer(page, request, marker);
	await commentOf(page).fill('Позвонить вечером');
	await page.keyboard.press(`${await mod(page)}+Enter`);
	const note = acceptedNote(page);
	await expect(note).toHaveText(
		/^Заказ № \d+ уже принят\. Изменения после первой попытки в него не попали — откройте заказ и внесите их там/
	);
	await expect(commentOf(page)).toHaveValue('Позвонить вечером');
	late.letGo();
	expect(await ordersWith(request, marker)).toBe(1);
	const id = Number((await note.textContent())?.match(/№ (\d+)/)?.[1]);
	expect((await fetchOrder(request, id)).internal_comment).toBe('');
	// «Открыть»: the order, where the changes go.
	await sheetOf(page).getByRole('button', { name: 'Открыть' }).click();
	await expect(sheetOf(page)).toBeHidden();
	await expect(panelOf(page, id)).toBeVisible();
});

test('no answer for a new client, Esc, N: the client that attempt made is found, «Это он» keeps it one order', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const name = `Тестов ${uniq('Н')}`;
	await page.clock.install();
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await nameOf(page).fill(name);
	await fillByHand(page, marker);
	const late = await answerLate(page);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect.poll(() => ordersWith(request, marker)).toBe(1);
	await page.clock.fastForward(30_000);
	await expect(sheetOf(page).getByRole('alert')).toContainText('нет ответа от сервера');
	// Esc, a look at the list, N: the phone finds the client the first attempt made.
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	late.letGo();
	await openIntake(page);
	const card = sheetOf(page)
		.getByRole('list', { name: 'Найденные клиенты' })
		.getByRole('listitem')
		.filter({ hasText: name });
	await card.getByRole('button', { name: 'Это он' }).click();
	await page.keyboard.press(`${await mod(page)}+Enter`);
	// The same attempt: the order of the first one, not a second order.
	await expect(acceptedNote(page).or(toastsOf(page).getByText(/^Принят № \d+$/))).toBeVisible();
	expect(await ordersWith(request, marker)).toBe(1);
});

test('the order of the attempt was deleted meanwhile: «Принять» says so, the next one is a new order', async ({
	page,
	request
}) => {
	const marker = uniq('Не включается');
	const late = await acceptWithoutAnswer(page, request, marker);
	const made = (
		(await (
			await request.get(`/api/v1/orders?query=${encodeURIComponent(marker)}`)
		).json()) as OrderPage
	).items[0]!.id;
	await deleteOrder(request, made);

	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page).getByRole('alert')).toContainText(
		'этот приём уже был удалён — начните новый'
	);
	await expect(malfunctionOf(page)).toHaveValue(marker);
	await page.keyboard.press(`${await mod(page)}+Enter`);
	await expect(sheetOf(page)).toBeHidden();
	const id = await acceptedNumber(page);
	late.letGo();
	expect(id).not.toBe(made);
	expect((await fetchOrder(request, id)).malfunction).toBe(marker);
});

test('a photo chosen in intake goes to the accepted order', async ({ page, request }) => {
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillDevice(page);
	await malfunctionOf(page).fill('Разбит экран');
	await photoZoneOf(page)
		.locator('input[type=file]')
		.setInputFiles([png('a.png', 1)]);
	await expect(photoTilesOf(page)).toHaveCount(1);
	await expect(sheetOf(page).getByText('1 из 10')).toBeVisible();
	await sheetOf(page)
		.getByRole('button', { name: /^Принять/ })
		.click();
	const id = await acceptedNumber(page);
	const photos = (await (await request.get(`/api/v1/orders/${id}/attachments`)).json()) as {
		kind: string;
	}[];
	expect(photos.map((p) => p.kind)).toEqual(['photo']);
	// The next intake is a new draft: no photos.
	await openIntake(page);
	await expect(sheetOf(page).getByText('0 из 10')).toBeVisible();
	await expect(photoTilesOf(page)).toHaveCount(0);
});

test('the form cannot be changed while «Принять» waits for its photos: the order has what was sent', async ({
	page,
	request
}) => {
	const marker = uniq('Разбит экран');
	await openIntake(page);
	await phoneOf(page).fill(uniqPhone());
	await fillByHand(page, marker);
	let release: () => void = () => {};
	const gate = new Promise<void>((resolve) => (release = resolve));
	await page.route('**/api/v1/intake-drafts/*/attachments?*', async (route) => {
		if (route.request().method() !== 'POST') return route.fallback();
		await gate;
		await route.fallback();
	});
	await photoZoneOf(page)
		.locator('input[type=file]')
		.setInputFiles([png('slow.png', 5)]);
	await sheetOf(page)
		.getByRole('button', { name: /^Принять/ })
		.click();
	await expect(sheetOf(page).getByText('Загружаю фото…')).toBeVisible();
	// What is typed now would not be in the order: the field does not take it.
	await malfunctionOf(page).click({ force: true });
	await page.keyboard.type('xyz');
	await expect(malfunctionOf(page)).toHaveValue(marker);
	release();
	const id = await acceptedNumber(page);
	expect((await fetchOrder(request, id)).malfunction).toBe(marker);
});

test('cancel removes draft photos', async ({ page, request }) => {
	await openIntake(page);
	await photoZoneOf(page)
		.locator('input[type=file]')
		.setInputFiles([png('b.png', 2)]);
	await expect(photoTilesOf(page)).toHaveCount(1);
	const key = await page.evaluate(
		() =>
			(JSON.parse(localStorage.getItem('mw.intake.draft') ?? '{}') as { key?: string }).key ?? ''
	);
	expect(key).not.toBe('');
	const onServer = async () =>
		((await (await request.get(`/api/v1/intake-drafts/${key}/attachments`)).json()) as unknown[])
			.length;
	expect(await onServer()).toBe(1);
	// Esc and N: the photos wait with the draft.
	await page.keyboard.press('Escape');
	await expect(sheetOf(page)).toBeHidden();
	await openIntake(page);
	await expect(photoTilesOf(page)).toHaveCount(1);

	await sheetOf(page).getByRole('button', { name: 'Отмена' }).click();
	await expect(sheetOf(page)).toBeHidden();
	await expect.poll(onServer).toBe(0);
	await openIntake(page);
	await expect(photoTilesOf(page)).toHaveCount(0);
});

test('a picture pasted into the intake over an order card goes to the intake, not to the card', async ({
	page,
	request
}) => {
	const order = await createOrder(request, { model: uniq('M') });
	await page.goto(`/orders/${order.id}`);
	await expect(page.getByRole('heading', { name: `№ ${order.id}`, level: 1 })).toBeVisible();
	await expect(page.getByRole('group', { name: 'Область для фото' })).toBeVisible();
	await openIntake(page);
	await pastePicture(page, 11);
	await expect(photoTilesOf(page)).toHaveCount(1);
	const onCard = (await (
		await request.get(`/api/v1/orders/${order.id}/attachments`)
	).json()) as unknown[];
	expect(onCard).toHaveLength(0);
});

test('«Снять телефоном» explains how to turn on the host mode', async ({ page }) => {
	await openIntake(page);
	await sheetOf(page).getByRole('button', { name: 'Снять телефоном' }).click();
	const hint = page.getByRole('dialog', { name: 'Снять телефоном' });
	await expect(hint).toContainText(
		'Телефон подключается к компьютеру по сети. Включите режим хоста в настройках «Сеть» — он появится в следующих версиях'
	);
	await hint.getByRole('button', { name: 'Понятно' }).click();
	await expect(hint).toBeHidden();
	await expect(sheetOf(page)).toBeVisible();
});

test('the found clients go as soon as the phone is changed', async ({ page, request }) => {
	const phone = uniqPhone();
	await createOrder(request, { phone, model: uniq('M') });
	await openIntake(page);
	await phoneOf(page).fill(phone);
	const found = sheetOf(page).getByRole('list', { name: 'Найденные клиенты' });
	await expect(found.getByRole('listitem')).toHaveCount(1);
	// The lookup of the new number does not answer: the cards of the old one must not stay meanwhile.
	await page.route(
		(url) => url.pathname === '/api/v1/clients/by-phone',
		() => {}
	);
	await phoneOf(page).fill(uniqPhone());
	await expect(found).toHaveCount(0);
	await expect(nameOf(page)).toBeVisible();
	await page.unrouteAll({ behavior: 'ignoreErrors' });
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
