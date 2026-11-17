import { describe, expect, it } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { Status } from '$api/Status';
import { item, STATUSES } from '#lib/app/fixtures.test-helper.js';
import {
	emptyDraft,
	intakeStatuses,
	joinChips,
	mergeSuggestions,
	missingRequired,
	parseDraft,
	phoneQuery,
	REQUIRED_LABELS,
	sameOrder,
	serialHits,
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
		const order = toNewOrder(
			filled({
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
			}),
			2
		);
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
			request_key: null
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
			expect(parseDraft(raw, SETTINGS, STATUS_LIST), String(raw)).toEqual(empty());
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
			comment: 'тест',
			acceptedAs: 12
		});
		expect(parseDraft(JSON.stringify(d), SETTINGS, STATUS_LIST)).toEqual(d);
	});

	it('a mark of an order already accepted from the draft is a number or nothing', () => {
		for (const acceptedAs of ['12', 0, -3, 1.5, null]) {
			const raw = JSON.stringify({ malfunction: 'Не включается', acceptedAs });
			expect(parseDraft(raw, SETTINGS, STATUS_LIST).acceptedAs, String(acceptedAs)).toBeNull();
		}
		expect(parseDraft('{"acceptedAs":7}', SETTINGS, STATUS_LIST).acceptedAs).toBe(7);
		expect(empty().acceptedAs).toBeNull();
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
		expect(withPrefill(filled({ clientId: 6 }), { clientId: 5 }, SETTINGS, STATUS_LIST)).toEqual({
			...empty(),
			clientId: 5
		});
		expect(withPrefill(filled(), { clientId: 5 }, SETTINGS, STATUS_LIST)).toEqual({
			...empty(),
			clientId: 5
		});
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

describe('sameOrder', () => {
	it('the same client and device are the same order whatever else changed', () => {
		const sent = filled({ brand: 'Тест', model: 'М1', serial: '356789' });
		expect(sameOrder(sent, { ...sent, comment: 'Позвонить вечером', prepayment: 50_000 })).toBe(
			true
		);
		expect(sameOrder(sent, { ...sent, model: ' М1 ' })).toBe(true);
		for (const other of [
			{ ...sent, clientId: 4 },
			{ ...sent, fullName: 'Другой Клиент' },
			{ ...sent, phones: [{ phone: '89210000001', note: '' }] },
			{ ...sent, deviceType: 'Ноутбук' },
			{ ...sent, brand: 'Другой' },
			{ ...sent, model: 'М2' },
			{ ...sent, serial: '356780' }
		]) {
			expect(sameOrder(sent, other)).toBe(false);
		}
	});
});
