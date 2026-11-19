import type { AboutInfo } from '$api/AboutInfo';
import type { AppSettings } from '$api/AppSettings';
import type { DictionaryItem } from '$api/DictionaryItem';
import type { DictionaryItemPatch } from '$api/DictionaryItemPatch';
import type { CatalogUsage } from '$api/CatalogUsage';
import type { DictionaryKind } from '$api/DictionaryKind';
import type { DictionaryMerge } from '$api/DictionaryMerge';
import type { DictionaryRename } from '$api/DictionaryRename';
import type { NextOrderNumber } from '$api/NextOrderNumber';
import type { RewriteResult } from '$api/RewriteResult';
import type { ServicePoint } from '$api/ServicePoint';
import type { SettingsPatch } from '$api/SettingsPatch';
import type { Status } from '$api/Status';
import { request } from './client.js';

export const listStatuses = () => request<Status[]>('GET', '/statuses');

export const listServicePoints = () => request<ServicePoint[]>('GET', '/service-points');

/** The values of a dictionary; with `archived` also the hidden ones (intake asks without it). */
export const listDictionary = (kind: DictionaryKind, archived = false) =>
	request<DictionaryItem[]>('GET', `/dictionaries/${kind}`, {
		query: archived ? { archived: true } : undefined
	});

/** How many orders (clients, for `client_source`) use each value; unused values are absent. */
export const dictionaryUsage = (kind: DictionaryKind) =>
	request<CatalogUsage[]>('GET', `/dictionaries/${kind}/usage`);

export const deleteDictionaryItem = (id: number) =>
	request<void>('DELETE', `/dictionary-items/${id}`);

/** The new order of ALL the values of the kind, archived ones included. */
export const reorderDictionary = (kind: DictionaryKind, orderedIds: number[]) =>
	request<void>('PUT', `/dictionaries/${kind}/order`, { body: { ordered_ids: orderedIds } });

export const renameDictionaryItem = (id: number, body: DictionaryRename) =>
	request<RewriteResult>('POST', `/dictionary-items/${id}/rename`, { body });

export const mergeDictionary = (kind: DictionaryKind, body: DictionaryMerge) =>
	request<RewriteResult>('POST', `/dictionaries/${kind}/merge`, { body });

export const addDictionaryItem = (kind: DictionaryKind, value: string) =>
	request<DictionaryItem>('POST', `/dictionaries/${kind}`, { body: { value } });

export const updateDictionaryItem = (id: number, patch: DictionaryItemPatch) =>
	request<DictionaryItem>('PATCH', `/dictionary-items/${id}`, { body: patch });

export const getSettings = () => request<AppSettings>('GET', '/settings');

export const updateSettings = (patch: SettingsPatch) =>
	request<AppSettings>('PATCH', '/settings', { body: patch });

export const getNextOrderNumber = () => request<NextOrderNumber>('GET', '/orders/next-number');

export const setNextOrderNumber = (next: number) =>
	request<void>('PUT', '/orders/next-number', { body: { next } });

export const getAbout = () => request<AboutInfo>('GET', '/about');
