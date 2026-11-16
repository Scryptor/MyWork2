import { parseLocalDateTime, nowLocal, type LocalDateTime } from '#lib/format/date.js';

/** Moment as a number of milliseconds that behaves like a UTC clock for local-time fields. */
const asMillis = (t: LocalDateTime) =>
	Date.UTC(t.year, t.month - 1, t.day, t.hour, t.minute, t.second);

function fromMillis(ms: number): LocalDateTime {
	const d = new Date(ms);
	return {
		year: d.getUTCFullYear(),
		month: d.getUTCMonth() + 1,
		day: d.getUTCDate(),
		hour: d.getUTCHours(),
		minute: d.getUTCMinutes(),
		second: d.getUTCSeconds()
	};
}

class ServerClock {
	/** Server time minus `Date.now()` at the moment of the last sync; null before the first sync. */
	offset = $state<number | null>(null);

	sync(serverTime: string): void {
		const t = parseLocalDateTime(serverTime);
		if (!t) return;
		this.offset = asMillis(t) - Date.now();
	}

	/** Enterprise local time by the server clock. */
	now(): LocalDateTime {
		return this.offset === null ? nowLocal() : fromMillis(Date.now() + this.offset);
	}
}

export const serverClock = new ServerClock();
