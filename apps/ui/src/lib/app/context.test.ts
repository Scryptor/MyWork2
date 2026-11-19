import { afterEach, describe, expect, it, vi } from 'vitest';
import type { AppSettings } from '$api/AppSettings';
import type { ServerInfo } from '$api/ServerInfo';
import { POINTS, STATUSES, user } from './fixtures.test-helper.js';
import { POINT_KEY, app, readCatalogs, type AppData } from './context.svelte.js';

const SERVER: ServerInfo = {
	app: 'mw',
	version: '0.1.0',
	api_version: 1,
	auth_required: true,
	has_users: true,
	setup_required: false,
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
				const path = new URL(url, 'http://localhost').pathname;
				const body = path.endsWith('/statuses')
					? renamed
					: path.endsWith('/service-points')
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

describe('the catalogs of the program', () => {
	// The server as it is: archived statuses and points only when asked for.
	const OLD_STATUS = {
		id: 9,
		name: 'Старый',
		kind: 'open' as const,
		color: 'gray',
		sort_order: 9,
		archived: true
	};
	function serverWithArchive() {
		const fetch = vi.fn(async (url: string) => {
			const u = new URL(url, 'http://localhost');
			const all = u.searchParams.get('include_archived') === 'true';
			const body = u.pathname.endsWith('/statuses')
				? all
					? [...STATUSES, OLD_STATUS]
					: STATUSES
				: u.pathname.endsWith('/service-points')
					? all
						? POINTS
						: POINTS.filter((p) => !p.archived)
					: SETTINGS;
			return new Response(JSON.stringify(body), { status: 200 });
		});
		vi.stubGlobal('fetch', fetch);
		return fetch;
	}

	it('are read with the archived statuses and points: their orders keep a status and a point', async () => {
		serverWithArchive();
		const [statuses, points, settings] = await readCatalogs();
		expect(statuses.map((s) => s.id)).toContain(OLD_STATUS.id);
		expect(points.map((p) => p.name)).toEqual(['Центр', 'Склад']);
		expect(settings).toEqual(SETTINGS);
	});

	it('refreshCatalogs keeps the archived ones too; only the live points can be chosen', async () => {
		app.set(data({ statuses: STATUSES, points: POINTS }));
		serverWithArchive();
		await app.refreshCatalogs();
		expect(app.statusById(OLD_STATUS.id)?.name).toBe('Старый');
		expect(app.points.find((p) => p.id === 2)?.name).toBe('Склад');
		expect(app.activePoints.map((p) => p.name)).toEqual(['Центр']);
	});
});
