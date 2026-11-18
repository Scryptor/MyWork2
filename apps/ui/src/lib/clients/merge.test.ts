import { describe, expect, it } from 'vitest';
import { client } from '#lib/app/fixtures.test-helper.js';
import {
	defaultChoice,
	keepChoice,
	mergedPhones,
	toMergeRequest,
	type MergeClient
} from './merge.js';

const mc = (id: number, orders: number, over: Partial<MergeClient> = {}): MergeClient => ({
	...client(id),
	orders_count: orders,
	...over
});

describe('defaultChoice', () => {
	it('keeps the client with more orders, the smaller id on a tie', () => {
		expect(defaultChoice([mc(5, 1), mc(7, 3), mc(2, 3)]).targetId).toBe(2);
		expect(defaultChoice([mc(9, 0), mc(4, 0)]).targetId).toBe(4);
		expect(defaultChoice([mc(1, 0), mc(8, 2)]).targetId).toBe(8);
	});

	it('takes each field from the target, or from the first client that has it', () => {
		const a = mc(1, 5, { full_name: 'А', address: '', note: 'н1', source: '' });
		const b = mc(2, 1, { full_name: 'Б', address: 'ул. Б', note: 'н2', source: 'Сайт' });
		const c = mc(3, 0, { address: 'ул. В', source: 'Друг' });
		const choice = defaultChoice([a, b, c]);
		expect(choice.from).toEqual({
			full_name: 1,
			address: 2,
			note: 1,
			source: 2,
			blacklisted: 1
		});
	});

	it('blacklists when anyone is blacklisted', () => {
		const choice = defaultChoice([mc(1, 5), mc(2, 0, { blacklisted: true })]);
		const req = toMergeRequest(choice, [mc(1, 5), mc(2, 0, { blacklisted: true })]);
		expect(req.blacklisted).toBe(true);
		const none = defaultChoice([mc(1, 5), mc(2, 0)]);
		expect(toMergeRequest(none, [mc(1, 5), mc(2, 0)]).blacklisted).toBe(false);
	});
});

describe('toMergeRequest', () => {
	it('builds the request with versions, sources in the order given', () => {
		const list = [
			mc(1, 0, { version: 4, address: 'ул. 1' }),
			mc(2, 9, { version: 7, full_name: 'ВТОРОЙ' }),
			mc(3, 0, { version: 2 })
		];
		const choice = { ...defaultChoice(list), from: { ...defaultChoice(list).from, address: 1 } };
		expect(toMergeRequest(choice, list)).toEqual({
			target_id: 2,
			target_version: 7,
			sources: [
				{ id: 1, version: 4 },
				{ id: 3, version: 2 }
			],
			full_name: 'ВТОРОЙ',
			address: 'ул. 1',
			note: '',
			source: '',
			blacklisted: false
		});
	});
});

describe('mergedPhones', () => {
	it('puts the target first, then the sources in order, dropping repeats by digits', () => {
		const a = mc(1, 0, {
			phones: [
				{ phone: '79001112233', note: 'раб' },
				{ phone: '79005550000', note: '' }
			]
		});
		const b = mc(2, 0, {
			phones: [
				{ phone: '89001112233', note: 'дубль' },
				{ phone: '79006660000', note: 'жена' }
			]
		});
		const c = mc(3, 0, { phones: [{ phone: '79007770000', note: '' }] });
		expect(mergedPhones([a, b, c], 2)).toEqual([
			{ phone: '89001112233', note: 'дубль' },
			{ phone: '79006660000', note: 'жена' },
			{ phone: '79005550000', note: '' },
			{ phone: '79007770000', note: '' }
		]);
	});
});

describe('keepChoice', () => {
	it('keeps what was chosen while that client is still there', () => {
		const list = [mc(1, 0, { address: 'а' }), mc(2, 0, { address: 'б' })];
		const chosen = { ...defaultChoice(list), targetId: 2 };
		chosen.from = { ...chosen.from, address: 2 };
		expect(keepChoice(chosen, list)).toEqual(chosen);
		const without = [mc(1, 0), mc(3, 0)];
		expect(keepChoice(chosen, without)).toEqual(defaultChoice(without));
	});
});
