export interface ZoneChoice {
	/** The names to offer, the current one included. */
	items: { value: string }[];
	/** The engine could not list zones: the name is typed and the server checks it. */
	free: boolean;
}

/** The time zones to offer; `Intl.supportedValuesOf` is missing in older engines (WebKit before 15.4). */
export function timezoneChoices(current: string): ZoneChoice {
	const list: unknown =
		typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : null;
	if (!Array.isArray(list) || list.length === 0)
		return { items: current ? [{ value: current }] : [], free: true };
	const names = list as string[];
	const all = !current || names.includes(current) ? names : [current, ...names];
	return { items: all.map((value) => ({ value })), free: false };
}
