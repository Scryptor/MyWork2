// Notices for the login page: why the user is there (the session expired, another tab signed in, the
// password changed). The login page resets the session when it opens, and that clears every toast; a
// notice shown before that is lost whenever the page mounts after it. So the notice waits here and the
// login page shows it right after its own reset: the order no longer depends on timing.

export interface LoginNotice {
	message: string;
	tone?: 'success';
}

let waiting: LoginNotice[] = [];

/** Queues a notice for the login page that is about to open. */
export function queueLoginNotice(notice: LoginNotice): void {
	waiting.push(notice);
}

/** The queued notices, once. */
export function takeLoginNotices(): LoginNotice[] {
	const out = waiting;
	waiting = [];
	return out;
}
