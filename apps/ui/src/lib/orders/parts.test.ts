import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { OrderParts } from '$api/OrderParts';
import { ApiError } from '#lib/api/client.js';

const api = vi.hoisted(() => ({
	listOrderParts: vi.fn(),
	addOrderPart: vi.fn(),
	returnOrderPart: vi.fn()
}));
vi.mock('#lib/api/stock.js', () => api);
const count = vi.hoisted(() => ({ refresh: vi.fn() }));
vi.mock('#lib/stock/summary.svelte.js', () => ({ stockCount: count }));

import { createOrderParts, orderCost } from './parts.svelte.js';

const part = (id: number, quantity = 2, price = 100_000) => ({
	id,
	stock_item_id: 5,
	item_exists: true,
	name: 'Дисплей',
	price,
	quantity,
	sum: price * quantity,
	added_by_name: 'Админ',
	added_at: '2026-11-24T10:00:00',
	legacy: false
});
const parts = (...ids: number[]): OrderParts => ({
	parts: ids.map((id) => part(id)),
	total: ids.length * 200_000
});
const flush = () => new Promise((r) => setTimeout(r, 0));
const network = () => new ApiError(0, 'network', 'offline', null);

beforeEach(() => {
	vi.clearAllMocks();
});

describe('orderCost', () => {
	it('is the extra costs and the parts together, in kopecks', () => {
		expect(orderCost(50_000, 370_050)).toBe(420_050);
		expect(orderCost(0, 0)).toBe(0);
	});
});

describe('createOrderParts', () => {
	it('reads the parts of the order', async () => {
		api.listOrderParts.mockResolvedValue(parts(1, 2));
		const p = createOrderParts(7);
		expect(p.loading).toBe(false);
		const done = p.load();
		expect(p.loading).toBe(true);
		await done;
		expect(api.listOrderParts).toHaveBeenCalledWith(7);
		expect(p.parts.map((x) => x.id)).toEqual([1, 2]);
		expect(p.total).toBe(400_000);
		expect(p.loading).toBe(false);
		expect(p.error).toBeNull();
	});

	it('keeps the error of a failed read', async () => {
		api.listOrderParts.mockRejectedValue(network());
		const p = createOrderParts(7);
		await p.load();
		expect(p.error).toBe('offline');
	});

	it('adding twice before the answer sends one request', async () => {
		let answer!: (v: OrderParts) => void;
		api.addOrderPart.mockReturnValue(new Promise<OrderParts>((r) => (answer = r)));
		const p = createOrderParts(7);
		const first = p.add(5, 2);
		const second = p.add(5, 2);
		expect(p.adding).toBe(true);
		answer(parts(1));
		await Promise.all([first, second]);
		expect(api.addOrderPart).toHaveBeenCalledTimes(1);
		expect(p.parts).toHaveLength(1);
		expect(p.adding).toBe(false);
	});

	it('a repeat after a network error keeps the key; after success a new one is made', async () => {
		api.addOrderPart
			.mockRejectedValueOnce(network())
			.mockResolvedValueOnce(parts(1))
			.mockResolvedValueOnce(parts(1, 2));
		const p = createOrderParts(7);
		await expect(p.add(5, 2)).rejects.toMatchObject({ code: 'network' });
		await p.add(5, 2);
		await p.add(5, 2);
		const keys = api.addOrderPart.mock.calls.map((c) => c[1].request_key);
		expect(keys[0]).toBe(keys[1]);
		expect(keys[2]).not.toBe(keys[0]);
		expect(api.addOrderPart.mock.calls[0]).toEqual([
			7,
			{ stock_item_id: 5, quantity: 2, request_key: keys[0] }
		]);
		expect(keys[0]).toMatch(/^[0-9a-f-]{36}$/);
	});

	it('a refusal of the server drops the key', async () => {
		api.addOrderPart
			.mockRejectedValueOnce(new ApiError(422, 'validation', 'x', null))
			.mockResolvedValueOnce(parts(1));
		const p = createOrderParts(7);
		await expect(p.add(5, 2)).rejects.toBeInstanceOf(ApiError);
		await p.add(5, 2);
		const keys = api.addOrderPart.mock.calls.map((c) => c[1].request_key);
		expect(keys[0]).not.toBe(keys[1]);
	});

	it('the counter «Докупить» is asked again after adding and giving back', async () => {
		api.addOrderPart.mockResolvedValue(parts(1));
		api.returnOrderPart.mockResolvedValue(parts());
		const p = createOrderParts(7);
		await p.add(5, 2);
		expect(count.refresh).toHaveBeenCalledTimes(1);
		await p.giveBack(1);
		expect(count.refresh).toHaveBeenCalledTimes(2);
		expect(p.parts).toEqual([]);
		expect(p.total).toBe(0);
	});

	it('a refused write-off does not touch the counter', async () => {
		api.addOrderPart.mockRejectedValue(new ApiError(422, 'validation', 'x', null));
		const p = createOrderParts(7);
		await expect(p.add(5, 2)).rejects.toBeInstanceOf(ApiError);
		expect(count.refresh).not.toHaveBeenCalled();
	});

	it('giving back a part that is already gone (404) reads the list again, without an error', async () => {
		api.returnOrderPart.mockRejectedValue(new ApiError(404, 'not_found', 'нет', null));
		api.listOrderParts.mockResolvedValue(parts(2));
		const p = createOrderParts(7);
		await p.giveBack(1);
		expect(api.listOrderParts).toHaveBeenCalledWith(7);
		expect(p.parts.map((x) => x.id)).toEqual([2]);
		expect(p.error).toBeNull();
		expect(count.refresh).toHaveBeenCalledTimes(1);
	});

	it('giving back fails with other errors', async () => {
		api.returnOrderPart.mockRejectedValue(network());
		const p = createOrderParts(7);
		await expect(p.giveBack(1)).rejects.toMatchObject({ code: 'network' });
		expect(api.listOrderParts).not.toHaveBeenCalled();
	});

	it('giving back twice before the answer sends one request', async () => {
		let answer!: (v: OrderParts) => void;
		api.returnOrderPart.mockReturnValue(new Promise<OrderParts>((r) => (answer = r)));
		const p = createOrderParts(7);
		const a = p.giveBack(1);
		const b = p.giveBack(1);
		answer(parts());
		await Promise.all([a, b]);
		expect(api.returnOrderPart).toHaveBeenCalledTimes(1);
	});

	it('answers that come after detach are ignored', async () => {
		let answer!: (v: OrderParts) => void;
		api.listOrderParts.mockReturnValue(new Promise<OrderParts>((r) => (answer = r)));
		const p = createOrderParts(7);
		const done = p.load();
		p.detach();
		answer(parts(1));
		await done;
		await flush();
		expect(p.parts).toEqual([]);
	});

	it('the revision grows after a write-off, a return and a return that was done already', async () => {
		api.addOrderPart.mockResolvedValue(parts(1));
		api.returnOrderPart
			.mockResolvedValueOnce(parts())
			.mockRejectedValueOnce(new ApiError(404, 'not_found', 'нет', null));
		api.listOrderParts.mockResolvedValue(parts());
		const p = createOrderParts(7);
		expect(p.revision).toBe(0);
		await p.add(5, 2);
		expect(p.revision).toBe(1);
		await p.giveBack(1);
		expect(p.revision).toBe(2);
		await p.giveBack(1);
		expect(p.revision).toBe(3);
	});

	it('a refused write-off leaves the revision', async () => {
		api.addOrderPart.mockRejectedValue(new ApiError(422, 'validation', 'x', null));
		const p = createOrderParts(7);
		await expect(p.add(5, 2)).rejects.toBeInstanceOf(ApiError);
		expect(p.revision).toBe(0);
	});
});
