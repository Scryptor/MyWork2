import { redirect } from '@sveltejs/kit';
import type { ServerInfo } from '$api/ServerInfo';
import { serverInfo } from '#lib/api/auth.js';
import { failLoad } from '#lib/app/load-error.js';
import { safeNext } from '#lib/app/login.js';
import type { PageLoad } from './$types';

// Without users on a local address the program works without signing in (open mode): straight on to where
// the user was going (a report of an import that left no users, for one).
export const load: PageLoad = async ({ url }) => {
	let server: ServerInfo;
	try {
		server = await serverInfo();
	} catch (e) {
		failLoad(e);
	}
	if (!server.auth_required) redirect(307, safeNext(url.searchParams.get('next'), url.origin));
};
