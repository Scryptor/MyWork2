// Only for tests: the desktop program's bridge (`window.__TAURI_INTERNALS__`) without the program.
import { clearMocks, mockIPC, mockWindows } from '@tauri-apps/api/mocks';

const bridge = window as unknown as Record<string, unknown>;

/**
 * The desktop program's bridge: records who listens to which window event and can fire one. With `hold`,
 * a subscription is answered only on `release()` — the moment between asking and being subscribed.
 */
export function fakeDesktop({ hold = false } = {}) {
	const listeners = new Map<number, string>();
	const asked: string[] = [];
	const held: (() => void)[] = [];
	mockWindows('main');
	mockIPC((cmd, args) => {
		const a = args as { event: string; handler: number; eventId: number };
		if (cmd === 'plugin:event|listen') {
			asked.push(a.event);
			const subscribe = () => {
				listeners.set(a.handler, a.event);
				return a.handler;
			};
			if (!hold) return subscribe();
			return new Promise<number>((resolve) => held.push(() => resolve(subscribe())));
		}
		if (cmd === 'plugin:event|unlisten') listeners.delete(a.eventId);
		return null;
	});
	const internals = bridge.__TAURI_INTERNALS__ as {
		runCallback: (id: number, data: unknown) => void;
	};
	return {
		asked: () => [...asked],
		listening: () => [...listeners.values()].sort(),
		release: () => held.splice(0).forEach((answer) => answer()),
		fire(event: string, payload: unknown) {
			for (const [id, name] of listeners)
				if (name === event) internals.runCallback(id, { event, id, payload });
		}
	};
}

/** Back to a browser: the bridge is gone. */
export function leaveDesktop(): void {
	clearMocks();
	delete bridge.__TAURI_INTERNALS__;
	delete bridge.__TAURI_EVENT_PLUGIN_INTERNALS__;
}
