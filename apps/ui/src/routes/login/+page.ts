import { redirect } from '@sveltejs/kit';
import { serverInfo } from '#lib/api/auth.js';

// Without users on a local address the program works without signing in (open mode).
export async function load() {
	const server = await serverInfo();
	if (!server.auth_required) redirect(307, '/orders');
}
