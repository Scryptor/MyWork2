import type { AppSettings } from '$api/AppSettings';
import type { CurrentUser } from '$api/CurrentUser';
import type { Permission } from '$api/Permission';
import type { ServerInfo } from '$api/ServerInfo';
import type { ServicePoint } from '$api/ServicePoint';
import type { Status } from '$api/Status';
import { getSettings, listServicePoints, listStatuses } from '#lib/api/catalogs.js';
import { readStorage, removeStorage, writeStorage } from '#lib/design/storage.js';

/** The point chosen in the top bar, kept on this device; none stored is «Все точки». */
export const POINT_KEY = 'mw.point';

export interface AppData {
	server: ServerInfo;
	me: CurrentUser;
	statuses: Status[];
	points: ServicePoint[];
	settings: AppSettings;
}

/**
 * Statuses, points and settings as the program keeps them. The archived statuses and points come too: the
 * orders in them still show their status and point, and «В работе» still lists them. What is offered for a
 * choice leaves the archived ones out (`activePoints`, the pickers, the filters).
 */
export function readCatalogs(): Promise<[Status[], ServicePoint[], AppSettings]> {
	return Promise.all([listStatuses(true), listServicePoints(true), getSettings()]);
}

function storedPoint(points: ServicePoint[]): number | null {
	const raw = readStorage(POINT_KEY);
	const id = raw === null ? NaN : Number(raw);
	return points.some((p) => p.id === id && !p.archived) ? id : null;
}

/**
 * What every screen of the program knows: the server, the user, the catalogs and the chosen point.
 * Set by the load of `(app)/+layout.ts` before the screens render.
 */
class AppContext {
	#data = $state.raw<AppData | null>(null);
	#point = $state<number | null>(null);
	#statuses = $derived(new Map((this.#data?.statuses ?? []).map((s) => [s.id, s])));

	#get(): AppData {
		if (!this.#data) throw new Error('app context is used before (app)/+layout.ts loaded it');
		return this.#data;
	}

	set(data: AppData): void {
		this.#data = data;
		this.#point = storedPoint(data.points);
	}

	get server(): ServerInfo {
		return this.#get().server;
	}
	get me(): CurrentUser {
		return this.#get().me;
	}
	get statuses(): Status[] {
		return this.#get().statuses;
	}
	get points(): ServicePoint[] {
		return this.#get().points;
	}
	/** Points that can be chosen: not archived, in their order. */
	get activePoints(): ServicePoint[] {
		return this.points.filter((p) => !p.archived);
	}
	get settings(): AppSettings {
		return this.#get().settings;
	}

	get point(): number | null {
		return this.#point;
	}
	set point(id: number | null) {
		this.#point = id;
		if (id === null) removeStorage(POINT_KEY);
		else writeStorage(POINT_KEY, String(id));
	}

	/** Open mode comes with every permission from the server. */
	can(p: Permission): boolean {
		return this.me.permissions.includes(p);
	}

	statusById(id: number): Status | undefined {
		return this.#statuses.get(id);
	}

	/** Statuses, points and settings again, after they were changed. */
	async refreshCatalogs(): Promise<void> {
		const [statuses, points, settings] = await readCatalogs();
		this.#data = { ...this.#get(), statuses, points, settings };
		if (this.#point !== null && !points.some((p) => p.id === this.#point && !p.archived)) {
			this.point = null;
		}
	}
}

export const app = new AppContext();
