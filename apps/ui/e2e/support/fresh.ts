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

// Servers that are set up (no `--fresh`, no `--admin`: open mode) but belong to one spec each, for what is the
// firm's own and one per base, and so cannot be shared by the two engines running at the same time.
export const OWN_PORTS = {
	/** `settings-logo.spec.ts`: the logo of the firm is uploaded and removed. */
	logo: { chromium: 4179, webkit: 4180 }
} as const;

export type OwnKind = keyof typeof OWN_PORTS;

/** The origin of the server of `kind` for this engine. */
export function ownOrigin(kind: OwnKind, browserName: string): string {
	const ports: Record<string, number> = OWN_PORTS[kind];
	const port = ports[browserName];
	if (port === undefined) throw new Error(`no own server for ${browserName}`);
	return `http://127.0.0.1:${port}`;
}
