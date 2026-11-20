import type { NewUser } from '$api/NewUser';
import type { Role } from '$api/Role';
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
