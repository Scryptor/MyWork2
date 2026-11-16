import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { toast } from '#lib/ui/toast/toast.svelte.js';
import { undo } from './undo.svelte.js';

const messages = () => toast.items.map((t) => t.message);

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
	for (const t of [...toast.items]) toast.dismiss(t.id);
	vi.useRealTimers();
});

describe('undo', () => {
	it('runs the last pushed action once', async () => {
		const a = vi.fn(async () => {});
		const b = vi.fn(async () => {});
		undo.push('№ 1 → Готов', a);
		undo.push('№ 2 → Готов', b);
		await undo.run();
		expect(b).toHaveBeenCalledTimes(1);
		expect(a).not.toHaveBeenCalled();
		await undo.run();
		expect(a).toHaveBeenCalledTimes(1);
		expect(b).toHaveBeenCalledTimes(1);
		await undo.run();
		expect(a).toHaveBeenCalledTimes(1);
	});

	it('push shows «label · Отменить»; the button runs the same action and Ctrl+Z then has nothing', async () => {
		const a = vi.fn(async () => {});
		undo.push('№ 1 → Готов', a);
		expect(messages()).toEqual(['№ 1 → Готов']);
		const shown = toast.items[0]!;
		expect(shown.action?.label).toBe('Отменить');
		await shown.action?.run();
		expect(a).toHaveBeenCalledTimes(1);
		expect(messages()).toEqual([]);
		await undo.run();
		expect(a).toHaveBeenCalledTimes(1);
	});

	it('run removes the toast of what it undid', async () => {
		undo.push('№ 1 → Готов', async () => {});
		undo.push('№ 2 → Готов', async () => {});
		await undo.run();
		expect(messages()).toEqual(['№ 1 → Готов']);
	});

	it('an action is undoable while its toast is on screen', async () => {
		const a = vi.fn(async () => {});
		undo.push('№ 1 → Готов', a);
		vi.advanceTimersByTime(6000);
		expect(messages()).toEqual([]);
		await undo.run();
		expect(a).not.toHaveBeenCalled();
	});

	it('a failed undo says so', async () => {
		undo.push('№ 1 → Готов', async () => {
			throw new ApiError(0, 'network', 'Нет связи с сервером', null);
		});
		await undo.run();
		expect(messages()).toEqual(['Не удалось отменить: Нет связи с сервером']);
		expect(toast.items[0]!.tone).toBe('danger');
	});
});
