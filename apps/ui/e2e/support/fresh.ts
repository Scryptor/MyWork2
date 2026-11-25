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
	logo: { chromium: 4179, webkit: 4180 },
	/** `template-editor.spec.ts`: the templates of the documents are saved and put back, and stickers and acts are what every printing spec on 4173 renders. */
	templates: { chromium: 4181, webkit: 4182 },
	/** `backups.spec.ts`: copies are made, restored (requested) and deleted in the data folder of the base. */
	backups: { chromium: 4187, webkit: 4188 },
	/**
	 * `stock-map.spec.ts`: racks and cells are the base's own, and the sticker of a cell is a template of it (spec 5.5,
	 * plan 8.6). WebKit refuses 4190 (a restricted port of its list, «sieve»), and every page of the specs sets the
	 * storage of all these origins first: so its server is on 4191.
	 */
	stockMap: { chromium: 4189, webkit: 4191 }
} as const;

export type OwnKind = keyof typeof OWN_PORTS;

/** The origin of the server of `kind` for this engine. */
export function ownOrigin(kind: OwnKind, browserName: string): string {
	const ports: Record<string, number> = OWN_PORTS[kind];
	const port = ports[browserName];
	if (port === undefined) throw new Error(`no own server for ${browserName}`);
	return `http://127.0.0.1:${port}`;
}

// Servers with an administrator (`--admin`, as 4174) that belong to one spec each: the limiter of sign-in
// attempts counts an attempt when it is admitted, by account and by address, and the shared server on 4174
// has every spec that signs in; a spec that makes wrong attempts on purpose has a server of its own.
export const ADMIN_PORTS = {
	/** `login.spec.ts`: wrong passwords, repeated sign-ins of the same account. */
	login: { chromium: 4183, webkit: 4184 },
	/** `reports.spec.ts`: the «Отчёты» count every issued order of the base; the spec relaxes «Обязательно при выдаче» for its own orders. */
	reports: { chromium: 4185, webkit: 4186 }
} as const;

export type AdminKind = keyof typeof ADMIN_PORTS;

/** The origin of the server with an administrator of `kind` for this engine. */
export function adminOrigin(kind: AdminKind, browserName: string): string {
	const ports: Record<string, number> = ADMIN_PORTS[kind];
	const port = ports[browserName];
	if (port === undefined) throw new Error(`no admin server for ${browserName}`);
	return `http://127.0.0.1:${port}`;
}
