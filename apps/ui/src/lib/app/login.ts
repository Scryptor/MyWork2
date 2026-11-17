const HOME = '/orders';

/**
 * Where to go after signing in: a path of this program from `next`, or `/orders`. A link elsewhere
 * (`//host`, `/\host`, `https:`) or back to the login page is not followed.
 */
export function safeNext(raw: string | null, origin: string): string {
	if (!raw || !raw.startsWith('/') || raw.startsWith('//') || raw.startsWith('/\\')) return HOME;
	let url: URL;
	try {
		url = new URL(raw, origin);
	} catch {
		return HOME;
	}
	if (url.origin !== origin || url.pathname === '/login' || url.pathname === '/') return HOME;
	return url.pathname + url.search + url.hash;
}

/** The login page that brings the user back to `path` (pathname with the query). */
export function loginPath(path: string): '/login' | `/login?next=${string}` {
	return path === '/' || path.startsWith('/login')
		? '/login'
		: `/login?next=${encodeURIComponent(path)}`;
}

/**
 * The address is the sign-in page: the session ended (expired, signed out, new password). Guards of
 * unsaved edits let such a navigation through — without a session nothing can be saved anyway.
 */
export function isSignIn(url: URL): boolean {
	return url.pathname === '/login';
}
