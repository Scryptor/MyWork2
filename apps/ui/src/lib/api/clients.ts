import type { Client } from '$api/Client';
import type { ClientListItem } from '$api/ClientListItem';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import type { ClientPatch } from '$api/ClientPatch';
import type { MergeClients } from '$api/MergeClients';
import { request } from './client.js';

export const listClients = (q: ClientListQuery, signal?: AbortSignal) =>
	request<ClientPage>('GET', '/clients', { query: q, signal });

export const clientsByPhone = (phone: string, signal?: AbortSignal) =>
	request<Client[]>('GET', '/clients/by-phone', { query: { phone }, signal });

export const getClient = (id: number) => request<Client>('GET', `/clients/${id}`);

/** Phones replace the list whole; the first one is the main number. */
export const updateClient = (id: number, version: number, patch: ClientPatch) =>
	request<Client>('PATCH', `/clients/${id}`, { query: { version }, body: patch });

/** Clients with the same name or a common phone. */
export const similarClients = (id: number) =>
	request<ClientListItem[]>('GET', `/clients/${id}/similar`);

/** The sources go into the target and are deleted; any stale version is a 409 and changes nothing. */
export const mergeClients = (m: MergeClients) =>
	request<Client>('POST', '/clients/merge', { body: m });
