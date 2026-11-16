import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import { describe, expect, it, vi } from 'vitest';
import { item, order } from './fixtures.test-helper.js';
import { resolveScan } from './scan.js';

describe('resolveScan', () => {
	it('one order by barcode opens it, two ask, none says not found', async () => {
		const deps = (list: OrderListItem[]) => ({
			byBarcode: async () => list,
			byNumber: async () => null
		});
		expect(await resolveScan({ kind: 'barcode', value: '012345678905' }, deps([item(1)]))).toEqual({
			kind: 'open',
			id: 1
		});
		const two = await resolveScan(
			{ kind: 'barcode', value: '012345678905' },
			deps([item(1), item(2)])
		);
		expect(two).toEqual({ kind: 'choose', orders: [item(1), item(2)] });
		expect(await resolveScan({ kind: 'barcode', value: '012345678905' }, deps([]))).toEqual({
			kind: 'notFound',
			text: 'Заказ со штрихкодом 012345678905 не найден'
		});
	});

	it('a number opens its order or says it is not found', async () => {
		const byNumber = vi.fn(async (n: number): Promise<Order | null> => (n === 7 ? order(7) : null));
		const deps = { byBarcode: async () => [], byNumber };
		expect(await resolveScan({ kind: 'order', id: 7 }, deps)).toEqual({ kind: 'open', id: 7 });
		expect(await resolveScan({ kind: 'order', id: 8 }, deps)).toEqual({
			kind: 'notFound',
			text: 'Заказ № 8 не найден'
		});
	});

	it('other text goes to the search without asking the server', async () => {
		const deps = { byBarcode: vi.fn(async () => []), byNumber: vi.fn(async () => null) };
		expect(await resolveScan({ kind: 'text', value: 'SN-ABC123' }, deps)).toEqual({
			kind: 'search',
			text: 'SN-ABC123'
		});
		expect(deps.byBarcode).not.toHaveBeenCalled();
		expect(deps.byNumber).not.toHaveBeenCalled();
	});
});
