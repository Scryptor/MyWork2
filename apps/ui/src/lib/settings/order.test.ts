import { describe, expect, it, vi } from 'vitest';
import { movedId, putOrder, reapplyMove } from './order.js';

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
	it('first place: before its successor', () => {
		expect(reapplyMove([1, 2, 3, 9], [3, 1, 2], 3)).toEqual([3, 1, 2, 9]);
	});
});

describe('putOrder', () => {
	it('sends once when the server accepts', async () => {
		const put = vi.fn(async () => {});
		const fresh = vi.fn(async () => [1, 2]);
		expect(await putOrder({ ordered: [2, 1], moved: 2, put, fresh })).toEqual([2, 1]);
		expect(put).toHaveBeenCalledTimes(1);
		expect(fresh).not.toHaveBeenCalled();
	});
	it('a refusal with a changed list: read again, move again, retry once', async () => {
		const put = vi.fn(async () => {});
		put.mockRejectedValueOnce(new Error('stale'));
		const saved = await putOrder({
			ordered: [1, 3, 2],
			moved: 3,
			put,
			fresh: async () => [1, 2, 3, 9]
		});
		expect(saved).toEqual([1, 3, 2, 9]);
		expect(put).toHaveBeenLastCalledWith([1, 3, 2, 9]);
	});
	it('a refusal of the same list is not retried', async () => {
		const put = vi.fn(async () => {
			throw new Error('no right');
		});
		await expect(
			putOrder({ ordered: [2, 1], moved: 2, put, fresh: async () => [1, 2] })
		).rejects.toThrow('no right');
		expect(put).toHaveBeenCalledTimes(1);
	});
	it('a second refusal is thrown', async () => {
		const put = vi.fn(async () => {
			throw new Error('again');
		});
		await expect(
			putOrder({ ordered: [2, 1], moved: 2, put, fresh: async () => [1, 2, 9] })
		).rejects.toThrow('again');
		expect(put).toHaveBeenCalledTimes(2);
	});
});
