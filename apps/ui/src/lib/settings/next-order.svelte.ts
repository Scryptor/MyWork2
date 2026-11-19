// «Следующий номер заказа» of «Общие»: not a setting but a counter with its own requests. It is shown only
// with «settings_manage» and is read when the right is there (it may come after the screen is open). The
// number is sent when the field is left or on Enter, one request at a time: what is typed while a request is
// in flight waits in the queue and goes after it; the answer of an older request never overwrites newer input.
import { ApiError, errorMessage } from '#lib/api/client.js';
import { ru } from '#lib/i18n/ru.js';

const t = ru.settings;

/** The counter is a 32-bit number in the database; a bigger one would be refused with no word of why. */
export const MAX_NEXT_ORDER = 999_999_999;

export function createNextOrder(opts: {
	can: () => boolean;
	get: () => Promise<{ next: number }>;
	set: (next: number) => Promise<unknown>;
}) {
	let text = $state('');
	let error = $state<string | undefined>();
	let server: number | null = null;
	let loading = false;
	let busy = false;
	let again = false;

	async function load(): Promise<void> {
		if (!opts.can() || server !== null || loading) return;
		loading = true;
		try {
			const { next } = await opts.get();
			server = next;
			// Not over what is typed already.
			if (text === '') text = String(next);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			loading = false;
		}
	}

	async function commit(): Promise<void> {
		if (busy) {
			again = true;
			return;
		}
		busy = true;
		try {
			do {
				again = false;
				await sendOnce();
			} while (again);
		} finally {
			busy = false;
		}
	}

	async function sendOnce(): Promise<void> {
		const raw = text.trim();
		if (!opts.can() || server === null || raw === String(server)) return;
		if (!/^\d+$/.test(raw) || Number(raw) < 1) {
			error = t.nextOrderNotNumber;
			text = String(server);
			return;
		}
		if (Number(raw) > MAX_NEXT_ORDER) {
			error = t.nextOrderTooBig;
			text = String(server);
			return;
		}
		try {
			await opts.set(Number(raw));
			server = Number(raw);
			if (text.trim() === raw) {
				text = String(server);
				error = undefined;
			}
		} catch (e) {
			// Newer input has its own turn: it neither loses its text nor gets a mistake about the older one.
			if (text.trim() !== raw) return;
			error =
				e instanceof ApiError && e.code === 'validation' ? t.nextOrderTooSmall : errorMessage(e);
			text = String(server);
		}
	}

	return {
		get text() {
			return text;
		},
		set text(v: string) {
			text = v;
			error = undefined;
		},
		get error() {
			return error;
		},
		load,
		commit
	};
}

export type NextOrder = ReturnType<typeof createNextOrder>;
