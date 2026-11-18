import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { Status } from '$api/Status';
import { item, STATUSES } from '#lib/app/fixtures.test-helper.js';
import {
	acceptedAnswer,
	attemptOf,
	emptyDraft,
	intakeStatuses,
	joinChips,
	lateAnswer,
	mergeSuggestions,
	missingRequired,
	newDraftKey,
	parseDraft,
	phoneQuery,
	photoLock,
	REQUIRED_LABELS,
	serialHits,
	startsAnotherOrder,
	serialQuery,
	splitChips,
	toNewOrder,
	underWarranty,
	warrantyUntil,
	withPrefill,
	type IntakeDraft
} from './draft.js';

const SETTINGS: AppSettings = {
	currency_symbol: '₽',
	master_percent: 40,
	diagnostics_overdue_days: 3,
	ready_stale_days: 14,
	default_warranty: '30 дней',
	intake_required_fields: false,
	default_service_point_id: null,
	default_master: 'ТЕСТОВ',
	firm_name: '',
	firm_phone: '',
	firm_details: '',
	firm_legal_details: '',
	intake_terms: '',
	issue_terms: '',
	timezone: 'UTC'
};
const STATUS_LIST: Status[] = [
	...STATUSES,
	{
		id: 5,
		name: 'Старая диагностика',
		kind: 'diagnostics',
		color: 'red',
		sort_order: 0,
		archived: true
	},
	{
		id: 6,
		name: 'Ещё диагностика',
		kind: 'diagnostics',
		color: 'red',
		sort_order: 9,
		archived: false
	}
];
const empty = () => emptyDraft(SETTINGS, STATUS_LIST);
/** The required fields filled for a new client. */
const filled = (over: Partial<IntakeDraft> = {}): IntakeDraft => ({
	...empty(),
	phones: [{ phone: '89211234567', note: '' }],
	deviceType: 'Телефон',
	malfunction: 'не заряжается',
	...over
});
/** A draft key: a UUID, as `crypto.randomUUID` makes it. */
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
/** Every new draft has a key of its own: compare the rest. */
const sansKey = (d: IntakeDraft) => ({ ...d, key: '' });

afterEach(() => vi.unstubAllGlobals());

describe('emptyDraft', () => {
	it('empty draft takes master, warranty and the first diagnostics status', () => {
		const d = empty();
		expect(d.master).toBe('ТЕСТОВ');
		expect(d.warranty).toBe('30 дней');
		// «Диагностика» (2) comes before «Ещё диагностика» (6); the archived one (5) is skipped.
		expect(d.statusId).toBe(2);
		expect(d.clientId).toBeNull();
		expect(d.phones).toEqual([{ phone: '', note: '' }]);
		expect(d.completeness).toEqual([]);
		expect(d.prepayment).toBeNull();
		expect(d.acceptedAs).toBeNull();
	});

	it('every draft gets a key of its own', () => {
		const [a, b] = [empty(), empty()];
		expect(a.key).toMatch(UUID);
		expect(b.key).toMatch(UUID);
		expect(a.key).not.toBe(b.key);
	});

	it('the key is made without randomUUID too (a page not on a secure address)', () => {
		const getRandomValues = crypto.getRandomValues.bind(crypto);
		vi.stubGlobal('crypto', { getRandomValues });
		const [a, b] = [newDraftKey(), newDraftKey()];
		expect(a).toMatch(UUID);
		expect(a).not.toBe(b);
	});

	it('without a diagnostics status the server picks the status', () => {
		expect(
			emptyDraft(
				SETTINGS,
				STATUSES.filter((s) => s.kind !== 'diagnostics')
			).statusId
		).toBe(null);
	});

	it('a new order starts in work: open and diagnostics statuses, not archived, in their order', () => {
		expect(intakeStatuses(STATUS_LIST).map((s) => s.id)).toEqual([1, 2, 6]);
	});
});

describe('toNewOrder', () => {
	it('builds a new client order with kopecks and trimmed text', () => {
		const d = {
			...empty(),
			fullName: ' Пётр ',
			phones: [{ phone: '89211234567', note: '' }],
			deviceType: 'Телефон',
			malfunction: 'не заряжается',
			prepayment: 200_000
		};
		expect(toNewOrder(d, 1)).toMatchObject({
			client: {
				new: { full_name: 'Пётр', phones: [{ phone: '89211234567', note: '' }] }
			},
			service_point_id: 1,
			prepayment: 200_000
		});
	});

	it('uses the existing client id', () => {
		const order = toNewOrder(filled({ clientId: 5, fullName: 'Другое имя' }), null);
		expect(order.client).toEqual({ existing: 5 });
		expect(order.service_point_id).toBeNull();
	});

	it('takes every field of the form', () => {
		const draft = filled({
			fullName: 'ПЕТРОВ ПЁТР',
			phones: [
				{ phone: '89211234567', note: ' ' },
				{ phone: '', note: '' },
				{ phone: '+7 (921) 000-00-01', note: ' жена ' }
			],
			source: ' Авито ',
			address: ' ул. Тестовая, 1 ',
			brand: ' Тест ',
			model: 'Модель 5 ',
			color: 'Чёрный',
			serial: ' 356000000000001 ',
			password: ' 12 34 ',
			completeness: ['Чехол', 'Зарядка'],
			condition: ['Царапины'],
			malfunctionChips: ['Не включается', 'Залит'],
			malfunction: ' после падения ',
			master: ' АНТОН ',
			statusId: 6,
			estimateFrom: 500_000,
			estimateTo: 700_000,
			prepayment: null,
			warranty: ' 90 дней ',
			comment: ' позвонить вечером '
		});
		const order = toNewOrder(draft, 2);
		expect(order).toEqual({
			client: {
				new: {
					full_name: 'ПЕТРОВ ПЁТР',
					phones: [
						{ phone: '89211234567', note: '' },
						{ phone: '79210000001', note: 'жена' }
					],
					address: 'ул. Тестовая, 1',
					note: '',
					source: 'Авито',
					blacklisted: false
				}
			},
			service_point_id: 2,
			status_id: 6,
			device_type: 'Телефон',
			brand: 'Тест',
			model: 'Модель 5',
			serial: '356000000000001',
			color: 'Чёрный',
			// A password may have spaces of its own.
			device_password: ' 12 34 ',
			completeness: 'Чехол, Зарядка',
			device_condition: 'Царапины',
			malfunction: 'Не включается, Залит, после падения',
			internal_comment: 'позвонить вечером',
			master: 'АНТОН',
			estimate_from: 500_000,
			estimate_to: 700_000,
			prepayment: 0,
			warranty: '90 дней',
			need_call: false,
			need_parts: false,
			attachment_draft: null,
			request_key: draft.key
		});
	});

	it('the key of the draft is the key of the attempt, and of its photos when there are any', () => {
		const d = filled();
		expect(toNewOrder(d, null, true)).toMatchObject({
			attachment_draft: d.key,
			request_key: d.key
		});
		// Without photos the server has no draft to look at.
		expect(toNewOrder(d, null, false)).toMatchObject({
			attachment_draft: null,
			request_key: d.key
		});
	});
});

describe('missingRequired', () => {
	it('requires a phone or a name, a device type and a malfunction', () => {
		expect(missingRequired(empty()).map((k) => REQUIRED_LABELS[k])).toEqual([
			'Телефон или ФИО',
			'Тип устройства',
			'Неисправность'
		]);
		const base = filled({ phones: [{ phone: '', note: '' }] });
		expect(missingRequired(base)).toEqual(['contact']);
		expect(missingRequired({ ...base, fullName: '  ' })).toEqual(['contact']);
		expect(missingRequired({ ...base, phones: [{ phone: '1234', note: '' }] })).toEqual([
			'contact'
		]);
		// A small town number «2-12-34» is a phone; any of the phones counts.
		expect(
			missingRequired({
				...base,
				phones: [
					{ phone: '', note: '' },
					{ phone: '21234', note: '' }
				]
			})
		).toEqual([]);
		expect(missingRequired({ ...base, fullName: 'ПЕТРОВ' })).toEqual([]);
		// The client chosen among the found ones is the contact.
		expect(missingRequired({ ...base, clientId: 5 })).toEqual([]);
		expect(missingRequired(filled({ deviceType: ' ' }))).toEqual(['deviceType']);
		expect(missingRequired(filled({ malfunction: ' ' }))).toEqual(['malfunction']);
		expect(missingRequired(filled({ malfunction: '', malfunctionChips: ['Залит'] }))).toEqual([]);
	});

	it('with «intake_required_fields» the brand and the model too, as the server checks', () => {
		expect(missingRequired(filled(), true).map((k) => REQUIRED_LABELS[k])).toEqual([
			'Бренд',
			'Модель'
		]);
		expect(missingRequired(empty(), true)).toEqual([
			'contact',
			'deviceType',
			'brand',
			'model',
			'malfunction'
		]);
		expect(missingRequired(filled({ brand: 'Тест', model: ' М1 ' }), true)).toEqual([]);
	});
});

describe('parseDraft', () => {
	it('garbage in storage gives an empty draft', () => {
		for (const raw of [null, '', '{', '[]', 'null', '42', '"x"']) {
			const d = parseDraft(raw, SETTINGS, STATUS_LIST);
			expect(sansKey(d), String(raw)).toEqual(sansKey(empty()));
			expect(d.key).toMatch(UUID);
		}
	});

	it('a round trip keeps everything', () => {
		const d = filled({
			clientId: 7,
			newClient: true,
			fullName: 'ПЕТРОВ ПЁТР',
			phones: [
				{ phone: '89211234567', note: '' },
				{ phone: '89210000001', note: 'жена' }
			],
			source: 'Авито',
			address: 'ул. Тестовая, 1',
			completeness: ['Чехол'],
			condition: ['Царапины'],
			malfunctionChips: ['Залит'],
			master: '',
			statusId: 1,
			estimateFrom: 1,
			estimateTo: 2,
			prepayment: 100_000,
			warranty: '',
			comment: 'тест'
		});
		d.acceptedAs = { id: 12, key: d.key, clientId: 7 };
		d.attempt = attemptOf(d);
		expect(parseDraft(JSON.stringify(d), SETTINGS, STATUS_LIST)).toEqual(d);
		for (const attempt of [42, null, ['x']]) {
			const raw = JSON.stringify({ attempt });
			expect(parseDraft(raw, SETTINGS, STATUS_LIST).attempt, String(attempt)).toBeNull();
		}
	});

	it('a draft of the old kind (without a key) gets a new one', () => {
		const raw = JSON.stringify({ malfunction: 'Не включается' });
		const [a, b] = [parseDraft(raw, SETTINGS, STATUS_LIST), parseDraft(raw, SETTINGS, STATUS_LIST)];
		expect(a.malfunction).toBe('Не включается');
		expect(a.key).toMatch(UUID);
		expect(a.key).not.toBe(b.key);
		// The server takes 1–64 letters, digits and dashes: anything else is no key either.
		for (const key of ['', 'с пробелом', 'x'.repeat(65), 42]) {
			const d = parseDraft(JSON.stringify({ key }), SETTINGS, STATUS_LIST);
			expect(d.key, String(key)).toMatch(UUID);
		}
		expect(parseDraft('{"key":"Ab-1"}', SETTINGS, STATUS_LIST).key).toBe('Ab-1');
	});

	it('the mark of an order accepted from the draft belongs to its key', () => {
		const key = 'k-1';
		const read = (acceptedAs: unknown) =>
			parseDraft(JSON.stringify({ key, acceptedAs }), SETTINGS, STATUS_LIST).acceptedAs;
		expect(read({ id: 7, key, clientId: 3 })).toEqual({ id: 7, key, clientId: 3 });
		for (const acceptedAs of [
			// A mark of 5.1 (a number) has no key to belong to.
			7,
			'12',
			null,
			{ id: 7, key: 'k-2', clientId: 3 },
			{ id: 0, key, clientId: 3 },
			{ id: 1.5, key, clientId: 3 },
			{ id: 7, clientId: 3 },
			{ id: 7, key, clientId: -1 }
		]) {
			expect(read(acceptedAs), JSON.stringify(acceptedAs)).toBeNull();
		}
	});

	it('keeps valid fields and drops invalid ones', () => {
		const d = parseDraft(
			JSON.stringify({
				clientId: '5',
				phones: [{ phone: '+7 921 123-45-67', note: 'раб' }, { phone: 5 }, 'x'],
				deviceType: 'Телефон',
				completeness: ['Чехол', 7, 'Зарядка'],
				condition: 'Царапины, скол ',
				estimateFrom: 600000,
				estimateTo: -5,
				prepayment: 1.5,
				master: 42,
				statusId: 4,
				extra: 'ignored'
			}),
			SETTINGS,
			STATUS_LIST
		);
		expect(d).toEqual({
			...empty(),
			key: d.key,
			phones: [{ phone: '79211234567', note: 'раб' }],
			deviceType: 'Телефон',
			completeness: ['Чехол', 'Зарядка'],
			// Chips stored as text (R16) come back as chips.
			condition: ['Царапины', 'скол'],
			estimateFrom: 600000
		});
		// «Выдан» (4) cannot start an order: the default status instead.
		expect(d.statusId).toBe(2);
	});

	it('a draft without phones still has the phone field', () => {
		expect(parseDraft(JSON.stringify({ phones: [] }), SETTINGS, STATUS_LIST).phones).toEqual([
			{ phone: '', note: '' }
		]);
	});
});

describe('withPrefill', () => {
	it('intake for a client keeps the draft of that client and replaces any other', () => {
		const mine = filled({ clientId: 5, malfunction: 'своё' });
		expect(withPrefill(mine, { clientId: 5 }, SETTINGS, STATUS_LIST)).toEqual(mine);
		for (const other of [filled({ clientId: 6 }), filled()]) {
			const d = withPrefill(other, { clientId: 5 }, SETTINGS, STATUS_LIST);
			expect(sansKey(d)).toEqual(sansKey({ ...empty(), clientId: 5 }));
			// A new draft: its photos and its attempt are not those of the one replaced.
			expect(d.key).not.toBe(other.key);
		}
	});

	it('a draft already accepted is replaced by intake for any client', () => {
		for (const clientId of [5, 6, null]) {
			const accepted = filled({ clientId });
			accepted.acceptedAs = { id: 9, key: accepted.key, clientId: 5 };
			const d = withPrefill(accepted, { clientId: 5 }, SETTINGS, STATUS_LIST);
			expect(sansKey(d), String(clientId)).toEqual(sansKey({ ...empty(), clientId: 5 }));
			expect(d.key).not.toBe(accepted.key);
		}
	});

	it('without a prefill the draft stays', () => {
		const d = filled({ clientId: 6 });
		expect(withPrefill(d, {}, SETTINGS, STATUS_LIST)).toBe(d);
	});
});

describe('chips as text (R16)', () => {
	it('joins with a comma and splits back trimmed', () => {
		expect(joinChips(['Чехол', ' Зарядка '])).toBe('Чехол, Зарядка');
		expect(splitChips(' Чехол, Зарядка ,, ')).toEqual(['Чехол', 'Зарядка']);
		expect(splitChips('')).toEqual([]);
	});
});

describe('mergeSuggestions', () => {
	it('dictionary first, then the values of past orders not in it, ignoring case', () => {
		expect(mergeSuggestions(['Телефон', 'Ноутбук'], ['телефон', 'Планшет', 'Ноутбук '])).toEqual([
			'Телефон',
			'Ноутбук',
			'Планшет'
		]);
	});
});

describe('lookups', () => {
	it('the phone is looked up from five digits', () => {
		const d = (phone: string) => filled({ phones: [{ phone, note: '' }] });
		expect(phoneQuery(d('1234'))).toBeNull();
		expect(phoneQuery(d('12345'))).toBe('12345');
		expect(phoneQuery(d('+7 921 123-45-67'))).toBe('79211234567');
	});

	it('the serial is looked up from five characters', () => {
		expect(serialQuery(' 3560 ')).toBeNull();
		expect(serialQuery(' 35600 ')).toBe('35600');
	});

	it('«Этот IMEI уже был» keeps the orders whose serial has the text, ignoring case', () => {
		const orders = [
			item(1, { serial: 'AB356000' }),
			item(2, { serial: '', model: 'ab356' }),
			item(3, { serial: 'x-ab356-y' })
		];
		expect(serialHits(orders, 'Ab356').map((o) => o.id)).toEqual([1, 3]);
	});
});

describe('warranty of a past order', () => {
	const now = { year: 2026, month: 11, day: 17, hour: 12, minute: 0, second: 0 };

	it('ends the term after the issue', () => {
		expect(warrantyUntil('2026-11-01T10:00:00', { unit: 'days', value: 30 })).toEqual({
			year: 2026,
			month: 12,
			day: 1,
			hour: 10,
			minute: 0,
			second: 0
		});
		// The last day of a shorter month.
		expect(warrantyUntil('2026-01-31T09:00:00', { unit: 'months', value: 1 })).toMatchObject({
			year: 2026,
			month: 2,
			day: 28
		});
		expect(warrantyUntil('2026-11-30T09:00:00', { unit: 'months', value: 3 })).toMatchObject({
			year: 2027,
			month: 2,
			day: 28
		});
	});

	it('no term, «без гарантии» (0 of any unit) or a broken date: no warranty', () => {
		expect(warrantyUntil('2026-11-01T10:00:00', null)).toBeNull();
		expect(warrantyUntil('2026-11-01T10:00:00', { unit: 'months', value: 0 })).toBeNull();
		expect(warrantyUntil('2026-11-01T10:00:00', { unit: 'days', value: 0 })).toBeNull();
		expect(warrantyUntil('вчера', { unit: 'days', value: 30 })).toBeNull();
	});

	it('is under warranty while the end is after now', () => {
		const until = (issued: string) => underWarranty(issued, { unit: 'days', value: 30 }, now);
		expect(until('2026-11-01T10:00:00')).toMatchObject({ month: 12, day: 1 });
		expect(until('2026-10-18T12:00:00')).toBeNull();
		expect(until('2026-10-18T12:00:01')).not.toBeNull();
		expect(underWarranty('2026-11-01T10:00:00', { unit: 'days', value: 0 }, now)).toBeNull();
	});
});

describe('lateAnswer', () => {
	const sent = filled({ fullName: 'Тестов', brand: 'Тест', model: 'М1', serial: '356789' });

	it('the very draft that was sent goes', () => {
		expect(lateAnswer(structuredClone(sent), sent)).toBe('close');
	});

	it('the draft changed since then is marked as accepted, whatever was changed', () => {
		// Its key is the key of the attempt: the name, the phone or the device edited is still it.
		for (const now of [
			{ ...sent, fullName: 'Тестов Пётр' },
			{ ...sent, phones: [{ phone: '89210000001', note: '' }] },
			{ ...sent, deviceType: 'Ноутбук', model: 'М2', serial: '356780' },
			{ ...sent, comment: 'Позвонить вечером', prepayment: 50_000 }
		]) {
			expect(lateAnswer(now, sent), JSON.stringify(now)).toBe('mark');
		}
	});

	it('a repeat after no answer comes late: changed since the first attempt, it is marked, not closed', () => {
		const first = filled({ comment: '' });
		// «Принять» pressed again after an edit, then Esc: the repeat's answer comes to a closed sheet.
		const repeat = { ...first, comment: 'Позвонить вечером', attempt: attemptOf(first) };
		expect(lateAnswer(structuredClone(repeat), repeat)).toBe('mark');
		// Nothing changed since the first attempt: it goes.
		const same = { ...first, attempt: attemptOf(first) };
		expect(lateAnswer(structuredClone(same), same)).toBe('close');
	});

	it('a draft begun after it (another key) is left alone', () => {
		expect(lateAnswer({ ...sent, key: newDraftKey() }, sent)).toBe('other');
	});
});

describe('an earlier «Принять» of the draft that may have made the order', () => {
	const mark = (d: IntakeDraft, clientId: number) => ({ id: 9, key: d.key, clientId });

	it('locks the photos: they would not go to that order', () => {
		const d = filled();
		expect(empty().attempt).toBeNull();
		expect(photoLock(d)).toBeNull();
		// No answer in time: whether the order is there is not known yet.
		expect(photoLock({ ...d, attempt: attemptOf(d) })).toBe('unanswered');
		expect(photoLock({ ...d, attempt: attemptOf(d), acceptedAs: mark(d, 1) })).toBe('accepted');
	});

	it('the order it made closes the sheet, unless the draft was changed since', () => {
		const d = filled({ fullName: 'Тестов', model: 'М1' });
		// No earlier attempt: whatever came is this draft's order.
		expect(acceptedAnswer(d)).toBe('close');
		const sent = { ...d, attempt: attemptOf(d) };
		expect(acceptedAnswer(sent)).toBe('close');
		// Only what goes to the server counts: spaces around a value are no change.
		expect(acceptedAnswer({ ...sent, model: ' М1 ' })).toBe('close');
		for (const changed of [
			{ ...sent, comment: 'Позвонить вечером' },
			{ ...sent, fullName: 'Тестов Пётр' },
			{ ...sent, model: 'М2' },
			{ ...sent, prepayment: 100_000 }
		]) {
			expect(acceptedAnswer(changed), JSON.stringify(changed)).toBe('keep');
		}
	});

	it('another client than the one it was sent for starts another order', () => {
		const client = (id: number, ...phones: string[]) => ({
			id,
			phones: phones.map((phone) => ({ phone, note: '' }))
		});
		const fresh = filled();
		expect(startsAnotherOrder(fresh, client(5))).toBe(false);
		const existing = filled({ clientId: 5 });
		const timedOut = { ...existing, attempt: attemptOf(existing) };
		expect(startsAnotherOrder(timedOut, client(5))).toBe(false);
		expect(startsAnotherOrder(timedOut, client(6))).toBe(true);
		// Accepted: the client of that order is known.
		const marked = { ...fresh, attempt: attemptOf(fresh), acceptedAs: mark(fresh, 5) };
		expect(startsAnotherOrder(marked, client(5))).toBe(false);
		expect(startsAnotherOrder(marked, client(6))).toBe(true);
	});

	it('the client a new-client attempt may have made is the same client: one with its phone', () => {
		const client = (id: number, ...phones: string[]) => ({
			id,
			phones: phones.map((phone) => ({ phone, note: '' }))
		});
		const fresh = filled({ phones: [{ phone: '89211234567', note: '' }] });
		const timedOut = { ...fresh, attempt: attemptOf(fresh) };
		// Found by that phone after Esc and N: as the server keeps it, among others or not first.
		expect(startsAnotherOrder(timedOut, client(5, '79211234567'))).toBe(false);
		expect(startsAnotherOrder(timedOut, client(5, '74950000000', '9211234567'))).toBe(false);
		expect(startsAnotherOrder(timedOut, client(6, '79210000001'))).toBe(true);
		expect(startsAnotherOrder(timedOut, client(7))).toBe(true);
		// Sent with a name only: a client found by a phone typed later is another one.
		const named = filled({ phones: [{ phone: '', note: '' }], fullName: 'Тестов' });
		expect(startsAnotherOrder({ ...named, attempt: attemptOf(named) }, client(5, '1'))).toBe(true);
	});
});
