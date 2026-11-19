// Where the program sends the user before a screen opens: the first run (spec 6.9) while the base is still to
// be set up, the sign-in when it has users.
import type { ImportJob } from '$api/ImportJob';
import type { ServerInfo } from '$api/ServerInfo';
import { ApiError } from '#lib/api/client.js';
import { loginPath } from '#lib/app/login.js';
import { LATEST_REPORT } from '#lib/import/report.js';
import { jobState } from './progress.js';

export const SETUP_PATH = '/setup';

/**
 * A screen of the program (the `(app)` layout): the first run comes before anything else (it asks to sign
 * in itself when it must); then the sign-in, which brings the user back `here` (the path with the query).
 * Null: the screen opens. `here` is asked only for the sign-in: a load that reads its URL runs again on every
 * navigation (and on every link the pointer rests on).
 */
export function appGate(server: ServerInfo, signedIn: boolean, here: () => string): string | null {
	if (server.setup_required) return SETUP_PATH;
	if (server.auth_required && !signedIn) return loginPath(here());
	return null;
}

/**
 * The first-run screen: null when it shows, otherwise where it sends the user. A set-up base goes to the
 * orders, except while an import started there is under way: the base is written before its files are in
 * place, and its progress stays on the screen. When that import brought users (401), one of them signs in
 * to see its report.
 */
export async function setupGate(
	server: ServerInfo,
	signedIn: boolean,
	current: () => Promise<ImportJob | null>
): Promise<string | null> {
	if (server.setup_required)
		return server.auth_required && !signedIn ? loginPath(SETUP_PATH) : null;
	try {
		const job = await current();
		if (job !== null && jobState(job).kind === 'running') return null;
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) return loginPath(LATEST_REPORT);
	}
	return '/orders';
}
