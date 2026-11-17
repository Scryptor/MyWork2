import type { Client } from '$api/Client';
import type { ClientListQuery } from '$api/ClientListQuery';
import type { ClientPage } from '$api/ClientPage';
import type { Order } from '$api/Order';
import type { OrderListItem } from '$api/OrderListItem';
import type { OrderListQuery } from '$api/OrderListQuery';
import type { OrderPage } from '$api/OrderPage';
import type { Status } from '$api/Status';
import { ApiError } from '#lib/api/client.js';
import { displayName, formatPhone, shortName } from '#lib/format/index.js';
import { ru } from '#lib/i18n/ru.js';
import { deviceOf } from '#lib/orders/view.js';
import { filterCommands, type CommandGroup, type CommandItem } from '#lib/ui/commands.js';

/** Pause after the last key before the palette asks the server. */
export const PALETTE_DELAY_MS = 150;
const LIMIT = 5;

type OrderLike = Pick<OrderListItem, 'brand' | 'model' | 'device_type' | 'id' | 'status_id'> & {
	client_name?: string;
};

function orderCommand(o: OrderLike, statusOf?: (id: number) => Status | undefined): CommandItem {
	const status = statusOf?.(o.status_id);
	const hint = [deviceOf(o), o.client_name ? shortName(o.client_name) : '']
		.filter(Boolean)
		.join(' · ');
	return {
		id: `order:${o.id}`,
		label: `№ ${o.id}`,
		hint: hint || undefined,
		status: status ? { name: status.name, color: status.color } : undefined
	};
}

function clientCommand(c: Client): CommandItem {
	const phone = c.phones[0]?.phone;
	return {
		id: `client:${c.id}`,
		label: displayName(c.full_name),
		hint: phone ? formatPhone(phone) : undefined
	};
}

export interface PaletteInput {
	query: string;
	orders: OrderListItem[];
	/** The order whose number is the query, if there is one. */
	exact: Order | null;
	clients: Client[];
	recent: OrderListItem[];
	commands: CommandItem[];
	statusOf?: (id: number) => Status | undefined;
}

/**
 * Groups of Ctrl K (spec 6.10): an empty query gives «Недавние» and «Команды»; otherwise «Заказы» (the
 * order with this number first, once), «Клиенты» and the matching «Команды».
 */
export function paletteGroups(input: PaletteInput): CommandGroup[] {
	const { statusOf } = input;
	if (input.query.trim() === '') {
		return [
			{
				id: 'recent',
				title: ru.palette.recent,
				items: input.recent.map((o) => orderCommand(o, statusOf))
			},
			{ id: 'commands', title: ru.palette.commands, items: input.commands }
		];
	}
	const exact = input.exact;
	const rest = input.orders.filter((o) => o.id !== exact?.id);
	// The search row has the client's name; the order itself does not.
	const first: OrderLike | undefined = exact
		? (input.orders.find((o) => o.id === exact.id) ?? exact)
		: undefined;
	const orders: OrderLike[] = first ? [first, ...rest] : rest;
	return [
		{
			id: 'orders',
			title: ru.palette.orders,
			items: orders.map((o) => orderCommand(o, statusOf))
		},
		{ id: 'clients', title: ru.palette.clients, items: input.clients.map(clientCommand) },
		{
			id: 'commands',
			title: ru.palette.commands,
			items: filterCommands(input.commands, input.query)
		}
	];
}

/** A query of 1 to 7 digits is an order number. */
export function exactNumber(query: string): number | null {
	const q = query.trim();
	if (!/^\d{1,7}$/.test(q)) return null;
	const n = Number(q);
	return n > 0 ? n : null;
}

export interface PaletteResults {
	query: string;
	orders: OrderListItem[];
	exact: Order | null;
	clients: Client[];
}

export interface PaletteDeps {
	listOrders(q: OrderListQuery, signal?: AbortSignal): Promise<OrderPage>;
	getOrder(id: number, signal?: AbortSignal): Promise<Order>;
	listClients(q: ClientListQuery, signal?: AbortSignal): Promise<ClientPage>;
}

const isAbort = (e: unknown) => e instanceof DOMException && e.name === 'AbortError';
const isNotFound = (e: unknown) => e instanceof ApiError && e.status === 404;

/**
 * The searches of Ctrl K: PALETTE_DELAY_MS after the last key, each one aborting the one before.
 * An empty query answers `null` at once (the palette shows «Недавние»).
 */
export function createPaletteSearch(
	deps: PaletteDeps,
	on: { result(r: PaletteResults | null): void; error(e: unknown, query: string): void }
) {
	let timer: ReturnType<typeof setTimeout> | undefined;
	let controller: AbortController | null = null;

	function cancel() {
		clearTimeout(timer);
		timer = undefined;
		controller?.abort();
		controller = null;
	}

	async function run(query: string, c: AbortController) {
		const n = exactNumber(query);
		try {
			const [orders, exact, clients] = await Promise.all([
				deps.listOrders({ query, limit: LIMIT }, c.signal).then((p) => p.items),
				n === null
					? null
					: deps.getOrder(n, c.signal).catch((e: unknown) => {
							if (isNotFound(e)) return null;
							throw e;
						}),
				deps.listClients({ query, limit: LIMIT }, c.signal).then((p) => p.items)
			]);
			if (!c.signal.aborted) on.result({ query, orders, exact, clients });
		} catch (e) {
			if (!c.signal.aborted && !isAbort(e)) on.error(e, query);
		} finally {
			if (controller === c) controller = null;
		}
	}

	return {
		search(raw: string): void {
			cancel();
			const query = raw.trim();
			if (query === '') {
				on.result(null);
				return;
			}
			timer = setTimeout(() => {
				timer = undefined;
				const c = new AbortController();
				controller = c;
				void run(query, c);
			}, PALETTE_DELAY_MS);
		},
		cancel
	};
}

/** A list row made of an order and its client (the API has no list of orders by numbers). */
export function listItemOf(o: Order, c: Client | null): OrderListItem {
	return {
		id: o.id,
		received_at: o.received_at,
		issued_at: o.issued_at,
		client_id: o.client_id,
		client_name: c?.full_name ?? '',
		client_phone: c?.phones[0]?.phone ?? '',
		client_blacklisted: c?.blacklisted ?? false,
		service_point_id: o.service_point_id,
		status_id: o.status_id,
		device_type: o.device_type,
		brand: o.brand,
		model: o.model,
		serial: o.serial,
		malfunction: o.malfunction,
		master: o.master,
		estimate_from: o.estimate_from,
		estimate_to: o.estimate_to,
		prepayment: o.prepayment,
		final_price: o.final_price,
		need_call: o.need_call,
		need_parts: o.need_parts,
		barcode: o.barcode,
		deleted_at: o.deleted_at,
		version: o.version
	};
}

/** «Недавние»: the orders of these numbers in the same order; an order that is gone is skipped. */
export async function loadRecent(
	ids: number[],
	deps: { getOrder(id: number): Promise<Order>; getClient(id: number): Promise<Client> }
): Promise<OrderListItem[]> {
	const orders = await Promise.all(ids.map((id) => deps.getOrder(id).catch(() => null)));
	const found = orders.filter((o): o is Order => o !== null && o.deleted_at === null);
	const clientIds = [...new Set(found.map((o) => o.client_id))];
	const clients = await Promise.all(clientIds.map((id) => deps.getClient(id).catch(() => null)));
	const byId = new Map(clients.flatMap((c) => (c ? [[c.id, c] as const] : [])));
	return found.map((o) => listItemOf(o, byId.get(o.client_id) ?? null));
}
