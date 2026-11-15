export const STATUS_HUES = [
	'gray',
	'red',
	'orange',
	'amber',
	'lime',
	'green',
	'teal',
	'cyan',
	'blue',
	'indigo',
	'violet',
	'pink'
] as const;

export type StatusHue = (typeof STATUS_HUES)[number];

/** Hue name stored in `Status.color`; an unknown name renders gray. */
export function statusHue(color: string): StatusHue {
	const c = color.trim().toLowerCase();
	return STATUS_HUES.find((h) => h === c) ?? 'gray';
}
