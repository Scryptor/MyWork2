// Servers with a new base (`server.mjs --fresh`): no setup mark, no data. A base is set up, or imported into,
// only once, and the two engines run at the same time on a computer (in CI each engine is a job of its own):
// so every spec that needs a new base has a server per engine.
export const FRESH_PORTS = {
	/** `setup.spec.ts`: «Начать с пустой базы». */
	setup: { chromium: 4175, webkit: 4177 },
	/** `import.spec.ts`: the transfer from MyWork 2. */
	import: { chromium: 4176, webkit: 4178 }
} as const;

export type FreshKind = keyof typeof FRESH_PORTS;

/** The origin of the new base of `kind` for this engine. */
export function freshOrigin(kind: FreshKind, browserName: string): string {
	const ports: Record<string, number> = FRESH_PORTS[kind];
	const port = ports[browserName];
	if (port === undefined) throw new Error(`no fresh server for ${browserName}`);
	return `http://127.0.0.1:${port}`;
}
