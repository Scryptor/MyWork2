import type { NewUser } from '$api/NewUser';
import type { NewRole } from '$api/NewRole';
import type { Role } from '$api/Role';
import type { RolePatch } from '$api/RolePatch';
import type { User } from '$api/User';
import type { UserPatch } from '$api/UserPatch';
import { request } from './client.js';

/** Every user, disabled ones too (the right «Пользователи и права»). */
export const listUsers = () => request<User[]>('GET', '/users');

export const createUser = (body: NewUser) => request<User>('POST', '/users', { body });

export const updateUser = (id: number, version: number, patch: UserPatch) =>
	request<User>('PATCH', `/users/${id}`, { query: { version }, body: patch });

/** A new password for a user; the user's sessions end. */
export const setUserPassword = (id: number, password: string) =>
	request<void>('PUT', `/users/${id}/password`, { body: { password } });

export const listRoles = () => request<Role[]>('GET', '/roles');

export const createRole = (body: NewRole) => request<Role>('POST', '/roles', { body });

export const updateRole = (id: number, version: number, patch: RolePatch) =>
	request<Role>('PATCH', `/roles/${id}`, { query: { version }, body: patch });

/** Only a role without users goes; the server says why not (users in it, the last «Пользователи и права»). */
export const deleteRole = (id: number, version: number) =>
	request<void>('DELETE', `/roles/${id}`, { query: { version } });
