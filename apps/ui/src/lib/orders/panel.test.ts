import { afterEach, describe, expect, it } from 'vitest';
import { recentIds } from '#lib/app/recent.js';
import { orderPanel } from './panel.svelte.js';

afterEach(() => {
	orderPanel.close();
	localStorage.clear();
});

describe('orderPanel', () => {
	it('opens an order, remembers it in recent and closes', () => {
		orderPanel.open(15, { back: { label: 'Клиент', href: '/clients/3' } });
		expect(orderPanel.id).toBe(15);
		expect(orderPanel.back).toEqual({ label: 'Клиент', href: '/clients/3' });
		expect(recentIds()).toEqual([15]);
		orderPanel.close();
		expect(orderPanel.id).toBeNull();
		expect(orderPanel.back).toBeNull();
	});

	it('next goes along the neighbours as they are now and stops at the ends', () => {
		let list = [10, 11, 12];
		orderPanel.open(11, { neighbors: () => list });
		expect(orderPanel.hasNext(1)).toBe(true);
		orderPanel.next(1);
		expect(orderPanel.id).toBe(12);
		expect(orderPanel.hasNext(1)).toBe(false);
		orderPanel.next(1);
		expect(orderPanel.id).toBe(12);
		list = [12, 30, 10];
		orderPanel.next(-1);
		expect(orderPanel.id).toBe(12);
		orderPanel.next(1);
		expect(orderPanel.id).toBe(30);
		expect(recentIds()).toEqual([30, 12, 11]);
	});

	it('without neighbours, or out of the list, there is no next', () => {
		orderPanel.open(5);
		expect(orderPanel.hasNext(1)).toBe(false);
		orderPanel.next(1);
		expect(orderPanel.id).toBe(5);
		orderPanel.open(6, { neighbors: () => [1, 2] });
		expect(orderPanel.hasNext(-1)).toBe(false);
		orderPanel.next(-1);
		expect(orderPanel.id).toBe(6);
	});

	it('a guard decides whether the panel leaves the order: another one, a neighbour or closing', async () => {
		orderPanel.open(1, { neighbors: () => [1, 2, 3] });
		let allow = false;
		const asked: number[] = [];
		const off = orderPanel.guard(async () => {
			asked.push(orderPanel.id ?? 0);
			return allow;
		});

		await expect(orderPanel.open(2)).resolves.toBe(false);
		expect(orderPanel.id).toBe(1);
		await expect(orderPanel.next(1)).resolves.toBe(false);
		expect(orderPanel.id).toBe(1);
		await expect(orderPanel.close()).resolves.toBe(false);
		expect(orderPanel.id).toBe(1);
		expect(asked).toEqual([1, 1, 1]);

		// The same order again needs no check.
		await expect(orderPanel.open(1, { neighbors: () => [1, 2, 3] })).resolves.toBe(true);
		expect(asked).toHaveLength(3);

		allow = true;
		await expect(orderPanel.next(1)).resolves.toBe(true);
		expect(orderPanel.id).toBe(2);
		await expect(orderPanel.close()).resolves.toBe(true);
		expect(orderPanel.id).toBeNull();
		// Opening from closed needs no check either.
		allow = false;
		await expect(orderPanel.open(3)).resolves.toBe(true);
		off();
		await expect(orderPanel.open(2)).resolves.toBe(true);
		expect(orderPanel.id).toBe(2);
	});

	it('the order of the full card is not opened in the panel a second time', async () => {
		const off = orderPanel.cardShows(7);
		await expect(orderPanel.open(7)).resolves.toBe(true);
		expect(orderPanel.id).toBeNull();
		// Over the card another order makes way for it: the card shows the order asked for.
		await orderPanel.open(8);
		expect(orderPanel.id).toBe(8);
		await expect(orderPanel.open(7)).resolves.toBe(true);
		expect(orderPanel.id).toBeNull();
		expect(recentIds()).toEqual([8]);
		off();
		await orderPanel.open(7);
		expect(orderPanel.id).toBe(7);
		// The card comes for the order the panel shows: the panel goes.
		const again = orderPanel.cardShows(7);
		await Promise.resolve();
		expect(orderPanel.id).toBeNull();
		again();
	});

	it('a guard that fails keeps the order', async () => {
		orderPanel.open(1);
		const off = orderPanel.guard(() => Promise.reject(new Error('нет связи')));
		await expect(orderPanel.close()).resolves.toBe(false);
		expect(orderPanel.id).toBe(1);
		off();
	});
});
