import { describe, expect, it } from 'vitest';
import { loginPath, safeNext } from './login.js';

const ORIGIN = 'http://127.0.0.1:4174';

describe('safeNext', () => {
	it('keeps a path of this program with its query', () => {
		expect(safeNext('/orders?chips=all', ORIGIN)).toBe('/orders?chips=all');
		expect(safeNext('/orders/15#history', ORIGIN)).toBe('/orders/15#history');
	});

	it('anything else goes to /orders', () => {
		for (const raw of [
			null,
			'',
			'orders',
			'//evil.example/x',
			'/\\evil.example',
			'https://evil.example/',
			'javascript:alert(1)',
			'/login?next=/orders'
		]) {
			expect(safeNext(raw, ORIGIN)).toBe('/orders');
		}
	});
});

describe('loginPath', () => {
	it('carries the current page in next', () => {
		expect(loginPath('/orders?chips=all')).toBe('/login?next=%2Forders%3Fchips%3Dall');
	});
	it('the login page and the root need no next', () => {
		expect(loginPath('/login?next=%2Forders')).toBe('/login');
		expect(loginPath('/')).toBe('/login');
	});
});
