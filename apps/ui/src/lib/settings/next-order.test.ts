import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { createNextOrder } from './next-order.svelte.js';

const tooSmall = () =>
	new ApiError(422, 'validation', 'Ошибка', {
		code: 'validation',
		message: 'Ошибка',
		fields: [{ field: 'next', code: 'too_small', message: 'x' }],
		current_version: null,
		permission: null,
		retry_after_secs: null
	});

function deferred() {
	let resolve!: () => void;
	let reject!: (e: unknown) => void;
	const promise = new Promise<void>((res, rej) => {
		resolve = res;
		reject = rej;
	});
	return { promise, resolve, reject };
}

function make(opts: { can?: boolean; next?: number } = {}) {
	const rights = { can: opts.can ?? true };
	const get = vi.fn(async () => ({ next: opts.next ?? 100 }));
	const set = vi.fn<(next: number) => Promise<void>>(async () => {});
	const n = createNextOrder({ can: () => rights.can, get, set });
	return { n, get, set, rights };
}

describe('createNextOrder', () => {
	it('loads the number once there is the right — also when the right comes later', async () => {
		const { n, get, rights } = make({ can: false });
		await n.load();
		expect(get).not.toHaveBeenCalled();
		rights.can = true;
		await n.load();
		expect(get).toHaveBeenCalledTimes(1);
		expect(n.text).toBe('100');
		await n.load();
		expect(get).toHaveBeenCalledTimes(1);
	});

	it('saves a changed number and shows it', async () => {
		const { n, set } = make();
		await n.load();
		n.text = ' 250 ';
		await n.commit();
		expect(set).toHaveBeenCalledWith(250);
		expect(n.text).toBe('250');
		expect(n.error).toBeUndefined();
		// The same again: nothing to send.
		await n.commit();
		expect(set).toHaveBeenCalledTimes(1);
	});

	it('leaving the field while it is being saved queues the newer value', async () => {
		const { n, set } = make();
		await n.load();
		const first = deferred();
		set.mockReturnValueOnce(first.promise);
		n.text = '200';
		const running = n.commit();
		n.text = '300';
		void n.commit();
		expect(set).toHaveBeenCalledTimes(1);
		first.resolve();
		await running;
		expect(set).toHaveBeenCalledTimes(2);
		expect(set).toHaveBeenLastCalledWith(300);
		expect(n.text).toBe('300');
	});

	it('a failure does not overwrite what was typed meanwhile, nor show its mistake over it', async () => {
		const { n, set } = make();
		await n.load();
		const first = deferred();
		set.mockReturnValueOnce(first.promise);
		n.text = '50';
		const running = n.commit();
		n.text = '60';
		first.reject(tooSmall());
		await running;
		expect(n.text).not.toBe('100');
		expect(n.error).toBeUndefined();
	});

	it('a failure with nothing typed after it: the number of the server is back, the mistake shown', async () => {
		const { n, set } = make();
		await n.load();
		set.mockRejectedValueOnce(tooSmall());
		n.text = '5';
		await n.commit();
		expect(n.text).toBe('100');
		expect(n.error).toBe('Номер должен быть больше номеров существующих заказов');
	});

	it('a number over 999 999 999 is «Слишком большой номер» and is not sent', async () => {
		const { n, set } = make();
		await n.load();
		n.text = '1000000000';
		await n.commit();
		expect(set).not.toHaveBeenCalled();
		expect(n.error).toBe('Слишком большой номер');
		n.text = '999999999';
		await n.commit();
		expect(set).toHaveBeenCalledWith(999_999_999);
	});

	it('what is not a number is not sent', async () => {
		const { n, set } = make();
		await n.load();
		n.text = '12x';
		await n.commit();
		expect(set).not.toHaveBeenCalled();
		expect(n.error).toBe('Введите номер, например 10000');
		expect(n.text).toBe('100');
	});

	it('without the right nothing is sent', async () => {
		const { n, set, rights } = make();
		await n.load();
		rights.can = false;
		n.text = '500';
		await n.commit();
		expect(set).not.toHaveBeenCalled();
	});
});
