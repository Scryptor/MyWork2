import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { movedId, putOrder, reapplyMove } from './order.js';

const stale = () =>
	new ApiError(422, 'validation', 'Порядок тем временем изменили', {
		code: 'validation',
		message: 'Порядок тем временем изменили',
		fields: [{ field: 'ordered_ids', code: 'stale', message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

const refusal = (status: number, field: string, code: string) =>
	new ApiError(status, 'validation', 'Отказ', {
		code: 'validation',
		message: 'Отказ',
		fields: [{ field, code, message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});
/** The ids of the sent list are not the ids of the server: a mistake of `ordered_ids`, not of rights. */
const otherIds = () => refusal(422, 'ordered_ids', 'invalid');

describe('movedId', () => {
	it('finds the one id that moved', () => {
		expect(movedId([1, 2, 3], [1, 3, 2])).toBe(2);
		expect(movedId([1, 2, 3], [3, 1, 2])).toBe(3);
		expect(movedId([1, 2], [1, 2, 3])).toBeNull();
	});
});

describe('reapplyMove', () => {
	it('puts the moved id after its predecessor; new ids stay where the server has them', () => {
		// The user moved 3 between 1 and 2; somebody added 9 at the end meanwhile.
		expect(reapplyMove([1, 2, 3, 9], [1, 3, 2], 3)).toEqual([1, 3, 2, 9]);
		// The new id sits right after the predecessor: the moved id goes next to the predecessor.
		expect(reapplyMove([1, 9, 2, 3], [1, 3, 2], 3)).toEqual([1, 3, 9, 2]);
	});
	it('a moved id that the server no longer has does not come back', () => {
		expect(reapplyMove([1, 2], [1, 3, 2], 3)).toEqual([1, 2]);
		expect(reapplyMove([2, 9], [3, 2], 3)).toEqual([2, 9]);
	});
	it('first place: before its successor', () => {
		expect(reapplyMove([1, 2, 3, 9], [3, 1, 2], 3)).toEqual([3, 1, 2, 9]);
	});
});

describe('putOrder', () => {
	it('sends once, with the order it was made from, when the server accepts', async () => {
		const put = vi.fn(async () => {});
		const fresh = vi.fn(async () => [1, 2]);
		expect(await putOrder({ ordered: [2, 1], seen: [1, 2], moved: 2, put, fresh })).toEqual([2, 1]);
		expect(put).toHaveBeenCalledTimes(1);
		expect(put).toHaveBeenCalledWith([2, 1], [1, 2]);
		expect(fresh).not.toHaveBeenCalled();
	});
	it('a refusal with a changed list: read again, move again, retry once', async () => {
		const put = vi.fn(async () => {});
		put.mockRejectedValueOnce(otherIds());
		const saved = await putOrder({
			ordered: [1, 3, 2],
			seen: [1, 2, 3],
			moved: 3,
			put,
			fresh: async () => [1, 2, 3, 9]
		});
		expect(saved).toEqual([1, 3, 2, 9]);
		expect(put).toHaveBeenLastCalledWith([1, 3, 2, 9], [1, 2, 3, 9]);
	});
	it('a refusal because another screen moved something: its move stays, ours is made on top', async () => {
		const put = vi.fn(async () => {});
		put.mockRejectedValueOnce(otherIds());
		// We moved 3 to the top of [1, 2, 3, 4]; another screen meanwhile put 4 before 2.
		const saved = await putOrder({
			ordered: [3, 1, 2, 4],
			seen: [1, 2, 3, 4],
			moved: 3,
			put,
			fresh: async () => [1, 4, 2, 3]
		});
		expect(saved).toEqual([3, 1, 4, 2]);
		expect(put).toHaveBeenLastCalledWith([3, 1, 4, 2], [1, 4, 2, 3]);
	});
	it('a refusal of the very order we saw is not retried', async () => {
		const put = vi.fn(async () => {
			throw new Error('no right');
		});
		await expect(
			putOrder({ ordered: [2, 1], seen: [1, 2], moved: 2, put, fresh: async () => [1, 2] })
		).rejects.toThrow('no right');
		expect(put).toHaveBeenCalledTimes(1);
	});
	it('«stale» is retried even when the order read back is the one we saw (it settled meanwhile)', async () => {
		const put = vi.fn(async () => {});
		put.mockRejectedValueOnce(stale());
		const saved = await putOrder({
			ordered: [2, 1],
			seen: [1, 2],
			moved: 2,
			put,
			fresh: async () => [1, 2]
		});
		expect(saved).toEqual([2, 1]);
		expect(put).toHaveBeenLastCalledWith([2, 1], [1, 2]);
	});
	it('a refusal of rights or a failure of the server is not retried, even when the list changed', async () => {
		for (const e of [
			new ApiError(403, 'forbidden', 'Нет прав', {
				code: 'forbidden',
				message: 'Нет прав',
				fields: [],
				current_version: null,
				permission: 'settings_manage',
				retry_after_secs: null
			}),
			new ApiError(500, 'internal', 'Сбой', null),
			new TypeError('Failed to fetch')
		]) {
			const put = vi.fn(async () => {
				throw e;
			});
			const fresh = vi.fn(async () => [1, 2, 9]);
			await expect(putOrder({ ordered: [2, 1], seen: [1, 2], moved: 2, put, fresh })).rejects.toBe(
				e
			);
			expect(put).toHaveBeenCalledTimes(1);
			expect(fresh).not.toHaveBeenCalled();
		}
	});
	it('a second refusal is thrown', async () => {
		const put = vi.fn(async () => {
			throw otherIds();
		});
		await expect(
			putOrder({ ordered: [2, 1], seen: [1, 2], moved: 2, put, fresh: async () => [1, 2, 9] })
		).rejects.toBeInstanceOf(ApiError);
		expect(put).toHaveBeenCalledTimes(2);
	});
});
