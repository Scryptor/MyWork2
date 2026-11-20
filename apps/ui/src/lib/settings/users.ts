import type { User } from '$api/User';
import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

/** What the server found wrong in each field of a 422, in its own words (`last_admin`, `duplicate`, …). */
export function userFieldErrors(e: unknown): Record<string, string> {
	if (!(e instanceof ApiError) || e.code !== 'validation') return {};
	return Object.fromEntries((e.body?.fields ?? []).map((f) => [f.field, f.message || e.message]));
}

/** The name of a user as lists show it: the display name, else the login. */
export const userTitle = (u: Pick<User, 'display_name' | 'login'>) =>
	u.display_name || u.login || ru.users.editTitle;

/** The failure as a text: field messages together, or the general text. */
export function userFailure(e: unknown): string {
	const fields = Object.values(userFieldErrors(e));
	return fields.length > 0 ? fields.join('; ') : errorMessage(e);
}

/** The line under a user's name: login, or why there is none. */
export function signInLine(u: Pick<User, 'login' | 'can_sign_in' | 'has_password'>): string {
	if (!u.can_sign_in) return ru.users.noSignIn;
	return u.has_password ? u.login : `${u.login} · ${ru.users.noPassword}`;
}
