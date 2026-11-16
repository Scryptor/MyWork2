import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { ServerInfo } from '$api/ServerInfo';
import { POINTS, STATUSES, user } from './fixtures.test-helper.js';
import { POINT_KEY, app, type AppData } from './context.svelte.js';

const SERVER: ServerInfo = {
	app: 'mw',
	version: '0.1.0',
	api_version: 1,
	auth_required: true,
	has_users: true,
	server_time: '2026-11-15T10:24:00'
};
const SETTINGS = { ready_stale_days: 30, default_warranty: '30 дней' } as AppSettings;

const data = (over: Partial<AppData> = {}): AppData => ({
	server: SERVER,
	me: user(),
	statuses: STATUSES,
	points: POINTS,
	settings: SETTINGS,
	...over
});

afterEach(() => {
	vi.unstubAllGlobals();
	localStorage.clear();
});

describe('app context', () => {
	it('answers what the user may do and finds statuses', () => {
		app.set(data());
		expect(app.can('orders_create')).toBe(true);
		expect(app.can('settings_manage')).toBe(false);
		expect(app.statusById(3)?.name).toBe('Готов');
		expect(app.statusById(99)).toBeUndefined();
		expect(app.activePoints.map((p) => p.name)).toEqual(['Центр']);
	});

	it('keeps the chosen point on the device; an archived or unknown one is «Все точки»', () => {
		localStorage.setItem(POINT_KEY, '1');
		app.set(data());
		expect(app.point).toBe(1);
		app.point = null;
		expect(localStorage.getItem(POINT_KEY)).toBeNull();
		app.point = 1;
		expect(localStorage.getItem(POINT_KEY)).toBe('1');

		localStorage.setItem(POINT_KEY, '2');
		app.set(data());
		expect(app.point).toBeNull();
		localStorage.setItem(POINT_KEY, 'x');
		app.set(data());
		expect(app.point).toBeNull();
	});

	it('refreshCatalogs takes new statuses, points and settings from the server', async () => {
		app.set(data());
		const renamed = STATUSES.map((s) => (s.id === 3 ? { ...s, name: 'Готов к выдаче' } : s));
		vi.stubGlobal(
			'fetch',
			vi.fn(async (url: string) => {
				const body = url.endsWith('/statuses')
					? renamed
					: url.endsWith('/service-points')
						? POINTS.slice(0, 1)
						: { ...SETTINGS, ready_stale_days: 10 };
				return new Response(JSON.stringify(body), { status: 200 });
			})
		);
		await app.refreshCatalogs();
		expect(app.statusById(3)?.name).toBe('Готов к выдаче');
		expect(app.points).toHaveLength(1);
		expect(app.settings.ready_stale_days).toBe(10);
		expect(app.me.name).toBe('Тестовый');
	});
});
