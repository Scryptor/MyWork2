import type { ChangePasswordRequest } from '$api/ChangePasswordRequest';
import type { CurrentUser } from '$api/CurrentUser';
import type { LoginRequest } from '$api/LoginRequest';
import type { LoginResponse } from '$api/LoginResponse';
import type { ServerInfo } from '$api/ServerInfo';
import { request } from './client.js';

export const serverInfo = () => request<ServerInfo>('GET', '/server-info');

export const me = () => request<CurrentUser>('GET', '/auth/me');

export const login = (req: LoginRequest) =>
	request<LoginResponse>('POST', '/auth/login', { body: req });

export const logout = () => request<void>('POST', '/auth/logout');

export const changePassword = (req: ChangePasswordRequest) =>
	request<void>('POST', '/auth/password', { body: req });

/** Closes a session on the server by its token, for a tab that lost it; a failure changes nothing. */
export async function revokeToken(token: string): Promise<void> {
	try {
		await request<void>('POST', '/auth/logout', {
			token,
			keepalive: true,
			quietUnauthenticated: true
		});
	} catch {
		// the token is dead already, or there is no connection: it expires by itself
	}
}
