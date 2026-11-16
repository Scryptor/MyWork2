import { describe, expect, it } from 'vitest';
import { makeOrders } from './fake.js';
import {
	emptyDraft,
	findClient,
	findSerial,
	missingRequired,
	parseDraft,
	phoneKey,
	REQUIRED_LABELS
} from './intake.js';

const orders = makeOrders(2000);

describe('parseDraft', () => {
	it('nothing stored or garbage gives an empty draft', () => {
		for (const raw of [null, '', '{bad', '[]', 'null', '42', '"x"']) {
			expect(parseDraft(raw), String(raw)).toEqual(emptyDraft());
		}
	});

	it('keeps valid fields and drops invalid ones', () => {
		const draft = parseDraft(
			JSON.stringify({
				phone: '70001234567',
				name: 42,
				deviceType: 'Телефон',
				completeness: ['Чехол', 7, 'Зарядка'],
				condition: 'Царапины',
				estimateFrom: 600000,
				estimateTo: -5,
				prepayment: 1.5,
				master: 'АНТОН',
				statusId: '1',
				extra: 'ignored'
			})
		);
		expect(draft).toEqual({
			...emptyDraft(),
			phone: '70001234567',
			deviceType: 'Телефон',
			completeness: ['Чехол', 'Зарядка'],
			estimateFrom: 600000,
			master: 'АНТОН'
		});
	});

	it('a round trip keeps everything', () => {
		const draft = {
			...emptyDraft(),
			phone: '70001234567',
			name: 'ПЕТРОВ ПЁТР',
			malfunctionChips: ['Не включается'],
			malfunction: 'После падения',
			statusId: 4,
			prepayment: 100000,
			warranty: '90 дней'
		};
		expect(parseDraft(JSON.stringify(draft))).toEqual(draft);
	});
});

describe('missingRequired', () => {
	it('an empty draft misses the contact, the device type and the malfunction', () => {
		expect(missingRequired(emptyDraft()).map((k) => REQUIRED_LABELS[k])).toEqual([
			'Телефон или ФИО',
			'Тип устройства',
			'Неисправность'
		]);
	});

	it('a phone or a name is a contact; a chip is a malfunction', () => {
		const base = { ...emptyDraft(), deviceType: 'Телефон', malfunction: 'Не включается' };
		expect(missingRequired({ ...base, phone: '70001234567' })).toEqual([]);
		expect(missingRequired({ ...base, name: 'ПЕТРОВ' })).toEqual([]);
		expect(missingRequired({ ...base, phone: '123' })).toEqual(['contact']);
		expect(missingRequired({ ...base, name: '   ' })).toEqual(['contact']);
		expect(
			missingRequired({ ...base, name: 'ПЕТРОВ', malfunction: ' ', malfunctionChips: ['Залит'] })
		).toEqual([]);
		expect(missingRequired({ ...base, name: 'ПЕТРОВ', deviceType: ' ' })).toEqual(['deviceType']);
	});
});

describe('client lookup by phone', () => {
	it('compares the last ten digits', () => {
		expect(phoneKey('80000000001')).toBe('0000000001');
		expect(phoneKey('+7 (000) 000-00-01'.replace(/\D/g, ''))).toBe('0000000001');
		expect(phoneKey('000000001')).toBeNull();
	});

	it('8 000 000-00-01 finds the fixed fictional client with all their orders', () => {
		const client = findClient('80000000001', orders);
		expect(client).not.toBeNull();
		expect(client?.name).toBe('ЕЛКИН ПЁТР СЕРГЕЕВИЧ');
		expect(client?.phone).toBe('70000000001');
		expect(client?.orders.length).toBe(orders.filter((o) => o.client_id === client?.id).length);
		const at = client?.orders.map((o) => o.received_at) ?? [];
		expect(at).toEqual([...at].sort().reverse());
	});

	it('an unknown or short phone finds nobody', () => {
		expect(findClient('79990000000', orders)).toBeNull();
		expect(findClient('0001', orders)).toBeNull();
	});
});

describe('findSerial', () => {
	it('finds the newest order with this IMEI or serial, ignoring case and spaces', () => {
		const known = [...orders].reverse().find((o) => o.serial !== '');
		expect(known).toBeDefined();
		const found = findSerial(` ${known!.serial.toLowerCase()} `, orders);
		expect(found?.serial).toBe(known!.serial);
	});

	it('short or unknown serials find nothing', () => {
		expect(findSerial('35', orders)).toBeNull();
		expect(findSerial('NO-SUCH-SERIAL', orders)).toBeNull();
	});
});
