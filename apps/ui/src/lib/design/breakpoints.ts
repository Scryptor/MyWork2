export type ShellLayout = 'phone' | 'rail' | 'full';

export const PHONE_QUERY = '(max-width: 599px)';
export const RAIL_QUERY = '(min-width: 600px) and (max-width: 1279px)';

export function layoutForWidth(width: number): ShellLayout {
	if (width < 600) return 'phone';
	if (width < 1280) return 'rail';
	return 'full';
}
