import type { Client } from '$api/Client';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import { request } from './client.js';

export const listClients = (q: ClientListQuery) =>
	request<ClientPage>('GET', '/clients', { query: q });

export const clientsByPhone = (phone: string) =>
	request<Client[]>('GET', '/clients/by-phone', { query: { phone } });

export const getClient = (id: number) => request<Client>('GET', `/clients/${id}`);
