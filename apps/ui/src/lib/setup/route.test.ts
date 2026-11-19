import { describe, expect, it, vi } from 'vitest';
import type { ImportJob } from '$api/ImportJob';
import type { ServerInfo } from '$api/ServerInfo';
import { ApiError } from '#lib/api/client.js';
import { appGate, setupGate } from './route.js';

const info = (over: Partial<ServerInfo> = {}): ServerInfo => ({
	app: 'mw',
	version: '0.1.0',
	api_version: 1,
	auth_required: false,
	has_users: false,
	setup_required: false,
	server_time: '2026-11-19T10:00:00',
	...over
});
const job = (over: Partial<ImportJob> = {}): ImportJob => ({
	stage: 'copying',
	percent: 30,
	error: null,
	import_id: null,
	...over
});

describe('appGate: where a screen of the program sends the user first', () => {
	it('a base still to be set up: the first run, before any sign-in', () => {
		const here = vi.fn(() => '/orders');
		expect(appGate(info({ setup_required: true }), false, here)).toBe('/setup');
		expect(appGate(info({ setup_required: true, auth_required: true }), false, here)).toBe(
			'/setup'
		);
		expect(here).not.toHaveBeenCalled();
	});

	it('a set-up base with users: the sign-in, which brings the user back', () => {
		expect(appGate(info({ auth_required: true }), false, () => '/orders?chips=all')).toBe(
			'/login?next=%2Forders%3Fchips%3Dall'
		);
		expect(appGate(info({ auth_required: true }), true, () => '/orders')).toBeNull();
	});

	it('otherwise the screen opens, and the address is not read (the load would follow it)', () => {
		const here = vi.fn(() => '/orders');
		expect(appGate(info(), false, here)).toBeNull();
		expect(here).not.toHaveBeenCalled();
	});
});

describe('setupGate: whether the first run shows itself', () => {
	it('a new base: it shows, and asks nothing of the import', async () => {
		const current = vi.fn(async () => null);
		expect(await setupGate(info({ setup_required: true }), false, current)).toBeNull();
		expect(current).not.toHaveBeenCalled();
	});

	it('a new base with users: they sign in first and come back here', async () => {
		const current = vi.fn(async () => null);
		const server = info({ setup_required: true, auth_required: true });
		expect(await setupGate(server, false, current)).toBe('/login?next=%2Fsetup');
		expect(await setupGate(server, true, current)).toBeNull();
	});

	it('a set-up base goes to the orders', async () => {
		expect(await setupGate(info(), false, async () => null)).toBe('/orders');
	});

	it('an import still under way keeps the screen: it wrote the base, its progress goes on', async () => {
		expect(await setupGate(info(), false, async () => job())).toBeNull();
	});

	it('a finished or failed import does not', async () => {
		const done = job({ stage: 'done', percent: 100, import_id: 1 });
		expect(await setupGate(info(), false, async () => done)).toBe('/orders');
		const failed = job({ error: 'Сбой' });
		expect(await setupGate(info(), false, async () => failed)).toBe('/orders');
	});

	it('401: the import brought users; one of them signs in to see the report', async () => {
		const current = async () => {
			throw new ApiError(401, 'unauthenticated', 'Войдите', null);
		};
		expect(await setupGate(info({ auth_required: true }), false, current)).toBe(
			'/login?next=%2Fsettings%2Fimport%2Flatest'
		);
	});

	it('another refusal (a server on the network, no right): the orders', async () => {
		const current = async () => {
			throw new ApiError(403, 'forbidden', 'Только на этом компьютере', null);
		};
		expect(await setupGate(info(), false, current)).toBe('/orders');
	});
});
