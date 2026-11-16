type Props = Record<PropertyKey, unknown>;

/**
 * Props of two bits-ui triggers on one element (a tooltip inside a popover trigger): event handlers of
 * both run, the later set wins for the rest. Symbol keys (attachments such as the ref of a trigger) are
 * unique per trigger and all kept.
 */
export function mergeTriggerProps(base: Props, over: Props): Props {
	const out: Props = { ...base };
	for (const key of Reflect.ownKeys(over)) {
		const a = out[key];
		const b = over[key];
		if (b === undefined) continue;
		out[key] =
			typeof key === 'string' &&
			key.startsWith('on') &&
			typeof a === 'function' &&
			typeof b === 'function'
				? (...args: unknown[]) => {
						(a as (...x: unknown[]) => void)(...args);
						(b as (...x: unknown[]) => void)(...args);
					}
				: b;
	}
	return out;
}
