import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DictionaryItem } from '$api/DictionaryItem';
import { ApiError } from '#lib/api/client.js';
import { createCatalog, mergeOrder, type CatalogApi } from './catalog.svelte.js';

const item = (id: number, value: string, archived = false): DictionaryItem => ({
	id,
	kind: 'brand',
	value,
	sort_order: id,
	archived
});

const validation = (field: string, code: string) =>
	new ApiError(422, 'validation', 'Ошибка', {
		code: 'validation',
		message: 'Ошибка',
		fields: [{ field, code, message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

let api: { [K in keyof CatalogApi]: ReturnType<typeof vi.fn<CatalogApi[K]>> };

beforeEach(() => {
	api = {
		list: vi.fn(async () => [
			item(1, 'Alpha'),
			item(2, 'Beta'),
			item(3, 'Gamma'),
			item(4, 'Old', true)
		]),
		usage: vi.fn(async () => [{ id: 1, count: 3 }]),
		add: vi.fn(async (_k, v) => item(9, v)),
		update: vi.fn(async (id, p) => ({
			...item(id, p.value ?? 'x'),
			archived: p.archived ?? false
		})),
		remove: vi.fn(async () => {}),
		reorder: vi.fn(async () => {}),
		rename: vi.fn(async (id, b) => ({ item: item(id, b.value), records_changed: 2 })),
		merge: vi.fn(async (_k, b) => ({ item: item(b.target_id, 'Beta'), records_changed: 5 }))
	};
});

async function loaded() {
	const c = createCatalog(api);
	await c.load('brand');
	return c;
}

describe('mergeOrder', () => {
	it('puts the visible ones into the slots they occupy', () => {
		expect(mergeOrder([1, 2, 3, 4], [4, 2])).toEqual([1, 4, 3, 2]);
		expect(mergeOrder([1, 2, 3], [3, 2, 1])).toEqual([3, 2, 1]);
	});
});

describe('createCatalog', () => {
	it('loads the values with archived ones and the counts', async () => {
		const c = await loaded();
		expect(api.list).toHaveBeenCalledWith('brand', true);
		expect(c.active.map((r) => [r.label, r.count])).toEqual([
			['Alpha', 3],
			['Beta', 0],
			['Gamma', 0]
		]);
		expect(c.archived.map((r) => r.label)).toEqual(['Old']);
	});

	it('counts that cannot be read are unknown, not 0', async () => {
		api.usage.mockRejectedValueOnce(new ApiError(403, 'forbidden', 'Нет права', null));
		const c = await loaded();
		expect(c.active.map((r) => [r.label, r.count])).toEqual([
			['Alpha', null],
			['Beta', null],
			['Gamma', null]
		]);
		expect(c.loadError).toBeNull();
		// The next reading that succeeds brings them back.
		await c.refresh();
		expect(c.active.map((r) => r.count)).toEqual([3, 0, 0]);
	});

	it('filters by the query without case; the archive too', async () => {
		const c = await loaded();
		c.query = ' ALP';
		expect(c.active.map((r) => r.label)).toEqual(['Alpha']);
		expect(c.archived).toEqual([]);
		c.query = 'OL';
		expect(c.archived.map((r) => r.label)).toEqual(['Old']);
	});

	it('reorders at once and sends all ids with the archived ones last', async () => {
		const c = await loaded();
		const sent = c.reorder([3, 1, 2]);
		expect(c.active.map((r) => r.id)).toEqual([3, 1, 2]);
		expect(await sent).toBeNull();
		expect(api.reorder).toHaveBeenCalledWith('brand', [3, 1, 2, 4]);
	});

	it('returns the old order when the server refuses', async () => {
		const c = await loaded();
		api.reorder.mockRejectedValueOnce(new ApiError(403, 'forbidden', 'Нельзя', null));
		const error = await c.reorder([3, 2, 1]);
		expect(error).toBe('Нельзя');
		expect(c.active.map((r) => r.id)).toEqual([1, 2, 3]);
	});

	it('a stale order is refused once: the move is made again on the fresh list', async () => {
		const c = await loaded();
		api.reorder.mockRejectedValueOnce(new ApiError(422, 'validation', 'Список изменился', null));
		api.list.mockResolvedValue([
			item(1, 'Alpha'),
			item(2, 'Beta'),
			item(3, 'Gamma'),
			item(5, 'New'),
			item(4, 'Old', true)
		]);
		expect(await c.reorder([3, 1, 2])).toBeNull();
		expect(api.reorder).toHaveBeenCalledTimes(2);
		expect(api.reorder).toHaveBeenLastCalledWith('brand', [3, 1, 2, 4, 5]);
	});

	it('reorders the filtered rows inside their own slots', async () => {
		const c = await loaded();
		c.query = 'a';
		await c.reorder([3, 2, 1].filter((id) => id !== 2));
		expect(api.reorder).toHaveBeenCalledWith('brand', [3, 2, 1, 4]);
	});

	it('answers a duplicate and an empty value in words', async () => {
		const c = await loaded();
		api.add.mockRejectedValueOnce(validation('value', 'duplicate'));
		expect(await c.add('alpha')).toBe('Такое значение уже есть — объедините их');
		api.add.mockRejectedValueOnce(validation('value', 'required'));
		expect(await c.add(' ')).toBe('Обязательное поле');
	});

	it('answers a comma in a list and a change of the warranty term in words', async () => {
		const c = await loaded();
		api.add.mockRejectedValueOnce(validation('value', 'comma'));
		expect(await c.add('Чехол, стекло')).toBe(
			'В списке запятая разделяет значения — введите одно значение без запятой'
		);
		api.rename.mockRejectedValueOnce(validation('value', 'changes_term'));
		const term =
			'Срок гарантии в заказах изменился бы — замените значение только в справочнике, без замены в заказах';
		expect((await c.renameEverywhere(1, '60 дней', true)).error).toBe(term);
		api.merge.mockRejectedValueOnce(validation('target_id', 'changes_term'));
		expect((await c.merge(2, [1], true)).error).toBe(term);
	});

	it('adds, then reads the list and the counts again', async () => {
		const c = await loaded();
		expect(await c.add('Delta')).toBeNull();
		expect(api.add).toHaveBeenCalledWith('brand', 'Delta');
		expect(api.list).toHaveBeenCalledTimes(2);
		expect(api.usage).toHaveBeenCalledTimes(2);
	});

	it('archives and restores through update', async () => {
		const c = await loaded();
		expect(await c.setArchived(2, true)).toBeNull();
		expect(api.update).toHaveBeenCalledWith(2, { archived: true });
	});

	it('renames in orders and tells how many changed', async () => {
		const c = await loaded();
		const r = await c.renameEverywhere(1, 'Omega', true);
		expect(api.rename).toHaveBeenCalledWith(1, { value: 'Omega', in_orders: true });
		expect(r).toEqual({ error: null, changed: 2 });
	});

	it('merges with the target and sources', async () => {
		const c = await loaded();
		const r = await c.merge(2, [1, 3], true);
		expect(api.merge).toHaveBeenCalledWith('brand', {
			target_id: 2,
			source_ids: [1, 3],
			in_orders: true
		});
		expect(r).toEqual({ error: null, changed: 5 });
	});

	it('keeps the answer of the last kind only', async () => {
		const c = createCatalog(api);
		let first!: (v: DictionaryItem[]) => void;
		api.list.mockImplementationOnce(() => new Promise((r) => (first = r)));
		const slow = c.load('brand');
		await c.load('color');
		first([item(7, 'Stale')]);
		await slow;
		expect(c.kind).toBe('color');
		expect(c.active.map((r) => r.label)).not.toContain('Stale');
	});
});
