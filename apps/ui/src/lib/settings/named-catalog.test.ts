import { describe, expect, it, vi } from 'vitest';
import type { Status } from '$api/Status';
import { ApiError } from '#lib/api/client.js';
import { createNamedCatalog, type NamedApi } from './named-catalog.svelte.js';

const st = (id: number, name: string, archived = false): Status => ({
	id,
	name,
	kind: 'open',
	color: 'gray',
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
		retry_after_secs: null,
		diagnostics: []
	});

function make() {
	const api = {
		list: vi.fn(async () => [st(1, 'A'), st(2, 'B'), st(3, 'C'), st(4, 'Old', true)]),
		usage: vi.fn(async () => [{ id: 2, count: 3 }]),
		add: vi.fn(async () => st(9, 'N')),
		update: vi.fn(async () => st(1, 'A')),
		reorder: vi.fn(async () => {})
	} satisfies NamedApi<Status, unknown, unknown>;
	const changed = vi.fn();
	return { api, changed, c: createNamedCatalog<Status, unknown, unknown>(api, changed) };
}

describe('createNamedCatalog', () => {
	it('loads rows with counts; archived ones apart', async () => {
		const { c } = make();
		await c.load();
		expect(c.active.map((r) => [r.label, r.count])).toEqual([
			['A', 0],
			['B', 3],
			['C', 0]
		]);
		expect(c.archived.map((r) => r.label)).toEqual(['Old']);
	});

	it('counts that cannot be read are unknown, not 0', async () => {
		const { c, api } = make();
		api.usage.mockRejectedValueOnce(new Error('offline'));
		await c.load();
		expect(c.active.map((r) => r.count)).toEqual([null, null, null]);
		expect(c.countOf(2)).toBeNull();
		expect(c.loadError).toBeNull();
		await c.refresh();
		expect(c.countOf(2)).toBe(3);
		expect(c.countOf(1)).toBe(0);
	});

	it('without the right to the counts they are not asked for at all', async () => {
		const api = {
			list: vi.fn(async () => [st(1, 'A')]),
			usage: vi.fn(async () => [{ id: 1, count: 3 }]),
			add: vi.fn(async () => st(9, 'N')),
			update: vi.fn(async () => st(1, 'A')),
			reorder: vi.fn(async () => {})
		} satisfies NamedApi<Status, unknown, unknown>;
		const c = createNamedCatalog<Status, unknown, unknown>(
			api,
			() => {},
			() => false
		);
		await c.load();
		await c.refresh();
		expect(api.usage).not.toHaveBeenCalled();
		expect(c.countOf(1)).toBeNull();
	});

	it('a change reloads and tells the program; a refusal gives the text of the mistake', async () => {
		const { c, api, changed } = make();
		await c.load();
		expect(await c.act(() => api.update())).toBeNull();
		expect(changed).toHaveBeenCalledTimes(1);
		api.update.mockRejectedValueOnce(validation('kind', 'last_issued_status'));
		expect(await c.act(() => api.update())).toBe('Нужен хотя бы один статус вида „Выдан“');
		api.update.mockRejectedValueOnce(validation('name', 'duplicate'));
		expect(await c.act(() => api.update())).toBe('Такое название уже есть');
	});

	it('reorder keeps the archived at the end and rolls back on a refusal', async () => {
		const { c, api } = make();
		await c.load();
		expect(await c.reorder([3, 1, 2])).toBeNull();
		expect(api.reorder).toHaveBeenCalledWith([3, 1, 2, 4], [1, 2, 3, 4]);
		expect(c.active.map((r) => r.id)).toEqual([3, 1, 2]);
		// The server has the new order now: a refusal of the order we saw is not retried.
		api.list.mockResolvedValue(
			[st(3, 'C'), st(1, 'A'), st(2, 'B'), st(4, 'Old', true)].map((x, i) => ({
				...x,
				sort_order: i
			}))
		);
		api.reorder.mockRejectedValueOnce(new Error('boom'));
		expect(await c.reorder([1, 2, 3])).not.toBeNull();
		expect(api.reorder).toHaveBeenCalledTimes(2);
		expect(c.active.map((r) => r.id)).toEqual([3, 1, 2]);
	});

	it('an older answer that comes late is dropped', async () => {
		const { c, api } = make();
		let late: (v: Status[]) => void = () => {};
		api.list.mockImplementationOnce(() => new Promise((r) => (late = r)));
		const first = c.load();
		await c.load();
		late([st(77, 'Stale')]);
		await first;
		expect(c.active.map((r) => r.label)).toEqual(['A', 'B', 'C']);
	});

	it('a stale order is refused once: the move is made again on the fresh list and kept', async () => {
		const { c, api } = make();
		await c.load();
		api.reorder.mockRejectedValueOnce(validation('ordered_ids', 'stale'));
		api.list.mockResolvedValue([
			st(1, 'A'),
			st(2, 'B'),
			st(3, 'C'),
			st(5, 'New'),
			st(4, 'Old', true)
		]);
		expect(await c.reorder([3, 1, 2])).toBeNull();
		expect(api.reorder).toHaveBeenCalledTimes(2);
		expect(api.reorder).toHaveBeenLastCalledWith([3, 1, 2, 4, 5], [1, 2, 3, 4, 5]);
	});

	it('another screen moved a status meanwhile: our move is made on top of theirs', async () => {
		const { c, api } = make();
		await c.load();
		api.reorder.mockRejectedValueOnce(validation('ordered_ids', 'stale'));
		// The other screen put C first; the set is the same.
		api.list.mockResolvedValue([
			{ ...st(3, 'C'), sort_order: 0 },
			st(1, 'A'),
			st(2, 'B'),
			st(4, 'Old', true)
		]);
		// We move B before A.
		expect(await c.reorder([2, 1, 3])).toBeNull();
		expect(api.reorder).toHaveBeenCalledTimes(2);
		expect(api.reorder).toHaveBeenLastCalledWith([3, 2, 1, 4], [3, 1, 2, 4]);
	});
});
