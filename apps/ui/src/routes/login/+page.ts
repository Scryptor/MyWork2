import { redirect } from '@sveltejs/kit';
import type { ServerInfo } from '$api/ServerInfo';
import { serverInfo } from '#lib/api/auth.js';
import { failLoad } from '#lib/app/load-error.js';

// Without users on a local address the program works without signing in (open mode).
export async function load() {
	let server: ServerInfo;
	try {
		server = await serverInfo();
	} catch (e) {
		failLoad(e);
	}
	if (!server.auth_required) redirect(307, '/orders');
}
