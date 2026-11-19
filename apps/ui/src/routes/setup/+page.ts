import { redirect } from '@sveltejs/kit';
import type { ServerInfo } from '$api/ServerInfo';
import { serverInfo } from '#lib/api/auth.js';
import { currentLegacyImport } from '#lib/api/import.js';
import { getToken } from '#lib/api/session.js';
import { failLoad } from '#lib/app/load-error.js';
import { setupGate } from '#lib/setup/route.js';

// The first run (spec 6.9): only while the base is still to be set up, or an import started here is under way.
export async function load() {
	let server: ServerInfo;
	try {
		server = await serverInfo();
	} catch (e) {
		failLoad(e);
	}
	const to = await setupGate(server, getToken() !== null, currentLegacyImport);
	if (to !== null) redirect(307, to);
}
