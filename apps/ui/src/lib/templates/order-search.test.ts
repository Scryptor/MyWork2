import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createOrderSearch, type OrderChoice } from './order-search.svelte.js';

// «Заказ для примера»: «Пример» or an order found by what is typed. The search waits for a pause in the
// typing, an answer of a superseded query is dropped, and leaving the field with nothing picked puts the
// label of the order shown back.

const SAMPLE = 'Пример';
const choice = (id: number): OrderChoice => ({ id, label: `№ ${id} · Тест`, hint: `Клиент ${id}` });

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

function setup(find: (q: string) => Promise<OrderChoice[]>) {
	return createOrderSearch({ find, sample: SAMPLE, sampleHint: 'образец', delayMs: 200 });
}

describe('createOrderSearch', () => {
	it('starts with the sample and no order', () => {
		const s = setup(async () => []);
		expect(s.orderId).toBeNull();
		expect(s.text).toBe(SAMPLE);
		expect(s.items).toEqual([{ value: SAMPLE, hint: 'образец' }]);
	});

	it('does not search for the sample label or an empty text', async () => {
		const find = vi.fn(async () => [choice(1)]);
		const s = setup(find);
		s.setText(SAMPLE);
		s.setText('  ');
		await vi.advanceTimersByTimeAsync(500);
		expect(find).not.toHaveBeenCalled();
	});

	it('searches after a pause, once, for the last text', async () => {
		const find = vi.fn(async (q: string) => [choice(q.length)]);
		const s = setup(find);
		s.setText('a');
		await vi.advanceTimersByTimeAsync(100);
		s.setText('abc');
		await vi.advanceTimersByTimeAsync(250);
		expect(find).toHaveBeenCalledTimes(1);
		expect(find).toHaveBeenCalledWith('abc');
		expect(s.items.map((i) => i.value)).toEqual([SAMPLE, '№ 3 · Тест']);
		expect(s.items[1]!.hint).toBe('Клиент 3');
	});

	it('drops the answer of a superseded query', async () => {
		let release: (c: OrderChoice[]) => void = () => {};
		const find = vi
			.fn<(q: string) => Promise<OrderChoice[]>>()
			.mockImplementationOnce(() => new Promise((r) => (release = r)))
			.mockImplementationOnce(async () => [choice(2)]);
		const s = setup(find);
		s.setText('one');
		await vi.advanceTimersByTimeAsync(250);
		s.setText('two');
		await vi.advanceTimersByTimeAsync(250);
		release([choice(1)]);
		await vi.advanceTimersByTimeAsync(0);
		expect(s.items.map((i) => i.value)).toEqual([SAMPLE, '№ 2 · Тест']);
	});

	it('picks an order found, then the sample again', async () => {
		const s = setup(async () => [choice(7)]);
		s.setText('x');
		await vi.advanceTimersByTimeAsync(250);
		s.pick('№ 7 · Тест');
		expect(s.orderId).toBe(7);
		s.pick(SAMPLE);
		expect(s.orderId).toBeNull();
		// The sample is chosen: what was found before is not offered any more.
		expect(s.items).toEqual([{ value: SAMPLE, hint: 'образец' }]);
	});

	it('clears the offers when the text is emptied', async () => {
		const s = setup(async () => [choice(7)]);
		s.setText('x');
		await vi.advanceTimersByTimeAsync(250);
		expect(s.items).toHaveLength(2);
		s.setText('');
		expect(s.items).toHaveLength(1);
	});

	it('leaving the field with nothing picked shows the order chosen', async () => {
		const s = setup(async () => [choice(7)]);
		s.setText('x');
		await vi.advanceTimersByTimeAsync(250);
		s.pick('№ 7 · Тест');
		s.setText('garbage');
		s.leave();
		expect(s.text).toBe('№ 7 · Тест');
		s.setText('other');
		s.pick(SAMPLE);
		s.setText('zzz');
		s.leave();
		expect(s.text).toBe(SAMPLE);
	});

	it('a failed search offers nothing new', async () => {
		const s = setup(async () => {
			throw new Error('down');
		});
		s.setText('x');
		await vi.advanceTimersByTimeAsync(250);
		expect(s.items).toEqual([{ value: SAMPLE, hint: 'образец' }]);
	});

	it('dispose calls a waiting search off', async () => {
		const find = vi.fn(async () => [choice(1)]);
		const s = setup(find);
		s.setText('x');
		s.dispose();
		await vi.advanceTimersByTimeAsync(500);
		expect(find).not.toHaveBeenCalled();
	});
});
