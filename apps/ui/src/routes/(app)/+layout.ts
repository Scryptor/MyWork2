import { redirect } from '@sveltejs/kit';
import { serverInfo, me } from '#lib/api/auth.js';
import { getSettings, listServicePoints, listStatuses } from '#lib/api/catalogs.js';
import { ApiError } from '#lib/api/client.js';
import { serverClock } from '#lib/api/server-clock.svelte.js';
import { clearToken, getToken } from '#lib/api/session.js';
import { app, type AppData } from '#lib/app/context.svelte.js';
import { loginPath } from '#lib/app/login.js';
import type { LayoutLoad } from './$types';

// `url` is read only on the way to the login page: the data stays while the user moves between
// screens, the load does not run again on every navigation.
export const load: LayoutLoad = async ({ url }): Promise<AppData> => {
	const server = await serverInfo();
	serverClock.sync(server.server_time);
	if (server.auth_required && !getToken()) redirect(307, loginPath(url.pathname + url.search));
	try {
		const [user, statuses, points, settings] = await Promise.all([
			me(),
			listStatuses(),
			listServicePoints(),
			getSettings()
		]);
		const data: AppData = { server, me: user, statuses, points, settings };
		app.set(data);
		return data;
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) {
			clearToken();
			redirect(307, loginPath(url.pathname + url.search));
		}
		throw e;
	}
};
