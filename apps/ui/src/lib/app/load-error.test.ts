import { isHttpError } from '@sveltejs/kit';
import { describe, expect, it } from 'vitest';
import { ApiError } from '#lib/api/client.js';
import { failLoad } from './load-error.js';

function thrown(e: unknown): unknown {
	try {
		failLoad(e);
	} catch (caught) {
		return caught;
	}
	return undefined;
}

describe('failLoad', () => {
	it('no connection is 503 «Нет связи с сервером», marked offline', () => {
		const e = thrown(new ApiError(0, 'network', 'Нет связи с сервером', null));
		expect(isHttpError(e, 503)).toBe(true);
		expect(e).toMatchObject({ body: { message: 'Нет связи с сервером', offline: true } });
	});

	it('another answer of the server keeps its status and message', () => {
		const e = thrown(new ApiError(500, 'internal', 'Ошибка базы данных', null));
		expect(isHttpError(e, 500)).toBe(true);
		expect(e).toMatchObject({ body: { message: 'Ошибка базы данных' } });
		expect((e as { body: App.Error }).body.offline).toBeUndefined();
	});

	it('anything else is thrown as it is', () => {
		const bug = new TypeError('x is undefined');
		expect(thrown(bug)).toBe(bug);
	});
});
