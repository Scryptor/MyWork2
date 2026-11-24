import MapPin from '@lucide/svelte/icons/map-pin';
import Warehouse from '@lucide/svelte/icons/warehouse';
import type { Component } from 'svelte';
import type { AuditEntry } from '$api/AuditEntry';

/** Entries that move things between the stock and an order: they carry the warehouse icon. */
const STOCK_ACTIONS = ['order.part_added', 'order.part_returned', 'order.costs_split'];

function actionIcon(action: string): Component | undefined {
	if (STOCK_ACTIONS.includes(action)) return Warehouse;
	// The order went to another service point.
	if (action === 'order.moved') return MapPin;
	return undefined;
}

/** History lines for `Timeline`: the server writes the text of each entry in Russian. */
export function historyItems(entries: readonly AuditEntry[]): {
	id: number;
	at: string;
	who: string;
	text: string;
	icon?: Component;
}[] {
	return entries.map((e) => {
		const icon = actionIcon(e.action);
		return { id: e.id, at: e.at, who: e.user_name, text: e.summary, ...(icon ? { icon } : {}) };
	});
}

function statusMove(e: AuditEntry): { from: number; to: number } | null {
	if (e.action !== 'order.status_changed') return null;
	const d = e.details;
	if (typeof d !== 'object' || d === null || Array.isArray(d)) return null;
	const { from, to } = d as Record<string, unknown>;
	return typeof from === 'number' && typeof to === 'number' ? { from, to } : null;
}

/**
 * «Вернуть в работу»: the status the order had before it got the current one — the newest
 * `order.status_changed` into it. Null when there is none or that status cannot be chosen now.
 * `entries` come newest first, as the server gives them.
 */
export function statusBeforeIssue(
	entries: readonly AuditEntry[],
	current: number,
	choices: readonly number[]
): number | null {
	for (const e of entries) {
		const move = statusMove(e);
		if (move?.to === current) return choices.includes(move.from) ? move.from : null;
	}
	return null;
}

/** Entries of «Статусы и сроки»: the intake, every status the order got, warranty cases. */
const TIMELINE_ACTIONS = [
	'order.created',
	'order.status_changed',
	'legacy.status',
	'order.warranty_case'
];

export function statusTimeline(entries: readonly AuditEntry[]) {
	return historyItems(entries.filter((e) => TIMELINE_ACTIONS.includes(e.action)));
}

export interface WarrantyCaseItem {
	id: number;
	/** When the client came back; the time of the entry when the details do not say. */
	at: string;
	complaint: string;
}

/** «Гарантийные случаи»: the date and the complaint of each `order.warranty_case`. */
export function warrantyCases(entries: readonly AuditEntry[]): WarrantyCaseItem[] {
	return entries
		.filter((e) => e.action === 'order.warranty_case')
		.map((e) => {
			const d = e.details;
			const fields =
				typeof d === 'object' && d !== null && !Array.isArray(d)
					? (d as Record<string, unknown>)
					: {};
			return {
				id: e.id,
				at: typeof fields.at === 'string' ? fields.at : e.at,
				complaint: typeof fields.complaint === 'string' ? fields.complaint : e.summary
			};
		});
}

/** A fresh first page over the entries shown: new ones on top, each entry once, newest first. */
export function mergeHistory(
	shown: readonly AuditEntry[],
	fresh: readonly AuditEntry[]
): AuditEntry[] {
	const byId = new Map(shown.map((e) => [e.id, e]));
	for (const e of fresh) byId.set(e.id, e);
	return [...byId.values()].sort((a, b) => b.id - a.id);
}

export interface HistoryCursor {
	before_at: string;
	before_id: number;
}

/**
 * The cursor after a page as the server gave it (time, then id, both falling): its last entry. The next
 * page goes on from the last page read, not from the oldest entry shown: entries added on top later (a
 * refresh) may have an earlier time when the clock went back, and would make it skip the rest.
 */
export function historyCursor(page: readonly AuditEntry[]): HistoryCursor | undefined {
	const last = page.at(-1);
	return last && { before_at: last.at, before_id: last.id };
}
