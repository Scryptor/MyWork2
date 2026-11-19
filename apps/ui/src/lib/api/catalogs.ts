import type { AboutInfo } from '$api/AboutInfo';
import type { AppSettings } from '$api/AppSettings';
import type { DictionaryItem } from '$api/DictionaryItem';
import type { DictionaryItemPatch } from '$api/DictionaryItemPatch';
import type { DictionaryKind } from '$api/DictionaryKind';
import type { NextOrderNumber } from '$api/NextOrderNumber';
import type { ServicePoint } from '$api/ServicePoint';
import type { SettingsPatch } from '$api/SettingsPatch';
import type { Status } from '$api/Status';
import { request } from './client.js';

export const listStatuses = () => request<Status[]>('GET', '/statuses');

export const listServicePoints = () => request<ServicePoint[]>('GET', '/service-points');

export const listDictionary = (kind: DictionaryKind) =>
	request<DictionaryItem[]>('GET', `/dictionaries/${kind}`);

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
