// After a transfer that wrote users of its own (or a replacement, which closed every session) nobody is
// signed in here any more: the token is dropped, the sign-in leads to the report of the transfer.
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { clearToken } from '#lib/api/session.js';
import { loginPath } from '#lib/app/login.js';
import { LATEST_REPORT } from './report.js';

/** «Войти» after a transfer: to the sign-in, then to the report. */
export function signInForReport(): void {
	clearToken();
	void goto(resolve(loginPath(LATEST_REPORT)));
}
