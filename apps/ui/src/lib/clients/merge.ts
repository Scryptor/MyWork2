// Merging clients (spec 6.6): which client stays, which client each field comes from, and the request.
import type { Client } from '$api/Client';
import type { ClientPhone } from '$api/ClientPhone';
import type { MergeClients } from '$api/MergeClients';
import { phoneKey } from '#lib/format/phone.js';

/** How many phones a client may have: the server's `MAX_CLIENT_PHONES`. */
export const MAX_CLIENT_PHONES = 10;

/** A client with the number of their orders (the target is the one with more). */
export type MergeClient = Client & { orders_count: number };

export const MERGE_FIELDS = ['full_name', 'address', 'note', 'source', 'blacklisted'] as const;
export type MergeField = (typeof MERGE_FIELDS)[number];

/** The client who stays and, for every field, the number of the client the value is taken from. */
export type MergeChoice = { targetId: number; from: Record<MergeField, number> };

const filled = (c: Client, f: MergeField): boolean => (f === 'blacklisted' ? c[f] : c[f] !== '');

/** The target has the most orders (the smaller number on a tie); a field is the target's or the first filled. */
export function defaultChoice(clients: readonly MergeClient[]): MergeChoice {
	const target = clients.reduce((best, c) =>
		c.orders_count > best.orders_count || (c.orders_count === best.orders_count && c.id < best.id)
			? c
			: best
	);
	const others = [target, ...clients.filter((c) => c !== target)];
	const from = {} as Record<MergeField, number>;
	for (const f of MERGE_FIELDS) {
		// A blacklisted client anywhere blacklists the result; the other fields: the first that has a value.
		from[f] = (others.find((c) => filled(c, f)) ?? target).id;
	}
	return { targetId: target.id, from };
}

export function toMergeRequest(
	choice: MergeChoice,
	clients: readonly Client[],
	dropped: ReadonlySet<string> = new Set()
): MergeClients {
	const byId = (id: number) => clients.find((c) => c.id === id);
	const target = byId(choice.targetId);
	if (!target) throw new Error(`client ${choice.targetId} is not in the merge`);
	const pick = <F extends MergeField>(f: F): Client[F] => (byId(choice.from[f]) ?? target)[f];
	return {
		target_id: target.id,
		target_version: target.version,
		sources: clients
			.filter((c) => c.id !== target.id)
			.map((c) => ({ id: c.id, version: c.version })),
		full_name: pick('full_name'),
		address: pick('address'),
		note: pick('note'),
		source: pick('source'),
		blacklisted: pick('blacklisted'),
		// The numbers the user unchecked: the server leaves them out in the same step as the merge.
		drop_phones: [...dropped]
	};
}

/** As the server joins them: the target's numbers, then each source's in order; a number met again is dropped. */
export function mergedPhones(clients: readonly Client[], targetId: number): ClientPhone[] {
	const target = clients.find((c) => c.id === targetId);
	const ordered = target ? [target, ...clients.filter((c) => c !== target)] : [...clients];
	const seen = new Set<string>();
	const out: ClientPhone[] = [];
	for (const c of ordered) {
		for (const p of c.phones) {
			const key = phoneKey(p.phone);
			if (key === '' || seen.has(key)) continue;
			seen.add(key);
			out.push(p);
		}
	}
	return out;
}

/** The joined numbers without those the user unchecked (`dropped`: their `phoneKey`s). */
export function keptMergedPhones(
	clients: readonly Client[],
	targetId: number,
	dropped: ReadonlySet<string>
): ClientPhone[] {
	return mergedPhones(clients, targetId).filter((p) => !dropped.has(phoneKey(p.phone)));
}

/** The choice after the clients were read again: values the user picked stay while their client is there. */
export function keepChoice(previous: MergeChoice, clients: readonly MergeClient[]): MergeChoice {
	const ids = new Set(clients.map((c) => c.id));
	const fresh = defaultChoice(clients);
	if (!ids.has(previous.targetId)) return fresh;
	const from = { ...fresh.from };
	for (const f of MERGE_FIELDS) if (ids.has(previous.from[f])) from[f] = previous.from[f];
	return { targetId: previous.targetId, from };
}
